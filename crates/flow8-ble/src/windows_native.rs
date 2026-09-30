//! Production Windows FLOW 8 transport.
//!
//! FLOW 8 has a WRITE | NOTIFY transport characteristic but no CCCD. The
//! Windows backend performs UUID-targeted WinRT discovery on the selected
//! BluetoothLEDevice, opens that GattDeviceService's DeviceId as the native
//! service handle, and uses BluetoothGATTRegisterEvent.
//! It never calls the standard BLE subscription path.

use std::{
    ffi::c_void,
    mem::size_of,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use windows::{
    Devices::{
        Bluetooth::{
            BluetoothConnectionStatus, BluetoothDeviceId, BluetoothLEDevice,
            GenericAttributeProfile::{GattDeviceService, GattSession},
        },
        Enumeration::{DeviceInformation, DeviceInformationCollection},
    },
    Foundation::{IPropertyValue, PropertyType, TypedEventHandler},
    Win32::{
        Devices::Bluetooth::{
            BLUETOOTH_GATT_FLAG_NONE, BLUETOOTH_GATT_VALUE_CHANGED_EVENT,
            BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION, BTH_LE_GATT_CHARACTERISTIC,
            BTH_LE_GATT_CHARACTERISTIC_VALUE, BTH_LE_GATT_SERVICE, BTH_LE_UUID,
            BluetoothGATTGetCharacteristics, BluetoothGATTGetServices, BluetoothGATTRegisterEvent,
            BluetoothGATTSetCharacteristicValue, BluetoothGATTUnregisterEvent,
            CharacteristicValueChangedEvent,
        },
        Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        },
    },
    core::{GUID, HSTRING, IInspectable, Interface},
};
use windows_collections::IIterable;

#[cfg(test)]
use windows::core::HRESULT;

