//! Production Windows FLOW 8 transport.
//!
//! FLOW 8 has a WRITE | NOTIFY transport characteristic but no CCCD. The
//! Windows backend enumerates the native Windows BLE/GATT device interfaces,
//! opens the matching service interface, and uses BluetoothGATTRegisterEvent.
//! It never calls the standard BLE subscription path.

use std::{
    ffi::c_void,
    mem::size_of,
    ptr::addr_of,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};
use windows::{
    Devices::{
        Bluetooth::{
            BluetoothConnectionStatus, BluetoothLEDevice, GenericAttributeProfile::GattSession,
        },
        Enumeration::{DeviceInformation, DeviceInformationCollection},
    },
    Foundation::{IPropertyValue, PropertyType, TypedEventHandler},
    Win32::{
        Devices::{
            Bluetooth::{
                BLUETOOTH_GATT_FLAG_NONE, BLUETOOTH_GATT_VALUE_CHANGED_EVENT,
                BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION, BTH_LE_GATT_CHARACTERISTIC,
                BTH_LE_GATT_CHARACTERISTIC_VALUE, BTH_LE_GATT_SERVICE, BTH_LE_UUID,
                BluetoothGATTGetCharacteristics, BluetoothGATTGetServices,
                BluetoothGATTRegisterEvent, BluetoothGATTSetCharacteristicValue,
                BluetoothGATTUnregisterEvent, CharacteristicValueChangedEvent,
                GUID_BLUETOOTH_GATT_SERVICE_DEVICE_INTERFACE, GUID_BLUETOOTHLE_DEVICE_INTERFACE,
            },
            DeviceAndDriverInstallation::{
                CM_Get_Device_IDW, CM_Get_Parent, CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
                HDEVINFO, MAX_DEVICE_ID_LEN, SP_DEVICE_INTERFACE_DATA,
                SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA, SetupDiDestroyDeviceInfoList,
                SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
                SetupDiGetDeviceInterfaceDetailW, SetupDiGetDevicePropertyW,
            },
            Properties::{
                DEVPKEY_Device_ContainerId, DEVPKEY_Device_Parent, DEVPROP_TYPE_GUID,
                DEVPROP_TYPE_STRING, DEVPROPTYPE,
            },
        },
        Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, GENERIC_READ, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        },
    },
    core::{GUID, HRESULT, HSTRING, IInspectable, Interface, PCWSTR},
};
use windows_collections::IIterable;

use super::{
    BleError, DeviceEvent, NativeConnectionError, NativeConnectionStage, RxIngress, TransportRx,
};

const FLOW_SERVICE_GUID: GUID = GUID::from_u128(0x14839ad4_8d7e_415c_9a42_167340cf2339);
const FLOW_CHARACTERISTIC_GUID: GUID = GUID::from_u128(0x0034594a_a8e7_4b1a_a6b1_cd5243059a57);
const MAX_FLOW_FRAME_BYTES: usize = 512;
const DEVICE_INSTANCE_ID_PROPERTY: &str = "System.Devices.DeviceInstanceId";
const CONTAINER_ID_PROPERTY: &str = "System.Devices.ContainerId";
const PARENT_ID_PROPERTY: &str = "System.Devices.Parent";
const REQUESTED_IDENTITY_PROPERTIES: [&str; 3] = [
    DEVICE_INSTANCE_ID_PROPERTY,
    CONTAINER_ID_PROPERTY,
    PARENT_ID_PROPERTY,
];

struct DeviceInfoSet(HDEVINFO);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: this object uniquely owns the SetupDiGetClassDevsW result.
            if let Err(error) = unsafe { SetupDiDestroyDeviceInfoList(self.0) } {
                warn!(
                    error = %native_error(&error),
                    "closing SetupDi device information set failed"
                );
            }
        }
    }
}

struct ServiceHandle(HANDLE);

// Windows kernel handles may be used from the dedicated transport runtime
// thread. The callback never receives or closes this handle.
unsafe impl Send for ServiceHandle {}

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: this object uniquely owns the CreateFileW result.
            if let Err(error) = unsafe { CloseHandle(self.0) } {
                warn!(error = %native_error(&error), "closing native FLOW 8 service handle failed");
            }
        }
    }
}

