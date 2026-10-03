//! Windows-only passive FLOW 8 handshake receiver.
//!
//! The FLOW 8 characteristic has no CCCD. This module attaches WinRT's
//! `GattCharacteristic::ValueChanged` event directly and deliberately never
//! calls any `WriteClientCharacteristicConfigurationDescriptor*` API.

use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use windows::{
    Devices::{
        Bluetooth::{
            BluetoothCacheMode, BluetoothLEDevice,
            GenericAttributeProfile::{
                GattCharacteristic, GattCommunicationStatus, GattSession,
                GattValueChangedEventArgs, GattWriteOption, GattWriteResult,
            },
        },
        Enumeration::DeviceInformation,
    },
    Foundation::{IReference, TypedEventHandler},
    Storage::Streams::{DataReader, DataWriter},
    core::{Error as WinrtError, GUID, HSTRING, Ref},
};

use super::{
    DiagnosticLogger, PassiveAttemptResult, PassiveHandshakePipeline, diagnostic_hex,
    probe_elapsed, unix_timestamp_ms,
};

pub(super) const FLOW_SERVICE_UUID: GUID = GUID::from_u128(0x14839ad4_8d7e_415c_9a42_167340cf2339);
pub(super) const FLOW_CHARACTERISTIC_UUID: GUID =
    GUID::from_u128(0x0034594a_a8e7_4b1a_a6b1_cd5243059a57);

pub(super) fn guid_text(value: GUID) -> String {
    format!("{value:?}").to_ascii_lowercase()
}

pub(super) fn winrt_error_text(error: &WinrtError) -> String {
    format!(
        "error={} debug={error:?} hresult=0x{:08X}",
        error,
        error.code().0 as u32,
    )
}

pub(super) fn protocol_error_text(value: windows::core::Result<IReference<u8>>) -> String {
    match value {
        Ok(reference) => match reference.Value() {
            Ok(value) => format!("0x{value:02X}"),
            Err(error) => format!("VALUE_UNAVAILABLE({})", winrt_error_text(&error)),
        },
        Err(error) => format!("NOT_EXPOSED_OR_NONE({})", winrt_error_text(&error)),
    }
}