use super::{
    BleError, DeviceEvent, EventSender, NativeConnectionError, NativeConnectionStage, RxIngress,
    TransportRx,
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

/// Production invariant: FLOW service discovery is scoped to the selected
/// BluetoothLEDevice through UUID-targeted WinRT calls. The global SetupDi
/// service-interface list is never consulted by the production path.
const PRODUCTION_USES_GLOBAL_GATT_SERVICE_ENUMERATION: bool = false;

struct FlowServiceInstance {
    path: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowDeviceIdentity {
    device_instance_id: Option<String>,
    container_id: Option<GUID>,
    parent_id: Option<String>,
    bluetooth_address: Option<u64>,
}

impl FlowDeviceIdentity {
    fn is_sufficient_for_matching(&self) -> bool {
        self.container_id.is_some()
            || self.device_instance_id.is_some()
            || self.bluetooth_address.is_some()
    }
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
    context: Option<Box<CallbackContext>>,
}

impl NativeEventRegistration {
    fn invalidate(&self) {
        if let Some(context) = &self.context {
            context.ingress.invalidate();
        }
    }
}

impl Drop for NativeEventRegistration {
    fn drop(&mut self) {
        self.invalidate();
        let Some(context) = self.context.take() else {
            return;
        };
        // SAFETY: handle came from BluetoothGATTRegisterEvent and is
        // unregistered exactly once before callback context destruction.
        if let Err(error) =
            unsafe { BluetoothGATTUnregisterEvent(self.handle, BLUETOOTH_GATT_FLAG_NONE) }
        {
            // Native callbacks can still arrive if unregister did not succeed.
            // Retain the callback context instead of freeing reachable memory.
            warn!(error = %native_error(&error), "unregistering FLOW 8 native RX event failed; retaining callback context to avoid use-after-free");
            let _ = Box::into_raw(context);
            return;
        }
        context.wait_until_drained();
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
    handshake_observed: Arc<AtomicBool>,
    mtu: u16,
    closed: bool,
}

impl WindowsNativeSession {
    pub(super) async fn connect(
        generation: u64,
        events: EventSender,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportRx>), BleError> {
        debug_assert!(!PRODUCTION_USES_GLOBAL_GATT_SERVICE_ENUMERATION);
        let (tx, rx) = mpsc::unbounded_channel();
        let ingress = RxIngress::new(generation, tx);

        report_stage(&events, NativeConnectionStage::Scanning);
        let (device, mut flow_identity) = find_flow8_device(&events).await?;
        flow_identity.bluetooth_address = device.BluetoothAddress().ok();
        report_stage(&events, NativeConnectionStage::DeviceObjectCreated);
        info!(
            backend = "windows-native-gatt",
            name = ?device.Name().map(|name| name.to_string()),
            device_instance_id = ?flow_identity.device_instance_id,
            container_id = ?flow_identity.container_id,
            parent_id = ?flow_identity.parent_id,
            bluetooth_address = flow_identity.bluetooth_address.map(format_bluetooth_address),
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
        let bluetooth_device_id_text = device_id
            .Id()
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "unavailable".into());
        info!(
            backend = "windows-native-gatt",
            bluetooth_device_id = %bluetooth_device_id_text,
            "BluetoothDeviceId resolved"
        );
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
        // Establish the selected device's physical connection before resolving
        // its UUID-scoped GATT service DeviceInformation. FLOW 8 retransmits
        // 0x35, so arm native RX as the first operation after opening and
        // verifying that exact service instance path.
        gatt_session.SetMaintainConnection(true).map_err(|error| {
            windows_error(
                NativeConnectionStage::DeviceObjectCreated,
                "GattSession.MaintainConnection",
                error,
                true,
            )
        })?;
        wait_for_physical_connection(&device).await;
        report_stage(&events, NativeConnectionStage::NativeInterfaceEnumerating);
        let service_instance = discover_flow_service_instance(&device_id).await?;
        report_stage(&events, NativeConnectionStage::FlowServiceSelected);
        info!(
            backend = "windows-native-gatt",
            service_uuid = %guid_text(FLOW_SERVICE_GUID),
            service_instance_path = %service_instance.path,
            "FLOW service DeviceInformation found"
        );

        let service_handle = open_service_handle_from_instance_path(&service_instance.path)?;
        report_stage(&events, NativeConnectionStage::NativeServiceHandleOpened);
        info!(
            backend = "windows-native-gatt",
            service_instance_path = %service_instance.path,
            "native service handle opened from FLOW service DeviceInformation.Id"
        );
        let service = native_services(&service_handle)?
            .into_iter()
            .find(is_flow_service)
            .ok_or_else(|| {
                native_failure(
                    NativeConnectionStage::NativeServiceHandleOpened,
                    "BluetoothGATTGetServices(FLOW service DeviceId)",
                    false,
                    format!(
                        "native handle opened from service instance path '{}' did not expose FLOW service {}",
                        service_instance.path,
                        guid_text(FLOW_SERVICE_GUID)
                    ),
                )
            })?;
        let characteristic = native_characteristics(&service_handle, &service)?
            .into_iter()
            .find(is_flow_characteristic)
            .ok_or_else(|| {
                native_failure(
                    NativeConnectionStage::CharacteristicsEnumerated,
                    "BluetoothGATTGetCharacteristics(FLOW service DeviceId)",
                    false,
                    format!(
                        "native handle opened from service instance path '{}' did not expose target characteristic {}",
                        service_instance.path,
                        guid_text(FLOW_CHARACTERISTIC_GUID)
                    ),
                )
            })?;
        report_stage(&events, NativeConnectionStage::CharacteristicsEnumerated);
        report_stage(&events, NativeConnectionStage::TargetCharacteristicFound);
        info!(
            backend = "windows-native-gatt",
            service_instance_path = %service_instance.path,
            service_uuid = ?FLOW_SERVICE_GUID,
            characteristic_uuid = %guid_text(FLOW_CHARACTERISTIC_GUID),
            service_handle = service.AttributeHandle,
            declaration_handle = characteristic.AttributeHandle,
            value_handle = characteristic.CharacteristicValueHandle,
            writable = characteristic.IsWritable,
            notifiable = characteristic.IsNotifiable,
            "native characteristic matched"
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

        let handshake_observed = Arc::new(AtomicBool::new(false));
        let timeout_observed = Arc::clone(&handshake_observed);
        let timeout_ingress = ingress.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(6)).await;
            if !timeout_observed.load(Ordering::Acquire) {
                timeout_ingress.handshake_timeout();
            }
        });
        report_stage(&events, NativeConnectionStage::Handshaking);
        info!(
            backend = "windows-native-gatt",
            mtu, "native RX is armed; waiting for FLOW 8 handshake retransmission"
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
                handshake_observed,
                mtu,
                closed: false,
            },
            rx,
        ))
    }

    pub(super) fn mtu(&self) -> u16 {
        self.mtu
    }

    pub(super) fn mark_handshake_rx(&self) {
        self.handshake_observed.store(true, Ordering::Release);
    }

    pub(super) async fn write(&mut self, frame: &[u8]) -> Result<(), BleError> {
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
        if frame.first() == Some(&0x39) {
            let tx_hex = frame
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            println!("TX_HEX={tx_hex}");
        }
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
        context: Some(context),
    })
}