struct NativeGattCandidate {
    path: String,
    owner_instance_id: String,
    owner_container_id: Option<GUID>,
    handle: ServiceHandle,
    service: BTH_LE_GATT_SERVICE,
    characteristic: BTH_LE_GATT_CHARACTERISTIC,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowDeviceIdentity {
    device_instance_id: Option<String>,
    container_id: Option<GUID>,
    parent_id: Option<String>,
}

impl FlowDeviceIdentity {
    fn is_sufficient_for_matching(&self) -> bool {
        self.container_id.is_some() || self.device_instance_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeDeviceInterface {
    path: String,
    instance_id: String,
    container_id: Option<GUID>,
    parent_chain: Vec<String>,
}

struct CallbackContext {
    ingress: RxIngress,
    value_handle: u16,
    in_flight: AtomicUsize,
    drained_lock: Mutex<()>,
    drained: Condvar,
}

impl CallbackContext {
    fn wait_until_drained(&self) {
        let mut guard = self
            .drained_lock
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while self.in_flight.load(Ordering::Acquire) != 0 {
            guard = self
                .drained
                .wait(guard)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

struct CallbackUse<'a>(&'a CallbackContext);

impl Drop for CallbackUse<'_> {
    fn drop(&mut self) {
        if self.0.in_flight.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.drained.notify_all();
        }
    }
}

struct NativeEventRegistration {
    handle: isize,
    context: Box<CallbackContext>,
}

impl NativeEventRegistration {
    fn invalidate(&self) {
        self.context.ingress.invalidate();
    }
}

impl Drop for NativeEventRegistration {
    fn drop(&mut self) {
        self.invalidate();
        // SAFETY: handle came from BluetoothGATTRegisterEvent and is
        // unregistered exactly once before callback context destruction.
        if let Err(error) =
            unsafe { BluetoothGATTUnregisterEvent(self.handle, BLUETOOTH_GATT_FLAG_NONE) }
        {
            warn!(error = %native_error(&error), "unregistering FLOW 8 native RX event failed");
        }
        self.context.wait_until_drained();
    }
}

/// Windows native service/characteristic owner. Field order is intentional:
/// the native registration is removed before the service handle is closed.
pub(super) struct WindowsNativeSession {
    registration: Option<NativeEventRegistration>,
    service_handle: ServiceHandle,
    service: BTH_LE_GATT_SERVICE,
    characteristic: BTH_LE_GATT_CHARACTERISTIC,
    device: BluetoothLEDevice,
    gatt_session: GattSession,
    connection_token: Option<i64>,
    ingress: RxIngress,
    mtu: u16,
    closed: bool,
}

impl WindowsNativeSession {
    pub(super) async fn connect(
        generation: u64,
        events: mpsc::UnboundedSender<DeviceEvent>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportRx>), BleError> {
        let (tx, rx) = mpsc::unbounded_channel();
        let ingress = RxIngress::new(generation, tx);

        report_stage(&events, NativeConnectionStage::Scanning);
        let (device, flow_identity) = find_flow8_device(&events).await?;
        report_stage(&events, NativeConnectionStage::DeviceObjectCreated);
        info!(
            backend = "windows-native-gatt",
            name = ?device.Name().map(|name| name.to_string()),
            device_instance_id = ?flow_identity.device_instance_id,
            container_id = ?flow_identity.container_id,
            parent_id = ?flow_identity.parent_id,
            "FLOW 8 device object created"
        );

        let device_id = device.BluetoothDeviceId().map_err(|error| {
            windows_error(
                NativeConnectionStage::DeviceObjectCreated,
                "BluetoothLEDevice.BluetoothDeviceId",
                error,
                true,
            )
        })?;
        let gatt_session = GattSession::FromDeviceIdAsync(&device_id)
            .map_err(|error| {
                windows_error(
                    NativeConnectionStage::DeviceObjectCreated,
                    "GattSession.FromDeviceIdAsync",
                    error,
                    true,
                )
            })?
            .await
            .map_err(|error| {
                windows_error(
                    NativeConnectionStage::DeviceObjectCreated,
                    "GattSession.FromDeviceIdAsync completion",
                    error,
                    true,
                )
            })?;
        let mtu = gatt_session.MaxPduSize().map_err(|error| {
            windows_error(
                NativeConnectionStage::DeviceObjectCreated,
                "GattSession.MaxPduSize",
                error,
                true,
            )
        })?;
        report_stage(&events, NativeConnectionStage::NativeInterfaceEnumerating);
        let device_interfaces = enumerate_device_interfaces(GUID_BLUETOOTHLE_DEVICE_INTERFACE)?;
        let matching_device_interfaces = device_interfaces
            .iter()
            .filter(|interface| belongs_to_flow8_device(interface, &flow_identity))
            .count();
        info!(
            backend = "windows-native-gatt",
            device_interfaces_total = device_interfaces.len(),
            device_interfaces_matching_flow_device = matching_device_interfaces,
            "associated native Bluetooth LE device interfaces with selected FLOW 8"
        );
        let candidate = open_flow_service_interface(&flow_identity, &events)?;
        let NativeGattCandidate {
            path,
            owner_instance_id,
            owner_container_id,
            handle: service_handle,
            service,
            characteristic,
            ..
        } = candidate;
        info!(
            backend = "windows-native-gatt",
            interface_path = %path,
            device_instance = %owner_instance_id,
            container_id = ?owner_container_id,
            service_uuid = ?FLOW_SERVICE_GUID,
            service_handle = service.AttributeHandle,
            declaration_handle = characteristic.AttributeHandle,
            value_handle = characteristic.CharacteristicValueHandle,
            writable = characteristic.IsWritable,
            notifiable = characteristic.IsNotifiable,
            "selected native FLOW 8 service and characteristic"
        );

        if !characteristic.IsWritable || !characteristic.IsNotifiable {
            return Err(native_failure(
                NativeConnectionStage::TargetCharacteristicFound,
                "BTH_LE_GATT_CHARACTERISTIC properties",
                false,
                format!(
                    "target characteristic properties are incompatible: writable={} notifiable={}",
                    characteristic.IsWritable, characteristic.IsNotifiable
                ),
            ));
        }

        report_stage(&events, NativeConnectionStage::NativeRxRegistering);
        let registration =
            register_value_changes(&service_handle, characteristic, ingress.clone())?;
        report_stage(&events, NativeConnectionStage::NativeRxArmed);
        info!(
            backend = "windows-native-gatt",
            cccd_write = false,
            btleplug_subscribe = false,
            "native FLOW 8 RX event path armed"
        );

        let connection_ingress = ingress.clone();
        let connection_handler =
            TypedEventHandler::<BluetoothLEDevice, IInspectable>::new(move |sender, _| {
                let result = sender.ok().and_then(|device| device.ConnectionStatus());
                match result {
                    Ok(BluetoothConnectionStatus::Connected) => {
                        connection_ingress.connected();
                    }
                    Ok(BluetoothConnectionStatus::Disconnected) => {
                        connection_ingress.disconnected();
                    }
                    Ok(_) => {}
                    Err(error) => {
                        connection_ingress.error(format!(
                            "ConnectionStatusChanged failed: {}",
                            native_error(&error)
                        ));
                    }
                }
                Ok(())
            });
        let connection_token = device
            .ConnectionStatusChanged(&connection_handler)
            .map_err(|error| {
                windows_error(
                    NativeConnectionStage::NativeRxArmed,
                    "BluetoothLEDevice.ConnectionStatusChanged",
                    error,
                    true,
                )
            })?;

        gatt_session.SetMaintainConnection(true).map_err(|error| {
            windows_error(
                NativeConnectionStage::NativeRxArmed,
                "GattSession.MaintainConnection",
                error,
                true,
            )
        })?;
        report_stage(&events, NativeConnectionStage::Handshaking);
        info!(
            backend = "windows-native-gatt",
            mtu, "physical FLOW 8 connection requested after native RX arm"
        );

        if device.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected) {
            ingress.connected();
        }

        Ok((
            Self {
                registration: Some(registration),
                service_handle,
                service,
                characteristic,
                device,
                gatt_session,
                connection_token: Some(connection_token),
                ingress,
                mtu,
                closed: false,
            },
            rx,
        ))
    }

    pub(super) fn mtu(&self) -> u16 {
        self.mtu
    }

    pub(super) async fn write(&self, frame: &[u8]) -> Result<(), BleError> {
        if frame.len() > MAX_FLOW_FRAME_BYTES {
            return Err(native_failure(
                NativeConnectionStage::Handshaking,
                "BluetoothGATTSetCharacteristicValue",
                false,
                format!(
                    "FLOW frame is too large for native write buffer: {} bytes",
                    frame.len()
                ),
            ));
        }
        let mut value = NativeGattValue::default();
        value.data_size = frame.len() as u32;
        value.data[..frame.len()].copy_from_slice(frame);
        // BLUETOOTH_GATT_FLAG_NONE is the with-response path.
        unsafe {
            BluetoothGATTSetCharacteristicValue(
                self.service_handle.0,
                &self.characteristic,
                (&value as *const NativeGattValue).cast::<BTH_LE_GATT_CHARACTERISTIC_VALUE>(),
                None,
                BLUETOOTH_GATT_FLAG_NONE,
            )
        }
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::Handshaking,
                "BluetoothGATTSetCharacteristicValue",
                error,
                true,
            )
        })?;
        debug!(
            backend = "windows-native-gatt",
            command = frame.first().copied().unwrap_or_default(),
            bytes = frame.len(),
            "FLOW frame written with response"
        );
        Ok(())
    }

    pub(super) fn disconnect(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.ingress.invalidate();
        self.registration.take();
        if let Some(token) = self.connection_token.take()
            && let Err(error) = self.device.RemoveConnectionStatusChanged(token)
        {
            warn!(error = %native_error(&error), "removing connection-status handler failed");
        }
        if let Err(error) = self.gatt_session.SetMaintainConnection(false) {
            warn!(error = %native_error(&error), "stopping FLOW 8 connection maintenance failed");
        }
        info!(
            backend = "windows-native-gatt",
            "FLOW 8 native transport disconnected"
        );
    }
}

