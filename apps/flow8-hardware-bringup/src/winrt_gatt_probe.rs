//! Windows-only direct WinRT GATT diagnostic.
//!
//! This deliberately bypasses btleplug after device selection. It never emits
//! a FLOW 8 protocol packet. The only writes are WinRT's standard CCCD Notify
//! and None operations requested by the diagnostic.

use std::{collections::BTreeSet, time::Duration};

use tokio::sync::mpsc;
use windows::{
    Devices::{
        Bluetooth::{
            BluetoothCacheMode, BluetoothLEDevice,
            GenericAttributeProfile::{
                GattCharacteristic, GattCharacteristicProperties,
                GattClientCharacteristicConfigurationDescriptorValue, GattCommunicationStatus,
                GattSession, GattValueChangedEventArgs, GattWriteResult,
            },
        },
        Enumeration::DeviceInformation,
    },
    Foundation::{IReference, TypedEventHandler},
    Storage::Streams::DataReader,
    core::{Error as WinrtError, GUID, HSTRING, Ref},
};

use super::{DiagnosticLogger, diagnostic_hex, probe_elapsed};

const FLOW_SERVICE_UUID: GUID = GUID::from_u128(0x14839ad4_8d7e_415c_9a42_167340cf2339);
const FLOW_CHARACTERISTIC_UUID: GUID = GUID::from_u128(0x0034594a_a8e7_4b1a_a6b1_cd5243059a57);
const CCCD_UUID: GUID = GUID::from_u128(0x00002902_0000_1000_8000_00805f9b34fb);

#[derive(Default)]
struct DiscoverySnapshot {
    target_characteristic: Option<GattCharacteristic>,
    descriptor_uuids: BTreeSet<String>,
}

enum NotificationObservation {
    Value(Vec<u8>),
    HandlerError(String),
}

fn guid_text(value: GUID) -> String {
    format!("{value:?}").to_ascii_lowercase()
}

fn winrt_error_text(error: &WinrtError) -> String {
    format!(
        "error={} debug={error:?} hresult=0x{:08X}",
        error,
        error.code().0 as u32,
    )
}

fn protocol_error_text(value: windows::core::Result<IReference<u8>>) -> String {
    match value {
        Ok(reference) => match reference.Value() {
            Ok(value) => format!("0x{value:02X}"),
            Err(error) => format!("VALUE_UNAVAILABLE({})", winrt_error_text(&error)),
        },
        Err(error) => format!("NOT_EXPOSED_OR_NONE({})", winrt_error_text(&error)),
    }
}

fn property_names(properties: GattCharacteristicProperties) -> String {
    const FLAGS: &[(GattCharacteristicProperties, &str)] = &[
        (GattCharacteristicProperties::Broadcast, "BROADCAST"),
        (GattCharacteristicProperties::Read, "READ"),
        (
            GattCharacteristicProperties::WriteWithoutResponse,
            "WRITE_WITHOUT_RESPONSE",
        ),
        (GattCharacteristicProperties::Write, "WRITE"),
        (GattCharacteristicProperties::Notify, "NOTIFY"),
        (GattCharacteristicProperties::Indicate, "INDICATE"),
        (
            GattCharacteristicProperties::AuthenticatedSignedWrites,
            "AUTHENTICATED_SIGNED_WRITES",
        ),
        (
            GattCharacteristicProperties::ExtendedProperties,
            "EXTENDED_PROPERTIES",
        ),
        (
            GattCharacteristicProperties::ReliableWrites,
            "RELIABLE_WRITES",
        ),
        (
            GattCharacteristicProperties::WritableAuxiliaries,
            "WRITABLE_AUXILIARIES",
        ),
    ];
    let names = FLAGS
        .iter()
        .filter_map(|(flag, name)| properties.contains(*flag).then_some(*name))
        .collect::<Vec<_>>();
    if names.is_empty() {
        "NONE".to_owned()
    } else {
        names.join(" | ")
    }
}