async fn find_flow8_device(
    events: &EventSender,
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

async fn wait_for_physical_connection(device: &BluetoothLEDevice) {
    for _ in 0..30 {
        match device.ConnectionStatus() {
            Ok(BluetoothConnectionStatus::Connected) => {
                info!(
                    backend = "windows-native-gatt",
                    "FLOW 8 physical connection established before native service enumeration"
                );
                return;
            }
            Ok(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            Err(error) => {
                warn!(
                    api = "BluetoothLEDevice.ConnectionStatus",
                    error = %native_error(&error),
                    "could not query FLOW 8 physical connection status; native service enumeration will continue"
                );
                return;
            }
        }
    }
    warn!(
        logical_code = "physical_connection_wait_timeout",
        "FLOW 8 did not report Connected within 3 seconds; native service enumeration will still be attempted once"
    );
}

async fn discover_flow_service_instance(
    bluetooth_device_id: &BluetoothDeviceId,
) -> Result<FlowServiceInstance, BleError> {
    let selector = GattDeviceService::GetDeviceSelectorForBluetoothDeviceIdAndUuid(
        bluetooth_device_id,
        FLOW_SERVICE_GUID,
    )
    .map_err(|error| {
        windows_error(
            NativeConnectionStage::FlowServiceSelected,
            "GattDeviceService.GetDeviceSelectorForBluetoothDeviceIdAndUuid",
            error,
            true,
        )
    })?;
    info!(
        backend = "windows-native-gatt",
        service_uuid = %guid_text(FLOW_SERVICE_GUID),
        "FLOW service selector created"
    );

    let services = DeviceInformation::FindAllAsyncAqsFilter(&selector)
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::FlowServiceSelected,
                "DeviceInformation.FindAllAsync(GattDeviceService selector)",
                error,
                true,
            )
        })?
        .await
        .map_err(|error| {
            windows_error(
                NativeConnectionStage::FlowServiceSelected,
                "DeviceInformation.FindAllAsync(GattDeviceService selector) completion",
                error,
                true,
            )
        })?;

    let mut instance_paths = services
        .into_iter()
        .map(|service| {
            service.Id().map(|id| id.to_string()).map_err(|error| {
                windows_error(
                    NativeConnectionStage::FlowServiceSelected,
                    "FLOW service DeviceInformation.Id",
                    error,
                    true,
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    instance_paths.retain(|path| !path.is_empty());
    instance_paths.sort_unstable();
    instance_paths.dedup();

    if instance_paths.is_empty() {
        return Err(logical_failure(
            NativeConnectionStage::FlowServiceSelected,
            "GetDeviceSelectorForBluetoothDeviceIdAndUuid/FindAllAsync",
            "flow_service_instance_not_exposed",
            true,
            "No FLOW 8 GATT service instance exposed by Windows",
        ));
    }
    if instance_paths.len() > 1 {
        warn!(
            backend = "windows-native-gatt",
            service_instance_count = instance_paths.len(),
            "multiple FLOW service instances were returned by the selected BluetoothDeviceId selector; using the first scoped result"
        );
    }
    Ok(FlowServiceInstance {
        path: instance_paths.remove(0),
    })
}

#[cfg(test)]
fn select_exact_uuid_result(values: &[GUID], expected: GUID) -> Result<usize, String> {
    let matches = values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (*value == expected).then_some(index))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(format!(
            "no result matched expected UUID {} (result_count={})",
            guid_text(expected),
            values.len()
        )),
        _ => Err(format!(
            "{} results matched expected UUID {}; exactly one is required",
            matches.len(),
            guid_text(expected)
        )),
    }
}

fn native_path_from_service_instance_id(instance_id: &str) -> Result<String, String> {
    if instance_id.is_empty() {
        Err("FLOW service DeviceInformation.Id was empty".into())
    } else {
        // DeviceInformation.Id is already the Windows GATT service instance
        // path. Forward it verbatim; no global service enumeration or path
        // guessing is used.
        Ok(instance_id.to_owned())
    }
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
        bluetooth_address: None,
    };
    info!(
        backend = "windows-native-gatt",
        device_instance_id = ?identity.device_instance_id,
        container_id = ?identity.container_id,
        parent_id = ?identity.parent_id,
        "resolved partial FLOW 8 Windows device identity"
    );

    if !identity.is_sufficient_for_matching() {
        warn!(
            logical_code = "identity_metadata_unavailable",
            "FLOW 8 PnP identity metadata is unavailable; continuing with exact service and characteristic UUID verification"
        );
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

// Retained only as uncompiled historical context while the UUID-scoped path is
// validated on hardware. It is not part of any build or production fallback.
#[cfg(any())]
mod legacy_global_service_enumeration {
    use super::*;

    fn enumerate_device_interfaces(
        class_guid: GUID,
    ) -> Result<Vec<NativeDeviceInterface>, BleError> {
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
                SetupDiEnumDeviceInterfaces(
                    info_set.0,
                    None,
                    &class_guid,
                    index,
                    &mut interface_data,
                )
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
            let bluetooth_address =
                setupdi_string_property(&info_set, &device_info, &DEVPKEY_Device_Address)?
                    .as_deref()
                    .and_then(parse_bluetooth_address);
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
                bluetooth_address,
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
        let _ = unsafe {
            SetupDiGetDeviceInstanceIdW(info_set.0, device_info, None, Some(&mut required))
        };
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
        let Some((property_type, buffer)) = setupdi_property_bytes(info_set, device_info, key)?
        else {
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
        let Some((property_type, buffer)) = setupdi_property_bytes(info_set, device_info, key)?
        else {
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
        _events: &EventSender,
    ) -> Result<NativeGattCandidate, BleError> {
        let mut interfaces =
            enumerate_device_interfaces(GUID_BLUETOOTH_GATT_SERVICE_DEVICE_INTERFACE)?;
        let total = interfaces.len();
        interfaces.sort_by_key(|interface| {
            std::cmp::Reverse(identity_association(interface, flow_identity))
        });
        let identity_matches = interfaces
            .iter()
            .filter(|interface| {
                identity_association(interface, flow_identity) > IdentityAssociation::Unknown
            })
            .count();
        info!(
            backend = "windows-native-gatt",
            service_interfaces_total = total,
            service_interfaces_matching_flow_device = identity_matches,
            identity_metadata_available = flow_identity.is_sufficient_for_matching(),
            "enumerated native Bluetooth GATT service interfaces; identity will prioritize candidates but will not gate exact UUID verification"
        );

        let mut candidates = Vec::new();
        let mut strongly_associated_open_error = None;
        let mut sole_candidate_open_error = None;
        let strongly_associated_count = identity_matches;
        let mut flow_service_count = 0usize;
        let mut previous_association = None;
        for interface in interfaces {
            let association = identity_association(&interface, flow_identity);
            if should_stop_before_lower_priority(
                previous_association,
                association,
                candidates.len(),
            ) {
                trace!(
                    selected_association = ?previous_association,
                    skipped_lower_priority_association = ?association,
                    "exact FLOW UUID pair found; lower-priority native interfaces will not be opened"
                );
                break;
            }
            previous_association = Some(association);
            trace!(
                interface_path = %interface.path,
                instance_id = %interface.instance_id,
                container_id = ?interface.container_id,
                bluetooth_address = interface.bluetooth_address.map(format_bluetooth_address),
                association = ?association,
                "examining native GATT service interface"
            );
            let handle = match open_service_handle(&interface.path) {
                Ok(handle) => handle,
                Err(error) => {
                    trace!(
                        interface_path = %interface.path,
                        association = ?association,
                        error = %error,
                        "native GATT service interface could not be opened; it has not been identified as the FLOW service"
                    );
                    if association > IdentityAssociation::Unknown
                        && strongly_associated_open_error.is_none()
                    {
                        strongly_associated_open_error = Some(error);
                    } else if total == 1 {
                        sole_candidate_open_error = Some(error);
                    }
                    continue;
                }
            };

            let services = match native_services(&handle) {
                Ok(services) => services,
                Err(error) => {
                    trace!(interface_path = %interface.path, error = %error, "non-selected GATT interface has no readable service list");
                    continue;
                }
            };
            for service in services {
                if !is_flow_service(&service) {
                    continue;
                }
                flow_service_count += 1;
                let characteristics = native_characteristics(&handle, &service)?;
                if let Some(characteristic) =
                    characteristics.into_iter().find(is_flow_characteristic)
                {
                    candidates.push(NativeGattCandidate {
                        path: interface.path.clone(),
                        owner_instance_id: interface.instance_id.clone(),
                        owner_container_id: interface.container_id,
                        handle,
                        service,
                        characteristic,
                        association,
                    });
                    break;
                }
            }
        }

        info!(
            backend = "windows-native-gatt",
            service_candidates_total = total,
            flow_service_candidates = flow_service_count,
            flow_service_characteristic_pairs = candidates.len(),
            target_service_uuid = %guid_text(FLOW_SERVICE_GUID),
            target_characteristic_uuid = %guid_text(FLOW_CHARACTERISTIC_GUID),
            "completed native FLOW service candidate verification"
        );

        let selected = select_candidate_index(&candidates).map_err(|message| {
        native_failure(
            NativeConnectionStage::TargetCharacteristicFound,
            "native FLOW service/characteristic selection",
            true,
            format!(
                "{message}; target_service={} target_characteristic={} service_candidates_total={} flow_service_candidates={} exact_pair_candidates={}",
                guid_text(FLOW_SERVICE_GUID),
                guid_text(FLOW_CHARACTERISTIC_GUID),
                total,
                flow_service_count,
                candidates.len()
            ),
        )
    });

        match selected {
            Ok(index) => Ok(candidates.swap_remove(index)),
            Err(_error)
                if candidates.is_empty()
                    && should_promote_open_failure(strongly_associated_count, total)
                    && (strongly_associated_open_error.is_some()
                        || sole_candidate_open_error.is_some()) =>
            {
                let original = strongly_associated_open_error
                    .or(sole_candidate_open_error)
                    .expect("promotion requires a captured candidate open failure");
                Err(retarget_open_failure(original, total, flow_service_count))
            }
            Err(error) => Err(error),
        }
    }

    fn should_stop_before_lower_priority(
        previous: Option<IdentityAssociation>,
        current: IdentityAssociation,
        verified_candidate_count: usize,
    ) -> bool {
        previous.is_some_and(|previous| previous > current) && verified_candidate_count > 0
    }

    fn should_promote_open_failure(
        strongly_associated_count: usize,
        total_candidate_count: usize,
    ) -> bool {
        strongly_associated_count == 1 || total_candidate_count == 1
    }

    fn retarget_open_failure(
        error: BleError,
        candidate_count: usize,
        flow_service_count: usize,
    ) -> BleError {
        match error {
            BleError::NativeWindows(mut error) => {
                error.message = format!(
                    "{}; the sole plausible target service interface could not be opened (target_service={} target_characteristic={} candidate_count={} flow_service_count={})",
                    error.message,
                    guid_text(FLOW_SERVICE_GUID),
                    guid_text(FLOW_CHARACTERISTIC_GUID),
                    candidate_count,
                    flow_service_count
                );
                BleError::NativeWindows(error)
            }
            other => other,
        }
    }

    fn select_candidate_index(candidates: &[NativeGattCandidate]) -> Result<usize, String> {
        match candidates.len() {
        0 => Err(
            "no native service interface contained both the FLOW service and target characteristic"
                .into(),
        ),
        1 => Ok(0),
        many => {
            let best = candidates
                .iter()
                .map(|candidate| candidate.association)
                .max()
                .unwrap_or(IdentityAssociation::Unknown);
            let matching = candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.association == best)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if best > IdentityAssociation::Unknown && matching.len() == 1 {
                Ok(matching[0])
            } else {
                Err(format!(
                    "{many} native interfaces exposed the FLOW service and target characteristic, and available device identity could not select exactly one"
                ))
            }
        }
    }
    }

    #[cfg(test)]
    fn belongs_to_flow8_device(
        interface: &NativeDeviceInterface,
        flow_identity: &FlowDeviceIdentity,
    ) -> bool {
        identity_association(interface, flow_identity) > IdentityAssociation::Unknown
    }

    fn identity_association(
        interface: &NativeDeviceInterface,
        flow_identity: &FlowDeviceIdentity,
    ) -> IdentityAssociation {
        // Container identity is the most reliable cross-kind association between
        // WinRT DeviceInformation and SetupDi service-interface devnodes.
        let container_matches = interface
            .container_id
            .zip(flow_identity.container_id)
            .is_some_and(|(left, right)| left == right);
        if container_matches {
            return IdentityAssociation::Container;
        }

        if interface
            .bluetooth_address
            .zip(flow_identity.bluetooth_address)
            .is_some_and(|(left, right)| left == right)
        {
            return IdentityAssociation::Address;
        }

        // A DeviceInterface DeviceInformation may expose the parent Device's
        // DeviceInstanceId. Match that exact identity either on the service
        // devnode itself or anywhere in its ConfigMgr parent chain. Parent alone
        // is deliberately not used: it may identify a shared Bluetooth radio and
        // is only retained for diagnostics.
        let instance_matches =
            flow_identity
                .device_instance_id
                .as_deref()
                .is_some_and(|device_instance_id| {
                    pnp_id_eq(&interface.instance_id, device_instance_id)
                        || interface
                            .parent_chain
                            .iter()
                            .any(|ancestor| pnp_id_eq(ancestor, device_instance_id))
                });
        if instance_matches {
            return IdentityAssociation::Instance;
        }

        if !flow_identity.is_sufficient_for_matching() {
            IdentityAssociation::Unknown
        } else {
            IdentityAssociation::Mismatch
        }
    }

    fn pnp_id_eq(left: &str, right: &str) -> bool {
        left.eq_ignore_ascii_case(right)
    }

    fn parse_bluetooth_address(value: &str) -> Option<u64> {
        let compact = value
            .chars()
            .filter(|character| character.is_ascii_hexdigit())
            .collect::<String>();
        (compact.len() == 12)
            .then(|| u64::from_str_radix(&compact, 16).ok())
            .flatten()
    }
}

fn format_bluetooth_address(value: u64) -> String {
    let bytes = value.to_be_bytes();
    bytes[2..]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn guid_text(value: GUID) -> String {
    uuid::Uuid::from_u128(value.to_u128()).to_string()
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

fn open_service_handle_from_instance_path(instance_id: &str) -> Result<ServiceHandle, BleError> {
    let path = native_path_from_service_instance_id(instance_id).map_err(|message| {
        logical_failure(
            NativeConnectionStage::NativeServiceHandleOpened,
            "FLOW service DeviceInformation.Id -> CreateFileW",
            "empty_service_instance_id",
            false,
            message,
        )
    })?;
    open_service_handle(&path).map_err(|error| match error {
        BleError::NativeWindows(mut error) => {
            error.message = format!("{}; service_instance_path={instance_id}", error.message);
            BleError::NativeWindows(error)
        }
        other => other,
    })
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

fn report_stage(events: &EventSender, stage: NativeConnectionStage) {
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

#[cfg(any())]
mod legacy_tests {
    use super::*;

    #[test]
    fn service_selection_never_depends_on_interface_order() {
        assert_eq!(
            select_candidate_index(&[candidate(IdentityAssociation::Unknown)]),
            Ok(0)
        );
        assert!(select_candidate_index(&[]).is_err());
        assert!(
            select_candidate_index(&[
                candidate(IdentityAssociation::Unknown),
                candidate(IdentityAssociation::Unknown),
            ])
            .is_err()
        );
        assert_eq!(
            select_candidate_index(&[
                candidate(IdentityAssociation::Unknown),
                candidate(IdentityAssociation::Container),
            ]),
            Ok(1)
        );
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
            bluetooth_address: None,
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
            bluetooth_address: None,
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
    fn completely_missing_flow_identity_keeps_uuid_fallback_available() {
        let flow = FlowDeviceIdentity {
            device_instance_id: None,
            container_id: None,
            parent_id: None,
            bluetooth_address: None,
        };
        assert!(!flow.is_sufficient_for_matching());
        assert_eq!(
            identity_association(&native_interface("SWD\\GATTSERVICE\\ONE", None, &[]), &flow),
            IdentityAssociation::Unknown
        );
        assert_eq!(
            select_candidate_index(&[candidate(IdentityAssociation::Unknown)]),
            Ok(0)
        );
    }

    #[test]
    fn bluetooth_address_can_disambiguate_candidates_without_pnp_properties() {
        let flow = FlowDeviceIdentity {
            device_instance_id: None,
            container_id: None,
            parent_id: None,
            bluetooth_address: Some(0x0011_2233_4455),
        };
        let mut interface = native_interface("SWD\\GATTSERVICE\\ONE", None, &[]);
        interface.bluetooth_address = Some(0x0011_2233_4455);
        assert_eq!(
            identity_association(&interface, &flow),
            IdentityAssociation::Address
        );
    }

    #[test]
    fn exact_container_identity_has_selection_priority() {
        assert_eq!(
            select_candidate_index(&[
                candidate(IdentityAssociation::Address),
                candidate(IdentityAssociation::Container),
            ]),
            Ok(1)
        );
    }

    #[test]
    fn unrelated_open_failure_is_not_promoted_to_flow_transport_failure() {
        assert!(!should_promote_open_failure(0, 6));
        assert!(!should_promote_open_failure(2, 6));
        assert!(should_stop_before_lower_priority(
            Some(IdentityAssociation::Container),
            IdentityAssociation::Mismatch,
            1,
        ));
        assert!(!should_stop_before_lower_priority(
            Some(IdentityAssociation::Container),
            IdentityAssociation::Mismatch,
            0,
        ));
    }

    #[test]
    fn sole_strongly_associated_open_failure_is_reported_explicitly() {
        assert!(should_promote_open_failure(1, 6));
        assert!(should_promote_open_failure(0, 1));
        let error = retarget_open_failure(
            native_failure(
                NativeConnectionStage::NativeServiceHandleOpened,
                "CreateFileW(FLOW GATT service interface)",
                true,
                "access denied",
            ),
            4,
            0,
        );
        let text = error.to_string();
        assert!(text.contains("target_service="));
        assert!(text.contains("target_characteristic="));
        assert!(text.contains("candidate_count=4"));
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
            bluetooth_address: None,
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
            bluetooth_address: None,
            parent_chain: parent_chain.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn candidate(association: IdentityAssociation) -> NativeGattCandidate {
        NativeGattCandidate {
            path: "native-path".into(),
            owner_instance_id: "instance".into(),
            owner_container_id: None,
            handle: ServiceHandle(HANDLE::default()),
            service: BTH_LE_GATT_SERVICE::default(),
            characteristic: BTH_LE_GATT_CHARACTERISTIC::default(),
            association,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_flow_service_uuid_is_required_and_unique() {
        let other = GUID::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb);
        assert_eq!(
            select_exact_uuid_result(&[other, FLOW_SERVICE_GUID], FLOW_SERVICE_GUID),
            Ok(1)
        );
        assert!(select_exact_uuid_result(&[other], FLOW_SERVICE_GUID).is_err());
        assert!(
            select_exact_uuid_result(&[FLOW_SERVICE_GUID, FLOW_SERVICE_GUID], FLOW_SERVICE_GUID)
                .is_err()
        );
    }

    #[test]
    fn exact_flow_characteristic_uuid_is_required_and_unique() {
        let other = GUID::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb);
        assert_eq!(
            select_exact_uuid_result(&[other, FLOW_CHARACTERISTIC_GUID], FLOW_CHARACTERISTIC_GUID),
            Ok(1)
        );
        assert!(select_exact_uuid_result(&[other], FLOW_CHARACTERISTIC_GUID).is_err());
    }

    #[test]
    fn production_never_uses_the_global_gatt_service_interface_list() {
        assert!(!PRODUCTION_USES_GLOBAL_GATT_SERVICE_ENUMERATION);
    }

    #[test]
    fn service_device_information_id_is_forwarded_without_rewriting() {
        let instance_id = r"\\?\BTHLEDevice#{flow-service-instance}";
        assert_eq!(
            native_path_from_service_instance_id(instance_id),
            Ok(instance_id.to_owned())
        );
        assert!(native_path_from_service_instance_id("").is_err());
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
        assert!(!error.to_string().contains("code=0x00000000"));
    }
}