impl Drop for WindowsNativeSession {
    fn drop(&mut self) {
        self.disconnect();
        // Reading the field documents that the enumerated service identity is
        // retained for the same lifetime as the native service handle.
        let _ = self.service;
    }
}

#[repr(C)]
struct NativeGattValue {
    data_size: u32,
    data: [u8; MAX_FLOW_FRAME_BYTES],
}

impl Default for NativeGattValue {
    fn default() -> Self {
        Self {
            data_size: 0,
            data: [0; MAX_FLOW_FRAME_BYTES],
        }
    }
}

unsafe extern "system" fn value_changed_callback(
    event_type: windows::Win32::Devices::Bluetooth::BTH_LE_GATT_EVENT_TYPE,
    event_parameter: *const c_void,
    context: *const c_void,
) {
    if event_type != CharacteristicValueChangedEvent
        || event_parameter.is_null()
        || context.is_null()
    {
        return;
    }
    // SAFETY: both pointers are owned by BluetoothGATTRegisterEvent for the
    // callback duration. NativeEventRegistration unregisters before dropping
    // CallbackContext.
    let callback_context = unsafe { &*(context.cast::<CallbackContext>()) };
    callback_context.in_flight.fetch_add(1, Ordering::AcqRel);
    let _callback_use = CallbackUse(callback_context);
    if !callback_context.ingress.active.load(Ordering::Acquire) {
        return;
    }
    let event = unsafe { &*(event_parameter.cast::<BLUETOOTH_GATT_VALUE_CHANGED_EVENT>()) };
    if event.ChangedAttributeHandle != callback_context.value_handle {
        callback_context.ingress.error(format!(
            "native value-change callback used unexpected handle 0x{:04X} (expected 0x{:04X})",
            event.ChangedAttributeHandle, callback_context.value_handle
        ));
        return;
    }
    if event.CharacteristicValue.is_null() {
        return;
    }
    let value = unsafe { &*event.CharacteristicValue };
    let data_size = value.DataSize as usize;
    let header_size = size_of::<u32>();
    if data_size == 0
        || data_size > MAX_FLOW_FRAME_BYTES
        || event.CharacteristicValueDataSize < header_size + data_size
    {
        callback_context.ingress.error(format!(
            "native value-change payload has invalid size: data={} container={}",
            data_size, event.CharacteristicValueDataSize
        ));
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(value.Data.as_ptr(), data_size) };
    callback_context.ingress.forward(bytes);
}

fn register_value_changes(
    service_handle: &ServiceHandle,
    characteristic: BTH_LE_GATT_CHARACTERISTIC,
    ingress: RxIngress,
) -> Result<NativeEventRegistration, BleError> {
    let registration = BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION {
        NumCharacteristics: 1,
        Characteristics: [characteristic],
    };
    let context = Box::new(CallbackContext {
        ingress,
        value_handle: characteristic.CharacteristicValueHandle,
        in_flight: AtomicUsize::new(0),
        drained_lock: Mutex::new(()),
        drained: Condvar::new(),
    });
    let context_ptr = (&*context as *const CallbackContext).cast::<c_void>();
    let mut event_handle = 0isize;
    unsafe {
        BluetoothGATTRegisterEvent(
            service_handle.0,
            CharacteristicValueChangedEvent,
            (&registration as *const BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION)
                .cast::<c_void>(),
            Some(value_changed_callback),
            Some(context_ptr),
            &mut event_handle,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    }
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::NativeRxRegistering,
            "BluetoothGATTRegisterEvent",
            error,
            true,
        )
    })?;
    Ok(NativeEventRegistration {
        handle: event_handle,
        context,
    })
}

async fn find_flow8_device(
    events: &mpsc::UnboundedSender<DeviceEvent>,
) -> Result<(BluetoothLEDevice, FlowDeviceIdentity), BleError> {
    info!(backend = "windows-native-gatt", "searching for FLOW 8 LE");
    let selector = BluetoothLEDevice::GetDeviceSelectorFromDeviceName(&HSTRING::from("FLOW 8 LE"))
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::Scanning,
                "BluetoothLEDevice.GetDeviceSelectorFromDeviceName",
                error,
                true,
            )
        })?;
    debug!(
        requested_properties = ?REQUESTED_IDENTITY_PROPERTIES,
        "requesting FLOW 8 Windows PnP identity properties"
    );
    let requested_properties: IIterable<HSTRING> = REQUESTED_IDENTITY_PROPERTIES
        .into_iter()
        .map(HSTRING::from)
        .collect::<Vec<_>>()
        .into();
    let devices = DeviceInformation::FindAllAsyncAqsFilterAndAdditionalProperties(
        &selector,
        &requested_properties,
    )
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::Scanning,
            "DeviceInformation.FindAllAsyncAqsFilterAndAdditionalProperties",
            error,
            true,
        )
    })?
    .await
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::Scanning,
            "DeviceInformation.FindAllAsyncAqsFilterAndAdditionalProperties completion",
            error,
            true,
        )
    })?;
    let info = select_flow8_device_information(devices)?;
    report_stage(events, NativeConnectionStage::DeviceFound);
    let id = info.Id().map_err(|error| {
        windows_error(
            NativeConnectionStage::DeviceFound,
            "DeviceInformation.Id",
            error,
            true,
        )
    })?;
    let kind = info.Kind().map_err(|error| {
        windows_error(
            NativeConnectionStage::DeviceFound,
            "DeviceInformation.Kind",
            error,
            true,
        )
    })?;
    debug!(
        device_information_id = %id,
        device_information_kind = ?kind,
        requested_properties = ?REQUESTED_IDENTITY_PROPERTIES,
        "selected FLOW 8 DeviceInformation"
    );
    let identity = flow_device_identity(&info)?;
    let device = BluetoothLEDevice::FromIdAsync(&id)
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::DeviceFound,
                "BluetoothLEDevice.FromIdAsync",
                error,
                true,
            )
        })?
        .await
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::DeviceFound,
                "BluetoothLEDevice.FromIdAsync completion",
                error,
                true,
            )
        })?;
    Ok((device, identity))
}