pub(super) fn read_notification(
    args: Ref<GattValueChangedEventArgs>,
) -> windows::core::Result<Vec<u8>> {
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

async fn open_device(
    device_name: &str,
    logger: &mut DiagnosticLogger,
    started: Instant,
) -> Option<(BluetoothLEDevice, Option<GattSession>)> {
    let selector = match BluetoothLEDevice::GetDeviceSelectorFromDeviceName(&HSTRING::from(
        device_name,
    )) {
        Ok(selector) => selector,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=device_selector result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let operation = match DeviceInformation::FindAllAsyncAqsFilter(&selector) {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=device_enumeration api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let devices = match operation.await {
        Ok(devices) => devices,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=device_enumeration completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    logger.line(format!(
        "backend=winrt {} operation=device_enumeration returned_count={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        devices.Size().unwrap_or_default(),
    ));
    let mut selected = None;
    for info in devices {
        logger.line(format!(
            "backend=winrt {} candidate name={:?} id={:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            info.Name().map(|value| value.to_string()),
            info.Id(),
        ));
        if selected.is_none() {
            selected = Some(info);
        }
    }
    let info = match selected {
        Some(info) => info,
        None => {
            logger.line(format!(
                "backend=winrt {} target_device_found=NO evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
            ));
            return None;
        }
    };
    let id = match info.Id() {
        Ok(id) => id,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=device_id result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let operation = match BluetoothLEDevice::FromIdAsync(&id) {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=BluetoothLEDevice.FromIdAsync api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let device = match operation.await {
        Ok(device) => device,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=BluetoothLEDevice.FromIdAsync completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    logger.line(format!(
        "backend=winrt {} connect=SUCCESS name={:?} address={:?} connection_status={:?} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        device.Name().map(|value| value.to_string()),
        device.BluetoothAddress(),
        device.ConnectionStatus(),
    ));

    let gatt_session = match device.BluetoothDeviceId() {
        Ok(device_id) => match GattSession::FromDeviceIdAsync(&device_id) {
            Ok(operation) => match operation.await {
                Ok(session) => {
                    let maintain = session.SetMaintainConnection(true);
                    logger.line(format!(
                        "backend=winrt {} operation=GattSession.SetMaintainConnection(true) result={maintain:?} max_pdu_size={:?} can_maintain={:?} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(started),
                        session.MaxPduSize(),
                        session.CanMaintainConnection(),
                    ));
                    Some(session)
                }
                Err(error) => {
                    logger.line(format!(
                        "backend=winrt {} operation=GattSession.FromDeviceIdAsync completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(started),
                        winrt_error_text(&error),
                    ));
                    None
                }
            },
            Err(error) => {
                logger.line(format!(
                    "backend=winrt {} operation=GattSession.FromDeviceIdAsync api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    winrt_error_text(&error),
                ));
                None
            }
        },
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=BluetoothDeviceId result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    };
    Some((device, gatt_session))
}

async fn discover_target(
    device: &BluetoothLEDevice,
    logger: &mut DiagnosticLogger,
    started: Instant,
) -> Option<GattCharacteristic> {
    let operation = match device
        .GetGattServicesForUuidWithCacheModeAsync(FLOW_SERVICE_UUID, BluetoothCacheMode::Uncached)
    {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=uncached_target_service api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=uncached_target_service completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let status = result.Status();
    logger.line(format!(
        "backend=winrt {} operation=uncached_target_service communication_status={status:?} protocol_error={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        protocol_error_text(result.ProtocolError()),
    ));
    if status.ok() != Some(GattCommunicationStatus::Success) {
        return None;
    }
    let services = match result.Services() {
        Ok(services) => services,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=uncached_target_service collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let service = services.into_iter().next()?;
    logger.line(format!(
        "backend=winrt {} service={} discovered=YES cache=UNCACHED evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        guid_text(FLOW_SERVICE_UUID),
    ));

    let operation = match service.GetCharacteristicsForUuidWithCacheModeAsync(
        FLOW_CHARACTERISTIC_UUID,
        BluetoothCacheMode::Uncached,
    ) {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=uncached_target_characteristic api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=uncached_target_characteristic completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let status = result.Status();
    logger.line(format!(
        "backend=winrt {} operation=uncached_target_characteristic communication_status={status:?} protocol_error={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        protocol_error_text(result.ProtocolError()),
    ));
    if status.ok() != Some(GattCommunicationStatus::Success) {
        return None;
    }
    let characteristics = match result.Characteristics() {
        Ok(characteristics) => characteristics,
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} operation=uncached_target_characteristic collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    let characteristic = characteristics.into_iter().next()?;
    logger.line(format!(
        "backend=winrt {} characteristic={} properties={:?} discovered=YES cache=UNCACHED evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        guid_text(FLOW_CHARACTERISTIC_UUID),
        characteristic.CharacteristicProperties(),
    ));
    Some(characteristic)
}

pub(super) async fn write_with_response(
    backend: &str,
    characteristic: &GattCharacteristic,
    frame: &[u8],
    connected_at: Instant,
    logger: &mut DiagnosticLogger,
) -> bool {
    let stamp = unix_timestamp_ms();
    logger.line(format!(
        "timestamp_ms={stamp} backend={backend} {} TX raw={} write_type=WithResponse codec=PRODUCTION evidence=VERIFIED_FROM_APK",
        probe_elapsed(connected_at),
        diagnostic_hex(frame),
    ));
    let writer = match DataWriter::new() {
        Ok(writer) => writer,
        Err(error) => {
            logger.line(format!(
                "timestamp_ms={stamp} backend={backend} {} write=FAIL stage=DataWriter {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            ));
            return false;
        }
    };
    if let Err(error) = writer.WriteBytes(frame) {
        logger.line(format!(
            "timestamp_ms={stamp} backend={backend} {} write=FAIL stage=WriteBytes {} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            winrt_error_text(&error),
        ));
        return false;
    }
    let buffer = match writer.DetachBuffer() {
        Ok(buffer) => buffer,
        Err(error) => {
            logger.line(format!(
                "timestamp_ms={stamp} backend={backend} {} write=FAIL stage=DetachBuffer {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            ));
            return false;
        }
    };
    let operation = match characteristic
        .WriteValueWithResultAndOptionAsync(&buffer, GattWriteOption::WriteWithResponse)
    {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "timestamp_ms={stamp} backend={backend} {} write=FAIL stage=API_CALL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            ));
            return false;
        }
    };
    let result: GattWriteResult = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "timestamp_ms={stamp} backend={backend} {} write=FAIL stage=AWAIT {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            ));
            return false;
        }
    };
    match result.Status() {
        Ok(status) => {
            logger.line(format!(
                "timestamp_ms={stamp} backend={backend} {} write_status={status:?} protocol_error={} write_type=WithResponse evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                protocol_error_text(result.ProtocolError()),
            ));
            status == GattCommunicationStatus::Success
        }
        Err(error) => {
            logger.line(format!(
                "timestamp_ms={stamp} backend={backend} {} write=FAIL stage=STATUS {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            ));
            false
        }
    }
}

pub(super) async fn run(
    device_name: &str,
    timeout: Duration,
    logger: &mut DiagnosticLogger,
) -> Result<PassiveAttemptResult, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut pipeline = PassiveHandshakePipeline::new("winrt");
    logger.line("backend=winrt passive_listener=ATTEMPT policy=VALUE_CHANGED_WITHOUT_CCCD subscribe_calls=0 cccd_writes=0");
    let Some((device, gatt_session)) = open_device(device_name, logger, started).await else {
        return Ok(pipeline.result());
    };
    let connected_at = Instant::now();
    let Some(characteristic) = discover_target(&device, logger, connected_at).await else {
        if let Some(session) = gatt_session {
            let _ = session.SetMaintainConnection(false);
        }
        return Ok(pipeline.result());
    };

    let (notification_tx, mut notification_rx) = mpsc::unbounded_channel();
    let handler = TypedEventHandler::new(
        move |_: Ref<GattCharacteristic>, args: Ref<GattValueChangedEventArgs>| {
            let result = read_notification(args).map_err(|error| winrt_error_text(&error));
            let _ = notification_tx.send(result);
            Ok(())
        },
    );
    let token = match characteristic.ValueChanged(&handler) {
        Ok(token) => {
            logger.line(format!(
                "backend=winrt {} ValueChanged.attach=SUCCESS token={} subscribe_calls=0 cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                token,
            ));
            Some(token)
        }
        Err(error) => {
            logger.line(format!(
                "backend=winrt {} ValueChanged.attach=FAIL {} subscribe_calls=0 cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            ));
            None
        }
    };

    if token.is_some() {
        let deadline = tokio::time::Instant::now() + timeout;
        while !pipeline.is_ready() {
            match tokio::time::timeout_at(deadline, notification_rx.recv()).await {
                Ok(Some(Ok(value))) => {
                    let writes = pipeline.accept(
                        &value,
                        guid_text(FLOW_CHARACTERISTIC_UUID),
                        connected_at,
                        logger,
                    );
                    for frame in writes {
                        let _ = write_with_response(
                            "winrt",
                            &characteristic,
                            &frame,
                            connected_at,
                            logger,
                        )
                        .await;
                    }
                }
                Ok(Some(Err(error))) => logger.line(format!(
                    "backend=winrt {} ValueChanged.handler=FAIL error={error} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(connected_at),
                )),
                Ok(None) => {
                    logger.line(format!(
                        "backend=winrt {} ValueChanged.channel=ENDED evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(connected_at),
                    ));
                    break;
                }
                Err(_) => {
                    logger.line(format!(
                        "backend=winrt {} passive_handshake=TIMEOUT timeout_ms={} notifications={} subscribe_calls=0 cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(connected_at),
                        timeout.as_millis(),
                        pipeline.result().notification_count,
                    ));
                    break;
                }
            }
        }
    }

    if let Some(token) = token {
        match characteristic.RemoveValueChanged(token) {
            Ok(()) => logger.line(format!(
                "backend=winrt {} ValueChanged.remove=SUCCESS cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
            )),
            Err(error) => logger.line(format!(
                "backend=winrt {} ValueChanged.remove=FAIL {} cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                winrt_error_text(&error),
            )),
        }
    }
    if let Some(session) = gatt_session {
        logger.line(format!(
            "backend=winrt {} GattSession.SetMaintainConnection(false)={:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            session.SetMaintainConnection(false),
        ));
    }
    logger.line("backend=winrt SAFETY_RESULT subscribe_calls=0 cccd_writes=0 mixer_control_tx=0; only production-codec 0x39/0x37 responses were eligible for write");
    Ok(pipeline.result())
}