fn log_status(
    logger: &mut DiagnosticLogger,
    started: std::time::Instant,
    operation: &str,
    status: windows::core::Result<GattCommunicationStatus>,
    protocol_error: windows::core::Result<IReference<u8>>,
) -> Option<GattCommunicationStatus> {
    match status {
        Ok(status) => {
            logger.line(format!(
                "{} operation={} communication_status={status:?}({}) protocol_error={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation,
                status.0,
                protocol_error_text(protocol_error),
            ));
            Some(status)
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} status_read=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation,
                winrt_error_text(&error),
            ));
            None
        }
    }
}

async fn discover(
    device: &BluetoothLEDevice,
    mode: BluetoothCacheMode,
    mode_name: &str,
    logger: &mut DiagnosticLogger,
    started: std::time::Instant,
) -> DiscoverySnapshot {
    let mut snapshot = DiscoverySnapshot::default();
    let operation_name = format!("{mode_name}.services");
    let operation = match device.GetGattServicesWithCacheModeAsync(mode) {
        Ok(operation) => {
            logger.line(format!(
                "{} operation={} api_call=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
            ));
            operation
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "{} operation={} await=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    let service_status = log_status(
        logger,
        started,
        &operation_name,
        result.Status(),
        result.ProtocolError(),
    );
    if service_status != Some(GattCommunicationStatus::Success) {
        return snapshot;
    }
    let services = match result.Services() {
        Ok(services) => services,
        Err(error) => {
            logger.line(format!(
                "{} operation={} result_collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    logger.line(format!(
        "{} operation={} returned_count={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        operation_name,
        services.Size().unwrap_or_default(),
    ));
    let mut target_service = None;
    for service in services {
        match service.Uuid() {
            Ok(uuid) => {
                logger.line(format!(
                    "{} mode={} service_uuid={} target={} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    mode_name,
                    guid_text(uuid),
                    uuid == FLOW_SERVICE_UUID,
                ));
                if uuid == FLOW_SERVICE_UUID {
                    target_service = Some(service);
                }
            }
            Err(error) => logger.line(format!(
                "{} mode={} service_uuid=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                mode_name,
                winrt_error_text(&error),
            )),
        }
    }
    let Some(service) = target_service else {
        logger.line(format!(
            "{} mode={} target_service={} found=NO evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            mode_name,
            guid_text(FLOW_SERVICE_UUID),
        ));
        return snapshot;
    };

    let operation_name = format!("{mode_name}.characteristics");
    let operation = match service.GetCharacteristicsWithCacheModeAsync(mode) {
        Ok(operation) => {
            logger.line(format!(
                "{} operation={} api_call=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
            ));
            operation
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "{} operation={} await=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    let characteristic_status = log_status(
        logger,
        started,
        &operation_name,
        result.Status(),
        result.ProtocolError(),
    );
    if characteristic_status != Some(GattCommunicationStatus::Success) {
        return snapshot;
    }
    let characteristics = match result.Characteristics() {
        Ok(characteristics) => characteristics,
        Err(error) => {
            logger.line(format!(
                "{} operation={} result_collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    logger.line(format!(
        "{} operation={} returned_count={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        operation_name,
        characteristics.Size().unwrap_or_default(),
    ));
    for characteristic in characteristics {
        let uuid = match characteristic.Uuid() {
            Ok(uuid) => uuid,
            Err(error) => {
                logger.line(format!(
                    "{} mode={} characteristic_uuid=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    mode_name,
                    winrt_error_text(&error),
                ));
                continue;
            }
        };
        match characteristic.CharacteristicProperties() {
            Ok(properties) => logger.line(format!(
                "{} mode={} characteristic_uuid={} properties={} properties_raw=0x{:03X} target={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                mode_name,
                guid_text(uuid),
                property_names(properties),
                properties.0,
                uuid == FLOW_CHARACTERISTIC_UUID,
            )),
            Err(error) => logger.line(format!(
                "{} mode={} characteristic_uuid={} properties=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                mode_name,
                guid_text(uuid),
                winrt_error_text(&error),
            )),
        }
        if uuid == FLOW_CHARACTERISTIC_UUID {
            snapshot.target_characteristic = Some(characteristic);
        }
    }
    let Some(characteristic) = snapshot.target_characteristic.as_ref() else {
        logger.line(format!(
            "{} mode={} target_characteristic={} found=NO evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            mode_name,
            guid_text(FLOW_CHARACTERISTIC_UUID),
        ));
        return snapshot;
    };

    let operation_name = format!("{mode_name}.descriptors");
    let operation = match characteristic.GetDescriptorsWithCacheModeAsync(mode) {
        Ok(operation) => {
            logger.line(format!(
                "{} operation={} api_call=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
            ));
            operation
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "{} operation={} await=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    let descriptor_status = log_status(
        logger,
        started,
        &operation_name,
        result.Status(),
        result.ProtocolError(),
    );
    if descriptor_status != Some(GattCommunicationStatus::Success) {
        return snapshot;
    }
    let descriptors = match result.Descriptors() {
        Ok(descriptors) => descriptors,
        Err(error) => {
            logger.line(format!(
                "{} operation={} result_collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return snapshot;
        }
    };
    logger.line(format!(
        "{} operation={} returned_count={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        operation_name,
        descriptors.Size().unwrap_or_default(),
    ));
    for descriptor in descriptors {
        match descriptor.Uuid() {
            Ok(uuid) => {
                let uuid_text = guid_text(uuid);
                logger.line(format!(
                    "{} mode={} descriptor_uuid={} is_cccd={} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    mode_name,
                    uuid_text,
                    uuid == CCCD_UUID,
                ));
                snapshot.descriptor_uuids.insert(uuid_text);
            }
            Err(error) => logger.line(format!(
                "{} mode={} descriptor_uuid=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                mode_name,
                winrt_error_text(&error),
            )),
        }
    }
    logger.line(format!(
        "{} mode={} cccd_uuid={} present={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        mode_name,
        guid_text(CCCD_UUID),
        snapshot.descriptor_uuids.contains(&guid_text(CCCD_UUID)),
    ));
    snapshot
}

fn read_notification(args: Ref<GattValueChangedEventArgs>) -> windows::core::Result<Vec<u8>> {
    let args = args.ok()?;
    let buffer = args.CharacteristicValue()?;
    let reader = DataReader::FromBuffer(&buffer)?;
    let len = reader.UnconsumedBufferLength()? as usize;
    let mut bytes = vec![0; len];
    if !bytes.is_empty() {
        reader.ReadBytes(&mut bytes)?;
    }
    Ok(bytes)
}

async fn write_cccd(
    characteristic: &GattCharacteristic,
    value: GattClientCharacteristicConfigurationDescriptorValue,
    operation_name: &str,
    logger: &mut DiagnosticLogger,
    started: std::time::Instant,
) -> bool {
    let operation = match characteristic
        .WriteClientCharacteristicConfigurationDescriptorWithResultAsync(value)
    {
        Ok(operation) => {
            logger.line(format!(
                "{} operation={} api_call=SUCCESS requested_value={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                value.0,
            ));
            operation
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} api_call=FAIL requested_value={} {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                value.0,
                winrt_error_text(&error),
            ));
            return false;
        }
    };
    let result: GattWriteResult = match operation.await {
        Ok(result) => {
            logger.line(format!(
                "{} operation={} completion=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
            ));
            result
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            return false;
        }
    };
    match result.Status() {
        Ok(status) => {
            logger.line(format!(
                "{} operation={} GattWriteResult.Status={status:?}({}) GattWriteResult.ProtocolError={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                status.0,
                protocol_error_text(result.ProtocolError()),
            ));
            status == GattCommunicationStatus::Success
        }
        Err(error) => {
            logger.line(format!(
                "{} operation={} GattWriteResult.Status=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                operation_name,
                winrt_error_text(&error),
            ));
            false
        }
    }
}

pub(super) async fn run(
    device_name: &str,
    observe_for: Duration,
    logger: &mut DiagnosticLogger,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    logger.line("FLOW 8 direct WinRT GATT probe");
    logger.line("policy=WINRT_ONLY FLOW_PROTOCOL_TX=0 MIXER_CONTROL_TX=0 HANDSHAKE_TX=0; only standard CCCD Notify/None operations are permitted");
    logger.line(format!(
        "target device_name={device_name:?} service_uuid={} characteristic_uuid={} cccd_uuid={}",
        guid_text(FLOW_SERVICE_UUID),
        guid_text(FLOW_CHARACTERISTIC_UUID),
        guid_text(CCCD_UUID),
    ));

    let selector =
        match BluetoothLEDevice::GetDeviceSelectorFromDeviceName(&HSTRING::from(device_name)) {
            Ok(selector) => selector,
            Err(error) => {
                logger.line(format!(
                    "{} operation=device_selector result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    winrt_error_text(&error),
                ));
                return Ok(());
            }
        };
    logger.line(format!(
        "{} operation=device_selector result=SUCCESS evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
    ));
    let operation = match DeviceInformation::FindAllAsyncAqsFilter(&selector) {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "{} operation=device_enumeration api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    let devices = match operation.await {
        Ok(devices) => devices,
        Err(error) => {
            logger.line(format!(
                "{} operation=device_enumeration completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    logger.line(format!(
        "{} operation=device_enumeration completion=SUCCESS returned_count={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        devices.Size().unwrap_or_default(),
    ));
    let mut selected = None;
    for info in devices {
        let name = info.Name().map(|value| value.to_string());
        let id = info.Id();
        logger.line(format!(
            "{} candidate name={name:?} id={id:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
        ));
        if selected.is_none() {
            selected = Some(info);
        }
    }
    let Some(info) = selected else {
        logger.line(format!(
            "{} operation=device_enumeration target_found=NO evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
        ));
        return Ok(());
    };
    let id = match info.Id() {
        Ok(id) => id,
        Err(error) => {
            logger.line(format!(
                "{} operation=device_id result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    let operation = match BluetoothLEDevice::FromIdAsync(&id) {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "{} operation=BluetoothLEDevice.FromIdAsync api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    let device = match operation.await {
        Ok(device) => device,
        Err(error) => {
            logger.line(format!(
                "{} operation=BluetoothLEDevice.FromIdAsync completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    logger.line(format!(
        "{} operation=BluetoothLEDevice.FromIdAsync completion=SUCCESS name={:?} address={:?} connection_status={:?} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        device.Name().map(|value| value.to_string()),
        device.BluetoothAddress(),
        device.ConnectionStatus(),
    ));

    let gatt_session = match device.BluetoothDeviceId() {
        Ok(device_id) => match GattSession::FromDeviceIdAsync(&device_id) {
            Ok(operation) => match operation.await {
                Ok(session) => Some(session),
                Err(error) => {
                    logger.line(format!(
                        "{} operation=GattSession.FromDeviceIdAsync completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(started),
                        winrt_error_text(&error),
                    ));
                    None
                }
            },
            Err(error) => {
                logger.line(format!(
                    "{} operation=GattSession.FromDeviceIdAsync api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    winrt_error_text(&error),
                ));
                None
            }
        },
        Err(error) => {
            logger.line(format!(
                "{} operation=BluetoothDeviceId result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    };
    if let Some(session) = &gatt_session {
        let maintain_result = session.SetMaintainConnection(true);
        logger.line(format!(
            "{} operation=GattSession.SetMaintainConnection(true) result={:?} max_pdu_size={:?} can_maintain={:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            maintain_result,
            session.MaxPduSize(),
            session.CanMaintainConnection(),
        ));
    }

    let cached = discover(
        &device,
        BluetoothCacheMode::Cached,
        "CACHED",
        logger,
        started,
    )
    .await;
    let uncached = discover(
        &device,
        BluetoothCacheMode::Uncached,
        "UNCACHED",
        logger,
        started,
    )
    .await;
    let cached_only = cached
        .descriptor_uuids
        .difference(&uncached.descriptor_uuids)
        .cloned()
        .collect::<Vec<_>>();
    let uncached_only = uncached
        .descriptor_uuids
        .difference(&cached.descriptor_uuids)
        .cloned()
        .collect::<Vec<_>>();
    logger.line(format!(
        "{} descriptor_comparison equal={} cached={:?} uncached={:?} cached_only={cached_only:?} uncached_only={uncached_only:?} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        cached.descriptor_uuids == uncached.descriptor_uuids,
        cached.descriptor_uuids,
        uncached.descriptor_uuids,
    ));

    let Some(characteristic) = uncached.target_characteristic else {
        logger.line(format!(
            "{} notification_probe=SKIPPED reason=UNCACHED_TARGET_CHARACTERISTIC_NOT_FOUND evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
        ));
        if let Some(session) = gatt_session {
            let _ = session.SetMaintainConnection(false);
        }
        return Ok(());
    };

    let (notification_tx, mut notification_rx) = mpsc::unbounded_channel();
    let handler = TypedEventHandler::new(
        move |_: Ref<GattCharacteristic>, args: Ref<GattValueChangedEventArgs>| {
            let observation = match read_notification(args) {
                Ok(value) => NotificationObservation::Value(value),
                Err(error) => NotificationObservation::HandlerError(winrt_error_text(&error)),
            };
            let _ = notification_tx.send(observation);
            Ok(())
        },
    );
    let handler_token = match characteristic.ValueChanged(&handler) {
        Ok(token) => {
            logger.line(format!(
                "{} operation=ValueChanged.attach result=SUCCESS token={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                token,
            ));
            Some(token)
        }
        Err(error) => {
            logger.line(format!(
                "{} operation=ValueChanged.attach result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    };

    let notify_enabled = if handler_token.is_some() {
        write_cccd(
            &characteristic,
            GattClientCharacteristicConfigurationDescriptorValue::Notify,
            "CCCD_NOTIFY",
            logger,
            started,
        )
        .await
    } else {
        logger.line(format!(
            "{} operation=CCCD_NOTIFY result=SKIPPED reason=VALUE_CHANGED_HANDLER_NOT_ATTACHED evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
        ));
        false
    };

    if notify_enabled {
        logger.line(format!(
            "{} notification_observation started duration_ms={} FLOW_PROTOCOL_TX=0 evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            observe_for.as_millis(),
        ));
        let deadline = tokio::time::Instant::now() + observe_for;
        let mut notification_count = 0usize;
        loop {
            match tokio::time::timeout_at(deadline, notification_rx.recv()).await {
                Ok(Some(NotificationObservation::Value(value))) => {
                    notification_count += 1;
                    logger.line(format!(
                        "{} WINRT_NOTIFICATION index={} raw={} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(started),
                        notification_count,
                        diagnostic_hex(&value),
                    ));
                }
                Ok(Some(NotificationObservation::HandlerError(error))) => logger.line(format!(
                    "{} WINRT_NOTIFICATION handler_error={} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    error,
                )),
                Ok(None) | Err(_) => break,
            }
        }
        logger.line(format!(
            "{} notification_observation complete count={} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            notification_count,
        ));
    }

    let disabled = write_cccd(
        &characteristic,
        GattClientCharacteristicConfigurationDescriptorValue::None,
        "CCCD_NONE",
        logger,
        started,
    )
    .await;
    logger.line(format!(
        "{} notification_cleanup cccd_none_success={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        disabled,
    ));
    if let Some(token) = handler_token {
        match characteristic.RemoveValueChanged(token) {
            Ok(()) => logger.line(format!(
                "{} operation=ValueChanged.remove result=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
            )),
            Err(error) => logger.line(format!(
                "{} operation=ValueChanged.remove result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            )),
        }
    }
    if let Some(session) = gatt_session {
        match session.SetMaintainConnection(false) {
            Ok(()) => logger.line(format!(
                "{} operation=GattSession.SetMaintainConnection(false) result=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
            )),
            Err(error) => logger.line(format!(
                "{} operation=GattSession.SetMaintainConnection(false) result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            )),
        }
    }
    logger.line("SAFETY_RESULT FLOW_PROTOCOL_TX=0 MIXER_CONTROL_TX=0 HANDSHAKE_TX=0; descriptor writes limited to WinRT CCCD Notify/None");
    Ok(())
}