fn select_flow8_device_information(
    devices: DeviceInformationCollection,
) -> Result<DeviceInformation, BleError> {
    devices
        .into_iter()
        .find(|device| {
            device
                .Name()
                .map(|name| name.to_string() == "FLOW 8 LE")
                .unwrap_or(false)
        })
        .ok_or(BleError::DeviceNotFound)
}

fn flow_device_identity(info: &DeviceInformation) -> Result<FlowDeviceIdentity, BleError> {
    let properties = info.Properties().map_err(|error| {
        windows_error(
            NativeConnectionStage::DeviceFound,
            "DeviceInformation.Properties",
            error,
            true,
        )
    })?;
    let available_property_keys = available_property_keys(&properties);
    debug!(
        requested_properties = ?REQUESTED_IDENTITY_PROPERTIES,
        available_property_keys = ?available_property_keys,
        "FLOW 8 DeviceInformation property bag"
    );

    let identity = FlowDeviceIdentity {
        device_instance_id: try_get_string_property(&properties, DEVICE_INSTANCE_ID_PROPERTY)?
            .filter(|value| !value.is_empty()),
        container_id: try_get_guid_property(&properties, CONTAINER_ID_PROPERTY)?,
        parent_id: try_get_string_property(&properties, PARENT_ID_PROPERTY)?
            .filter(|value| !value.is_empty()),
    };
    info!(
        backend = "windows-native-gatt",
        device_instance_id = ?identity.device_instance_id,
        container_id = ?identity.container_id,
        parent_id = ?identity.parent_id,
        "resolved partial FLOW 8 Windows device identity"
    );

    if !identity.is_sufficient_for_matching() {
        return Err(logical_failure(
            NativeConnectionStage::DeviceFound,
            "FlowDeviceIdentity",
            "identity_insufficient",
            true,
            "neither System.Devices.ContainerId nor System.Devices.DeviceInstanceId is available; Parent alone is not a reliable FLOW 8 identity",
        ));
    }

    Ok(identity)
}

fn available_property_keys(
    properties: &windows_collections::IMapView<HSTRING, IInspectable>,
) -> Vec<String> {
    let iterator = match properties.First() {
        Ok(iterator) => iterator,
        Err(error) => {
            warn!(
                logical_code = "property_enumeration_failed",
                error = %winrt_error_description(&error),
                "could not enumerate FLOW 8 DeviceInformation property keys"
            );
            return Vec::new();
        }
    };
    let mut keys = Vec::new();
    loop {
        match iterator.HasCurrent() {
            Ok(false) => break,
            Ok(true) => {}
            Err(error) => {
                warn!(
                    logical_code = "property_enumeration_failed",
                    error = %winrt_error_description(&error),
                    "could not continue FLOW 8 DeviceInformation property-key enumeration"
                );
                break;
            }
        }
        match iterator.Current().and_then(|pair| pair.Key()) {
            Ok(key) => keys.push(key.to_string()),
            Err(error) => warn!(
                logical_code = "property_enumeration_failed",
                error = %winrt_error_description(&error),
                "could not read a FLOW 8 DeviceInformation property key"
            ),
        }
        match iterator.MoveNext() {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                warn!(
                    logical_code = "property_enumeration_failed",
                    error = %winrt_error_description(&error),
                    "could not advance FLOW 8 DeviceInformation property-key enumeration"
                );
                break;
            }
        }
    }
    keys.sort();
    keys
}

fn try_get_string_property(
    properties: &windows_collections::IMapView<HSTRING, IInspectable>,
    key: &str,
) -> Result<Option<String>, BleError> {
    let Some(property) = try_get_property(properties, key)? else {
        return Ok(None);
    };
    let property_type = match property.Type() {
        Ok(property_type) => property_type,
        Err(error) if hresult_succeeded(&error) => {
            warn_optional_property_error(key, "IPropertyValue.Type", &error);
            return Ok(None);
        }
        Err(error) => {
            return Err(windows_error(
                NativeConnectionStage::DeviceFound,
                "IPropertyValue.Type",
                error,
                true,
            ));
        }
    };
    if property_type != PropertyType::String {
        warn!(
            property = key,
            expected_type = ?PropertyType::String,
            actual_type = ?property_type,
            logical_code = "property_type_mismatch",
            "ignoring optional FLOW 8 string identity property with an unexpected type"
        );
        return Ok(None);
    }
    match property.GetString() {
        Ok(value) => Ok(Some(value.to_string())),
        Err(error) if hresult_succeeded(&error) => {
            warn_optional_property_error(key, "IPropertyValue.GetString", &error);
            Ok(None)
        }
        Err(error) => Err(windows_error(
            NativeConnectionStage::DeviceFound,
            "IPropertyValue.GetString",
            error,
            true,
        )),
    }
}

fn try_get_guid_property(
    properties: &windows_collections::IMapView<HSTRING, IInspectable>,
    key: &str,
) -> Result<Option<GUID>, BleError> {
    let Some(property) = try_get_property(properties, key)? else {
        return Ok(None);
    };
    let property_type = match property.Type() {
        Ok(property_type) => property_type,
        Err(error) if hresult_succeeded(&error) => {
            warn_optional_property_error(key, "IPropertyValue.Type", &error);
            return Ok(None);
        }
        Err(error) => {
            return Err(windows_error(
                NativeConnectionStage::DeviceFound,
                "IPropertyValue.Type",
                error,
                true,
            ));
        }
    };
    if property_type != PropertyType::Guid {
        warn!(
            property = key,
            expected_type = ?PropertyType::Guid,
            actual_type = ?property_type,
            logical_code = "property_type_mismatch",
            "ignoring optional FLOW 8 GUID identity property with an unexpected type"
        );
        return Ok(None);
    }
    match property.GetGuid() {
        Ok(value) => Ok(Some(value)),
        Err(error) if hresult_succeeded(&error) => {
            warn_optional_property_error(key, "IPropertyValue.GetGuid", &error);
            Ok(None)
        }
        Err(error) => Err(windows_error(
            NativeConnectionStage::DeviceFound,
            "IPropertyValue.GetGuid",
            error,
            true,
        )),
    }
}

fn try_get_property(
    properties: &windows_collections::IMapView<HSTRING, IInspectable>,
    key: &str,
) -> Result<Option<IPropertyValue>, BleError> {
    let hstring_key = HSTRING::from(key);
    let exists = match properties.HasKey(&hstring_key) {
        Ok(exists) => exists,
        Err(error) if hresult_succeeded(&error) => {
            warn_optional_property_error(key, "DeviceInformation.Properties.HasKey", &error);
            return Ok(None);
        }
        Err(error) => {
            return Err(windows_error(
                NativeConnectionStage::DeviceFound,
                "DeviceInformation.Properties.HasKey",
                error,
                true,
            ));
        }
    };
    if !exists {
        debug!(
            property = key,
            logical_code = "property_missing",
            "optional FLOW 8 DeviceInformation identity property is absent"
        );
        return Ok(None);
    }

    let value = match properties.Lookup(&hstring_key) {
        Ok(value) => value,
        Err(error) if hresult_succeeded(&error) => {
            warn_optional_property_error(key, "DeviceInformation.Properties.Lookup", &error);
            return Ok(None);
        }
        Err(error) => {
            return Err(windows_error(
                NativeConnectionStage::DeviceFound,
                "DeviceInformation.Properties.Lookup",
                error,
                true,
            ));
        }
    };
    match value.cast::<IPropertyValue>() {
        Ok(property) => Ok(Some(property)),
        Err(error) => {
            warn!(
                property = key,
                logical_code = "property_type_mismatch",
                error = %winrt_error_description(&error),
                "optional FLOW 8 identity property is not an IPropertyValue"
            );
            Ok(None)
        }
    }
}

fn warn_optional_property_error(key: &str, api: &str, error: &windows::core::Error) {
    warn!(
        property = key,
        api,
        logical_code = "optional_property_unavailable",
        error = %winrt_error_description(error),
        "optional FLOW 8 identity property could not be read; continuing with remaining identity fields"
    );
}

fn hresult_succeeded(error: &windows::core::Error) -> bool {
    error.code().0 >= 0
}

fn winrt_error_description(error: &windows::core::Error) -> String {
    if hresult_succeeded(error) {
        "WinRT projection returned no optional value despite a successful status".into()
    } else {
        native_error(error)
    }
}

fn enumerate_device_interfaces(class_guid: GUID) -> Result<Vec<NativeDeviceInterface>, BleError> {
    let info_set = DeviceInfoSet(
        unsafe {
            SetupDiGetClassDevsW(
                Some(&class_guid),
                PCWSTR::null(),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        }
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::NativeInterfaceEnumerating,
                "SetupDiGetClassDevsW",
                error,
                true,
            )
        })?,
    );

    let mut interfaces = Vec::new();
    let mut index = 0u32;
    loop {
        let mut interface_data = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        match unsafe {
            SetupDiEnumDeviceInterfaces(info_set.0, None, &class_guid, index, &mut interface_data)
        } {
            Ok(()) => {}
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_ITEMS.0) => break,
            Err(error) => {
                return Err(windows_error(
                    NativeConnectionStage::NativeInterfaceEnumerating,
                    "SetupDiEnumDeviceInterfaces",
                    error,
                    true,
                ));
            }
        }

        let mut required = 0u32;
        let _ = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                info_set.0,
                &interface_data,
                None,
                0,
                Some(&mut required),
                None,
            )
        };
        if required == 0 {
            return Err(native_failure(
                NativeConnectionStage::NativeInterfaceEnumerating,
                "SetupDiGetDeviceInterfaceDetailW(size)",
                true,
                "Windows returned a zero-sized device interface path",
            ));
        }

        let word_count = (required as usize).div_ceil(size_of::<usize>());
        let mut storage = vec![0usize; word_count];
        let detail = storage
            .as_mut_ptr()
            .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: storage is usize-aligned and sized from the required byte
        // count returned by SetupDiGetDeviceInterfaceDetailW.
        let mut device_info = SP_DEVINFO_DATA {
            cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        unsafe {
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            SetupDiGetDeviceInterfaceDetailW(
                info_set.0,
                &interface_data,
                Some(detail),
                required,
                None,
                Some(&mut device_info),
            )
        }
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::NativeInterfaceEnumerating,
                "SetupDiGetDeviceInterfaceDetailW",
                error,
                true,
            )
        })?;

        let path_ptr = unsafe { addr_of!((*detail).DevicePath).cast::<u16>() };
        let path_offset = path_ptr as usize - storage.as_ptr() as usize;
        let max_units = (required as usize - path_offset) / size_of::<u16>();
        let units = unsafe { std::slice::from_raw_parts(path_ptr, max_units) };
        let length = units
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(max_units);
        let path = String::from_utf16_lossy(&units[..length]);
        let instance_id = setupdi_instance_id(&info_set, &device_info)?;
        let container_id =
            setupdi_guid_property(&info_set, &device_info, &DEVPKEY_Device_ContainerId)?;
        let mut parent_chain = devnode_parent_chain(device_info.DevInst);
        if let Some(parent) =
            setupdi_string_property(&info_set, &device_info, &DEVPKEY_Device_Parent)?
            && !parent_chain.iter().any(|entry| pnp_id_eq(entry, &parent))
        {
            parent_chain.insert(0, parent);
        }
        interfaces.push(NativeDeviceInterface {
            path,
            instance_id,
            container_id,
            parent_chain,
        });
        index += 1;
    }
    Ok(interfaces)
}

fn setupdi_instance_id(
    info_set: &DeviceInfoSet,
    device_info: &SP_DEVINFO_DATA,
) -> Result<String, BleError> {
    let mut required = 0u32;
    let _ =
        unsafe { SetupDiGetDeviceInstanceIdW(info_set.0, device_info, None, Some(&mut required)) };
    if required == 0 {
        return Err(native_failure(
            NativeConnectionStage::NativeInterfaceEnumerating,
            "SetupDiGetDeviceInstanceIdW(size)",
            true,
            "Windows returned no PnP device-instance identifier for a GATT interface",
        ));
    }
    let mut buffer = vec![0u16; required as usize];
    unsafe {
        SetupDiGetDeviceInstanceIdW(
            info_set.0,
            device_info,
            Some(&mut buffer),
            Some(&mut required),
        )
    }
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::NativeInterfaceEnumerating,
            "SetupDiGetDeviceInstanceIdW",
            error,
            true,
        )
    })?;
    Ok(wide_buffer_to_string(&buffer))
}

fn setupdi_property_bytes(
    info_set: &DeviceInfoSet,
    device_info: &SP_DEVINFO_DATA,
    key: &windows::Win32::Foundation::DEVPROPKEY,
) -> Result<Option<(DEVPROPTYPE, Vec<u8>)>, BleError> {
    let mut property_type = DEVPROPTYPE::default();
    let mut required = 0u32;
    let _ = unsafe {
        SetupDiGetDevicePropertyW(
            info_set.0,
            device_info,
            key,
            &mut property_type,
            None,
            Some(&mut required),
            0,
        )
    };
    if required == 0 {
        return Ok(None);
    }
    let mut buffer = vec![0u8; required as usize];
    unsafe {
        SetupDiGetDevicePropertyW(
            info_set.0,
            device_info,
            key,
            &mut property_type,
            Some(&mut buffer),
            Some(&mut required),
            0,
        )
    }
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::NativeInterfaceEnumerating,
            "SetupDiGetDevicePropertyW",
            error,
            true,
        )
    })?;
    buffer.truncate(required as usize);
    Ok(Some((property_type, buffer)))
}

fn setupdi_guid_property(
    info_set: &DeviceInfoSet,
    device_info: &SP_DEVINFO_DATA,
    key: &windows::Win32::Foundation::DEVPROPKEY,
) -> Result<Option<GUID>, BleError> {
    let Some((property_type, buffer)) = setupdi_property_bytes(info_set, device_info, key)? else {
        return Ok(None);
    };
    if property_type != DEVPROP_TYPE_GUID || buffer.len() < size_of::<GUID>() {
        return Ok(None);
    }
    // SAFETY: the property buffer contains a DEVPROP_TYPE_GUID value. SetupDi
    // does not promise Rust alignment, so read_unaligned is required.
    Ok(Some(unsafe {
        std::ptr::read_unaligned(buffer.as_ptr().cast::<GUID>())
    }))
}

fn setupdi_string_property(
    info_set: &DeviceInfoSet,
    device_info: &SP_DEVINFO_DATA,
    key: &windows::Win32::Foundation::DEVPROPKEY,
) -> Result<Option<String>, BleError> {
    let Some((property_type, buffer)) = setupdi_property_bytes(info_set, device_info, key)? else {
        return Ok(None);
    };
    if property_type != DEVPROP_TYPE_STRING {
        return Ok(None);
    }
    let units = buffer
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    Ok(Some(wide_buffer_to_string(&units)))
}

fn devnode_parent_chain(mut devinst: u32) -> Vec<String> {
    let mut parents: Vec<String> = Vec::new();
    for _ in 0..16 {
        let mut parent = 0u32;
        if unsafe { CM_Get_Parent(&mut parent, devinst, 0) } != CR_SUCCESS {
            break;
        }
        let mut buffer = vec![0u16; MAX_DEVICE_ID_LEN as usize];
        if unsafe { CM_Get_Device_IDW(parent, &mut buffer, 0) } != CR_SUCCESS {
            break;
        }
        let id = wide_buffer_to_string(&buffer);
        if id.is_empty()
            || parents
                .iter()
                .any(|entry| pnp_id_eq(entry.as_str(), id.as_str()))
        {
            break;
        }
        parents.push(id);
        devinst = parent;
    }
    parents
}

fn wide_buffer_to_string(buffer: &[u16]) -> String {
    let length = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..length])
}

fn open_flow_service_interface(
    flow_identity: &FlowDeviceIdentity,
    events: &mpsc::UnboundedSender<DeviceEvent>,
) -> Result<NativeGattCandidate, BleError> {
    let interfaces = enumerate_device_interfaces(GUID_BLUETOOTH_GATT_SERVICE_DEVICE_INTERFACE)?;
    let total = interfaces.len();
    let interfaces = interfaces
        .into_iter()
        .filter(|interface| belongs_to_flow8_device(interface, flow_identity))
        .collect::<Vec<_>>();
    info!(
        backend = "windows-native-gatt",
        service_interfaces_total = total,
        service_interfaces_matching_flow_device = interfaces.len(),
        "filtered native Bluetooth GATT service interfaces by FLOW 8 PnP identity"
    );

    if interfaces.is_empty() {
        return Err(native_failure(
            NativeConnectionStage::NativeInterfaceEnumerating,
            "Windows PnP FLOW 8 service-interface association",
            true,
            "No GATT service interface belongs to selected FLOW 8 device",
        ));
    }

    let mut candidates = Vec::new();
    let mut last_open_error = None;
    let mut opened_interfaces = 0usize;
    for interface in interfaces {
        trace!(
            interface_path = %interface.path,
            instance_id = %interface.instance_id,
            container_id = ?interface.container_id,
            "examining FLOW 8-owned native GATT service interface"
        );
        let handle = match open_service_handle(&interface.path) {
            Ok(handle) => handle,
            Err(error) => {
                warn!(interface_path = %interface.path, error = %error, "FLOW 8 native GATT interface open failed");
                last_open_error = Some(error);
                continue;
            }
        };
        opened_interfaces += 1;
        report_stage(events, NativeConnectionStage::NativeServiceHandleOpened);

        let services = match native_services(&handle) {
            Ok(services) => services,
            Err(error) => {
                debug!(interface_path = %interface.path, error = %error, "FLOW 8 interface has no readable GATT service list");
                continue;
            }
        };
        for service in services {
            if !is_flow_service(&service) {
                continue;
            }
            let characteristics = native_characteristics(&handle, &service)?;
            report_stage(events, NativeConnectionStage::CharacteristicsEnumerated);
            if let Some(characteristic) = characteristics.into_iter().find(is_flow_characteristic) {
                candidates.push(NativeGattCandidate {
                    path: interface.path.clone(),
                    owner_instance_id: interface.instance_id.clone(),
                    owner_container_id: interface.container_id,
                    handle,
                    service,
                    characteristic,
                });
                break;
            }
        }
    }

    let selected = select_candidate_index(candidates.len()).map_err(|message| {
        native_failure(
            NativeConnectionStage::TargetCharacteristicFound,
            "native FLOW service/characteristic selection",
            true,
            message,
        )
    });

    match selected {
        Ok(index) => {
            report_stage(events, NativeConnectionStage::TargetCharacteristicFound);
            Ok(candidates.swap_remove(index))
        }
        Err(_error)
            if candidates.is_empty() && opened_interfaces == 0 && last_open_error.is_some() =>
        {
            Err(last_open_error.expect("checked above"))
        }
        Err(error) => Err(error),
    }
}

fn select_candidate_index(candidate_count: usize) -> Result<usize, String> {
    match candidate_count {
        0 => Err(
            "no native service interface contained both the FLOW service and target characteristic"
                .into(),
        ),
        1 => Ok(0),
        many => Err(format!(
            "{many} FLOW-owned service interfaces exposed the same FLOW service and characteristic"
        )),
    }
}

fn belongs_to_flow8_device(
    interface: &NativeDeviceInterface,
    flow_identity: &FlowDeviceIdentity,
) -> bool {
    // Container identity is the most reliable cross-kind association between
    // WinRT DeviceInformation and SetupDi service-interface devnodes.
    let container_matches = interface
        .container_id
        .zip(flow_identity.container_id)
        .is_some_and(|(left, right)| left == right);
    if container_matches {
        return true;
    }

    // A DeviceInterface DeviceInformation may expose the parent Device's
    // DeviceInstanceId. Match that exact identity either on the service
    // devnode itself or anywhere in its ConfigMgr parent chain. Parent alone
    // is deliberately not used: it may identify a shared Bluetooth radio and
    // is only retained for diagnostics.
    let Some(device_instance_id) = flow_identity.device_instance_id.as_deref() else {
        return false;
    };
    pnp_id_eq(&interface.instance_id, device_instance_id)
        || interface
            .parent_chain
            .iter()
            .any(|ancestor| pnp_id_eq(ancestor, device_instance_id))
}

fn pnp_id_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn open_service_handle(path: &str) -> Result<ServiceHandle, BleError> {
    let wide = HSTRING::from(path);
    let handle = unsafe {
        CreateFileW(
            &wide,
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::NativeServiceHandleOpened,
            "CreateFileW(FLOW GATT service interface)",
            error,
            true,
        )
    })?;
    Ok(ServiceHandle(handle))
}

fn native_services(handle: &ServiceHandle) -> Result<Vec<BTH_LE_GATT_SERVICE>, BleError> {
    let mut count = 0u16;
    let first =
        unsafe { BluetoothGATTGetServices(handle.0, None, &mut count, BLUETOOTH_GATT_FLAG_NONE) };
    if count == 0 {
        return Err(first
            .err()
            .map(|error| {
                windows_error(
                    NativeConnectionStage::NativeServiceHandleOpened,
                    "BluetoothGATTGetServices(size)",
                    error,
                    true,
                )
            })
            .unwrap_or_else(|| {
                native_failure(
                    NativeConnectionStage::NativeServiceHandleOpened,
                    "BluetoothGATTGetServices(size)",
                    true,
                    "native service interface returned no GATT services",
                )
            }));
    }

    let mut services = vec![BTH_LE_GATT_SERVICE::default(); count as usize];
    let mut actual = count;
    unsafe {
        BluetoothGATTGetServices(
            handle.0,
            Some(&mut services),
            &mut actual,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    }
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::NativeServiceHandleOpened,
            "BluetoothGATTGetServices",
            error,
            true,
        )
    })?;
    services.truncate(actual as usize);
    Ok(services)
}

fn native_characteristics(
    handle: &ServiceHandle,
    service: &BTH_LE_GATT_SERVICE,
) -> Result<Vec<BTH_LE_GATT_CHARACTERISTIC>, BleError> {
    let mut count = 0u16;
    let first = unsafe {
        BluetoothGATTGetCharacteristics(
            handle.0,
            Some(service),
            None,
            &mut count,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    };
    if count == 0 {
        return Err(first
            .err()
            .map(|error| {
                windows_error(
                    NativeConnectionStage::CharacteristicsEnumerated,
                    "BluetoothGATTGetCharacteristics(size)",
                    error,
                    true,
                )
            })
            .unwrap_or_else(|| {
                native_failure(
                    NativeConnectionStage::CharacteristicsEnumerated,
                    "BluetoothGATTGetCharacteristics(size)",
                    true,
                    "FLOW service returned no characteristics",
                )
            }));
    }

    let mut characteristics = vec![BTH_LE_GATT_CHARACTERISTIC::default(); count as usize];
    let mut actual = count;
    unsafe {
        BluetoothGATTGetCharacteristics(
            handle.0,
            Some(service),
            Some(&mut characteristics),
            &mut actual,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    }
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::CharacteristicsEnumerated,
            "BluetoothGATTGetCharacteristics",
            error,
            true,
        )
    })?;
    characteristics.truncate(actual as usize);
    Ok(characteristics)
}

fn bth_uuid_eq(value: &BTH_LE_UUID, target: GUID) -> bool {
    if value.IsShortUuid {
        let short = unsafe { value.Value.ShortUuid } as u128;
        let expanded = GUID::from_u128((short << 96) | 0x0000_0000_0000_1000_8000_0080_5f9b_34fb);
        expanded == target
    } else {
        unsafe { value.Value.LongUuid == target }
    }
}

fn is_flow_service(service: &BTH_LE_GATT_SERVICE) -> bool {
    bth_uuid_eq(&service.ServiceUuid, FLOW_SERVICE_GUID)
}

fn is_flow_characteristic(characteristic: &BTH_LE_GATT_CHARACTERISTIC) -> bool {
    bth_uuid_eq(&characteristic.CharacteristicUuid, FLOW_CHARACTERISTIC_GUID)
}

fn report_stage(events: &mpsc::UnboundedSender<DeviceEvent>, stage: NativeConnectionStage) {
    info!(
        backend = "windows-native-gatt",
        stage = stage.as_str(),
        "FLOW 8 connection stage"
    );
    let _ = events.send(DeviceEvent::ConnectionStage(stage));
}

fn windows_error(
    stage: NativeConnectionStage,
    api: &'static str,
    error: windows::core::Error,
    retryable: bool,
) -> BleError {
    if hresult_succeeded(&error) {
        return logical_failure(
            stage,
            api,
            "winrt_projection_error_without_failing_hresult",
            retryable,
            "Windows projection returned Err without a failing HRESULT; no HRESULT was recorded",
        );
    }
    NativeConnectionError {
        stage,
        api,
        logical_code: None,
        code: Some(error.code().0 as u32),
        retryable,
        message: error.message(),
    }
    .into()
}

fn native_failure(
    stage: NativeConnectionStage,
    api: &'static str,
    retryable: bool,
    message: impl Into<String>,
) -> BleError {
    NativeConnectionError {
        stage,
        api,
        logical_code: None,
        code: None,
        retryable,
        message: message.into(),
    }
    .into()
}

fn logical_failure(
    stage: NativeConnectionStage,
    api: &'static str,
    logical_code: &'static str,
    retryable: bool,
    message: impl Into<String>,
) -> BleError {
    NativeConnectionError {
        stage,
        api,
        logical_code: Some(logical_code),
        code: None,
        retryable,
        message: message.into(),
    }
    .into()
}

fn native_error(error: &windows::core::Error) -> String {
    format!("{} (HRESULT 0x{:08X})", error, error.code().0 as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_selection_never_depends_on_interface_order() {
        assert_eq!(select_candidate_index(1), Ok(0));
        assert!(select_candidate_index(0).is_err());
        assert!(select_candidate_index(2).is_err());
    }

    #[test]
    fn service_interface_matches_selected_flow_container() {
        let container = GUID::from_u128(0x12345678_1234_5678_9abc_def012345678);
        let flow = identity("BTHLE\\FLOW", Some(container), None);
        let interface = native_interface("SWD\\GATTSERVICE\\ONE", Some(container), &[]);
        assert!(belongs_to_flow8_device(&interface, &flow));
    }

    #[test]
    fn service_interface_matches_selected_flow_parent_chain() {
        let flow = identity("BTHLE\\FLOW", None, Some("BTHENUM\\RADIO"));
        let interface = native_interface(
            "SWD\\GATTSERVICE\\ONE",
            None,
            &["BTHLE\\FLOW", "BTHENUM\\RADIO"],
        );
        assert!(belongs_to_flow8_device(&interface, &flow));
    }

    #[test]
    fn partial_flow_identity_is_sufficient_with_container_only() {
        let flow = FlowDeviceIdentity {
            device_instance_id: None,
            container_id: Some(GUID::from_u128(1)),
            parent_id: None,
        };
        assert!(flow.is_sufficient_for_matching());
        assert!(belongs_to_flow8_device(
            &native_interface("SWD\\GATTSERVICE\\ONE", Some(GUID::from_u128(1)), &[]),
            &flow
        ));
    }

    #[test]
    fn partial_flow_identity_is_sufficient_with_device_instance_only() {
        let flow = identity("BTHLE\\FLOW", None, None);
        assert!(flow.is_sufficient_for_matching());
        assert!(belongs_to_flow8_device(
            &native_interface("BTHLE\\FLOW", None, &[]),
            &flow
        ));
    }

    #[test]
    fn parent_property_alone_is_not_a_reliable_device_identity() {
        let flow = FlowDeviceIdentity {
            device_instance_id: None,
            container_id: None,
            parent_id: Some("BTHENUM\\SHARED-RADIO".into()),
        };
        let interface = native_interface(
            "SWD\\GATTSERVICE\\OTHER-DEVICE",
            None,
            &["BTHENUM\\SHARED-RADIO"],
        );
        assert!(!flow.is_sufficient_for_matching());
        assert!(!belongs_to_flow8_device(&interface, &flow));
    }

    #[test]
    fn completely_missing_flow_identity_is_insufficient() {
        let flow = FlowDeviceIdentity {
            device_instance_id: None,
            container_id: None,
            parent_id: None,
        };
        assert!(!flow.is_sufficient_for_matching());
    }

    #[test]
    fn success_hresult_is_never_reported_as_a_native_failure_code() {
        let error = windows_error(
            NativeConnectionStage::DeviceFound,
            "DeviceInformation.Properties.Lookup",
            windows::core::Error::from_hresult(HRESULT(0)),
            true,
        );
        let BleError::NativeWindows(error) = error else {
            panic!("expected a structured native Windows error");
        };
        assert_eq!(error.code, None);
        assert_eq!(
            error.logical_code,
            Some("winrt_projection_error_without_failing_hresult")
        );
        assert!(!error.to_string().contains("code=0x00000000"));
    }

    #[test]
    fn unrelated_ble_service_interface_is_ignored_even_if_uuid_might_match() {
        let flow = identity(
            "BTHLE\\FLOW",
            Some(GUID::from_u128(1)),
            Some("BTHENUM\\FLOW-PARENT"),
        );
        let unrelated = native_interface(
            "SWD\\GATTSERVICE\\SAME-UUID-OTHER-DEVICE",
            Some(GUID::from_u128(2)),
            &["BTHLE\\OTHER", "BTHENUM\\OTHER-PARENT"],
        );
        assert!(!belongs_to_flow8_device(&unrelated, &flow));
    }

    #[test]
    fn pnp_matching_is_exact_and_case_insensitive_not_substring_based() {
        let flow = identity("BTHLE\\FLOW", None, None);
        let exact = native_interface("SWD\\SERVICE", None, &["bthle\\flow"]);
        let substring = native_interface("SWD\\SERVICE", None, &["BTHLE\\FLOW-OTHER"]);
        assert!(belongs_to_flow8_device(&exact, &flow));
        assert!(!belongs_to_flow8_device(&substring, &flow));
    }

    #[test]
    fn native_filter_requires_both_flow_service_and_target_characteristic() {
        let mut service = BTH_LE_GATT_SERVICE::default();
        service.ServiceUuid.IsShortUuid = false;
        service.ServiceUuid.Value.LongUuid = FLOW_SERVICE_GUID;
        assert!(is_flow_service(&service));

        let mut characteristic = BTH_LE_GATT_CHARACTERISTIC::default();
        characteristic.CharacteristicUuid.IsShortUuid = false;
        characteristic.CharacteristicUuid.Value.LongUuid = FLOW_CHARACTERISTIC_GUID;
        assert!(is_flow_characteristic(&characteristic));

        service.ServiceUuid.Value.LongUuid = GUID::from_u128(1);
        characteristic.CharacteristicUuid.Value.LongUuid = GUID::from_u128(2);
        assert!(!is_flow_service(&service));
        assert!(!is_flow_characteristic(&characteristic));
    }

    fn identity(
        instance_id: &str,
        container_id: Option<GUID>,
        parent_id: Option<&str>,
    ) -> FlowDeviceIdentity {
        FlowDeviceIdentity {
            device_instance_id: Some(instance_id.into()),
            container_id,
            parent_id: parent_id.map(str::to_owned),
        }
    }

    fn native_interface(
        instance_id: &str,
        container_id: Option<GUID>,
        parent_chain: &[&str],
    ) -> NativeDeviceInterface {
        NativeDeviceInterface {
            path: "native-path".into(),
            instance_id: instance_id.into(),
            container_id,
            parent_chain: parent_chain.iter().map(|value| (*value).into()).collect(),
        }
    }
}
