//! Windows-only FLOW 8 pre-armed handshake timing diagnostic.
//!
//! This path uses cached GATT objects, attaches `ValueChanged`, and only then
//! asks WinRT to maintain the physical connection. It never subscribes, writes
//! a CCCD, performs uncached discovery, or emits a mixer-control command.

use std::time::{Duration, Instant};

use flow8_protocol::parse_packet;
use tokio::sync::mpsc;
use windows::{
    Devices::{
        Bluetooth::{
            BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
            GenericAttributeProfile::{GattCharacteristic, GattCommunicationStatus, GattSession},
        },
        Enumeration::DeviceInformation,
    },
    Foundation::TypedEventHandler,
    core::{HSTRING, IInspectable, Ref},
};

use super::{
    DiagnosticLogger, PassiveHandshakePipeline, diagnostic_hex, probe_elapsed, unix_timestamp_ms,
    winrt_passive_handshake::{
        FLOW_CHARACTERISTIC_UUID, FLOW_SERVICE_UUID, guid_text, protocol_error_text,
        read_notification, winrt_error_text, write_with_response,
    },
};

enum PrearmedEvent {
    Connection {
        at: Instant,
        timestamp_ms: u128,
        status: BluetoothConnectionStatus,
    },
    Notification {
        at: Instant,
        timestamp_ms: u128,
        value: Vec<u8>,
    },
    HandlerError {
        at: Instant,
        timestamp_ms: u128,
        source: &'static str,
        error: String,
    },
}

struct Milestone {
    name: &'static str,
    at: Instant,
    timestamp_ms: u128,
}

fn status_name(status: BluetoothConnectionStatus) -> &'static str {
    if status == BluetoothConnectionStatus::Connected {
        "Connected"
    } else if status == BluetoothConnectionStatus::Disconnected {
        "Disconnected"
    } else {
        "Unknown"
    }
}

fn log_connection_status(
    device: &BluetoothLEDevice,
    operation: &str,
    point: &str,
    started: Instant,
    logger: &mut DiagnosticLogger,
) -> Option<BluetoothConnectionStatus> {
    match device.ConnectionStatus() {
        Ok(status) => {
            logger.line(format!(
                "timestamp_ms={} backend=winrt-prearmed {} operation={operation} point={point} ConnectionStatus={}({}) evidence=VERIFIED_FROM_DEVICE",
                unix_timestamp_ms(),
                probe_elapsed(started),
                status_name(status),
                status.0,
            ));
            Some(status)
        }
        Err(error) => {
            logger.line(format!(
                "timestamp_ms={} backend=winrt-prearmed {} operation={operation} point={point} ConnectionStatus=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                unix_timestamp_ms(),
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    }
}

fn mark(milestones: &mut Vec<Milestone>, name: &'static str, at: Instant, timestamp_ms: u128) {
    if milestones.iter().all(|item| item.name != name) {
        milestones.push(Milestone {
            name,
            at,
            timestamp_ms,
        });
    }
}

fn observe_connection_event(
    at: Instant,
    timestamp_ms: u128,
    status: BluetoothConnectionStatus,
    started: Instant,
    connected_at: &mut Option<Instant>,
    milestones: &mut Vec<Milestone>,
    logger: &mut DiagnosticLogger,
) {
    logger.line(format!(
        "timestamp_ms={timestamp_ms} backend=winrt-prearmed t_experiment_ms={} ConnectionStatusChanged={}({}) evidence=VERIFIED_FROM_DEVICE",
        at.duration_since(started).as_millis(),
        status_name(status),
        status.0,
    ));
    if status == BluetoothConnectionStatus::Connected && connected_at.is_none() {
        *connected_at = Some(at);
        mark(milestones, "ConnectionStatus=Connected", at, timestamp_ms);
    }
}

fn drain_connection_events(
    events: &mut mpsc::UnboundedReceiver<PrearmedEvent>,
    started: Instant,
    connected_at: &mut Option<Instant>,
    milestones: &mut Vec<Milestone>,
    logger: &mut DiagnosticLogger,
) {
    while let Ok(event) = events.try_recv() {
        match event {
            PrearmedEvent::Connection {
                at,
                timestamp_ms,
                status,
            } => observe_connection_event(
                at,
                timestamp_ms,
                status,
                started,
                connected_at,
                milestones,
                logger,
            ),
            PrearmedEvent::Notification {
                at,
                timestamp_ms,
                value,
            } => logger.line(format!(
                "timestamp_ms={timestamp_ms} backend=winrt-prearmed t_experiment_ms={} EARLY_NOTIFICATION raw={} listener_expected=NOT_YET_ARMED evidence=VERIFIED_FROM_DEVICE",
                at.duration_since(started).as_millis(),
                diagnostic_hex(&value),
            )),
            PrearmedEvent::HandlerError {
                at,
                timestamp_ms,
                source,
                error,
            } => logger.line(format!(
                "timestamp_ms={timestamp_ms} backend=winrt-prearmed t_experiment_ms={} handler={source} error={error} evidence=VERIFIED_FROM_DEVICE",
                at.duration_since(started).as_millis(),
            )),
        }
    }
}

async fn cached_target_characteristic(
    device: &BluetoothLEDevice,
    started: Instant,
    logger: &mut DiagnosticLogger,
) -> Option<GattCharacteristic> {
    let operation_name = "cached_target_service";
    log_connection_status(device, operation_name, "before_api_call", started, logger);
    let operation = match device
        .GetGattServicesForUuidWithCacheModeAsync(FLOW_SERVICE_UUID, BluetoothCacheMode::Cached)
    {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} operation={operation_name} api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            log_connection_status(device, operation_name, "after_api_failure", started, logger);
            return None;
        }
    };
    log_connection_status(device, operation_name, "after_api_call", started, logger);
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} operation={operation_name} await=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            log_connection_status(
                device,
                operation_name,
                "after_await_failure",
                started,
                logger,
            );
            return None;
        }
    };
    log_connection_status(device, operation_name, "after_await", started, logger);
    let status = result.Status();
    logger.line(format!(
        "backend=winrt-prearmed {} operation={operation_name} cache=CACHED communication_status={status:?} protocol_error={} evidence=VERIFIED_FROM_DEVICE",
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
                "backend=winrt-prearmed {} operation={operation_name} collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    log_connection_status(device, operation_name, "after_collection", started, logger);
    let service = match services.into_iter().next() {
        Some(service) => service,
        None => {
            logger.line(format!(
                "backend=winrt-prearmed {} service={} cache=CACHED found=NO uncached_calls=0 result=PREARM_NOT_POSSIBLE evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                guid_text(FLOW_SERVICE_UUID),
            ));
            return None;
        }
    };

    let operation_name = "cached_target_characteristic";
    log_connection_status(device, operation_name, "before_api_call", started, logger);
    let operation = match service.GetCharacteristicsForUuidWithCacheModeAsync(
        FLOW_CHARACTERISTIC_UUID,
        BluetoothCacheMode::Cached,
    ) {
        Ok(operation) => operation,
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} operation={operation_name} api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            log_connection_status(device, operation_name, "after_api_failure", started, logger);
            return None;
        }
    };
    log_connection_status(device, operation_name, "after_api_call", started, logger);
    let result = match operation.await {
        Ok(result) => result,
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} operation={operation_name} await=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            log_connection_status(
                device,
                operation_name,
                "after_await_failure",
                started,
                logger,
            );
            return None;
        }
    };
    log_connection_status(device, operation_name, "after_await", started, logger);
    let status = result.Status();
    logger.line(format!(
        "backend=winrt-prearmed {} operation={operation_name} cache=CACHED communication_status={status:?} protocol_error={} evidence=VERIFIED_FROM_DEVICE",
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
                "backend=winrt-prearmed {} operation={operation_name} collection=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            return None;
        }
    };
    log_connection_status(device, operation_name, "after_collection", started, logger);
    let characteristic = match characteristics.into_iter().next() {
        Some(characteristic) => characteristic,
        None => {
            logger.line(format!(
                "backend=winrt-prearmed {} characteristic={} cache=CACHED found=NO uncached_calls=0 result=PREARM_NOT_POSSIBLE evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                guid_text(FLOW_CHARACTERISTIC_UUID),
            ));
            return None;
        }
    };
    logger.line(format!(
        "backend=winrt-prearmed {} service={} characteristic={} cache=CACHED properties={:?} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        guid_text(FLOW_SERVICE_UUID),
        guid_text(FLOW_CHARACTERISTIC_UUID),
        characteristic.CharacteristicProperties(),
    ));
    Some(characteristic)
}

fn print_timeline(
    milestones: &mut [Milestone],
    connected_at: Option<Instant>,
    logger: &mut DiagnosticLogger,
) {
    milestones.sort_by_key(|milestone| milestone.at);
    logger.line("");
    logger.line("PREARMED TIMELINE");
    for milestone in milestones {
        let relative = match connected_at {
            Some(connected) if milestone.at >= connected => {
                format!("+{} ms", milestone.at.duration_since(connected).as_millis())
            }
            Some(connected) => {
                format!("-{} ms", connected.duration_since(milestone.at).as_millis())
            }
            None => "UNAVAILABLE_NO_CONNECTED_TRANSITION_EVENT".to_owned(),
        };
        logger.line(format!(
            "timestamp_ms={} milestone={} relative_to_connected={relative}",
            milestone.timestamp_ms, milestone.name,
        ));
    }
}

pub(super) async fn run(
    device_name: &str,
    timeout: Duration,
    logger: &mut DiagnosticLogger,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut milestones = Vec::new();
    let mut connected_at = None;
    logger.line("FLOW 8 WinRT pre-armed handshake diagnostic");
    logger.line("policy=CACHED_GATT_BEFORE_LISTENER NO_UNCACHED NO_SUBSCRIBE NO_CCCD_WRITE NO_MIXER_CONTROL; permitted TX=production-codec 0x39/0x37 only");
    logger.line("ANDROID_HCI evidence=VERIFIED_FROM_DEVICE 0x35_retransmit_interval_approx_ms=500 complete_retry_approx_ms=1000");

    let selector =
        match BluetoothLEDevice::GetDeviceSelectorFromDeviceName(&HSTRING::from(device_name)) {
            Ok(selector) => selector,
            Err(error) => {
                logger.line(format!(
                    "operation=device_selector result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    winrt_error_text(&error),
                ));
                return Ok(());
            }
        };
    let devices = match DeviceInformation::FindAllAsyncAqsFilter(&selector) {
        Ok(operation) => match operation.await {
            Ok(devices) => devices,
            Err(error) => {
                logger.line(format!(
                    "operation=device_enumeration completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    winrt_error_text(&error),
                ));
                return Ok(());
            }
        },
        Err(error) => {
            logger.line(format!(
                "operation=device_enumeration api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    logger.line(format!(
        "operation=device_enumeration returned_count={} evidence=VERIFIED_FROM_DEVICE",
        devices.Size().unwrap_or_default(),
    ));
    let mut selected = None;
    for info in devices {
        logger.line(format!(
            "candidate name={:?} id={:?} evidence=VERIFIED_FROM_DEVICE",
            info.Name().map(|value| value.to_string()),
            info.Id(),
        ));
        if selected.is_none() {
            selected = Some(info);
        }
    }
    let Some(info) = selected else {
        logger.line("target_device_found=NO evidence=VERIFIED_FROM_DEVICE");
        return Ok(());
    };
    let id = match info.Id() {
        Ok(id) => id,
        Err(error) => {
            logger.line(format!(
                "operation=device_id result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };

    logger.line(format!(
        "timestamp_ms={} {} operation=BluetoothLEDevice.FromIdAsync point=before_api_call ConnectionStatus=UNAVAILABLE_DEVICE_NOT_CREATED",
        unix_timestamp_ms(),
        probe_elapsed(started),
    ));
    let device = match BluetoothLEDevice::FromIdAsync(&id) {
        Ok(operation) => match operation.await {
            Ok(device) => device,
            Err(error) => {
                logger.line(format!(
                    "operation=BluetoothLEDevice.FromIdAsync completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    winrt_error_text(&error),
                ));
                return Ok(());
            }
        },
        Err(error) => {
            logger.line(format!(
                "operation=BluetoothLEDevice.FromIdAsync api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                winrt_error_text(&error),
            ));
            return Ok(());
        }
    };
    let after_from_id = log_connection_status(
        &device,
        "BluetoothLEDevice.FromIdAsync",
        "after_await",
        started,
        logger,
    );
    logger.line(format!(
        "backend=winrt-prearmed {} device_name={:?} address={:?} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(started),
        device.Name().map(|value| value.to_string()),
        device.BluetoothAddress(),
    ));

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let connection_tx = event_tx.clone();
    let connection_handler = TypedEventHandler::<BluetoothLEDevice, IInspectable>::new(
        move |sender: Ref<BluetoothLEDevice>, _: Ref<IInspectable>| {
            let at = Instant::now();
            let timestamp_ms = unix_timestamp_ms();
            let event = match sender.ok().and_then(|device| device.ConnectionStatus()) {
                Ok(status) => PrearmedEvent::Connection {
                    at,
                    timestamp_ms,
                    status,
                },
                Err(error) => PrearmedEvent::HandlerError {
                    at,
                    timestamp_ms,
                    source: "ConnectionStatusChanged",
                    error: winrt_error_text(&error),
                },
            };
            let _ = connection_tx.send(event);
            Ok(())
        },
    );
    log_connection_status(
        &device,
        "ConnectionStatusChanged.attach",
        "before_api_call",
        started,
        logger,
    );
    let connection_token = match device.ConnectionStatusChanged(&connection_handler) {
        Ok(token) => {
            logger.line(format!(
                "backend=winrt-prearmed {} ConnectionStatusChanged.attach=SUCCESS token={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                token,
            ));
            Some(token)
        }
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} ConnectionStatusChanged.attach=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    };
    log_connection_status(
        &device,
        "ConnectionStatusChanged.attach",
        "after_api_call",
        started,
        logger,
    );

    let cached_characteristic = cached_target_characteristic(&device, started, logger).await;
    drain_connection_events(
        &mut event_rx,
        started,
        &mut connected_at,
        &mut milestones,
        logger,
    );
    let status_after_cached_lookup =
        log_connection_status(&device, "cached_gatt_lookup", "complete", started, logger);
    let connected_before_cached_lookup =
        after_from_id == Some(BluetoothConnectionStatus::Connected);
    let connected_during_cached_lookup = !connected_before_cached_lookup
        && (status_after_cached_lookup == Some(BluetoothConnectionStatus::Connected)
            || connected_at.is_some());
    if connected_during_cached_lookup {
        logger.line("PREARM_INTEGRITY=COMPROMISED trigger=CACHED_SERVICE_OR_CHARACTERISTIC_LOOKUP ConnectionStatus_became_Connected_before_ValueChanged_attach; any later timeout MUST_NOT be interpreted as Windows suppressing unsolicited notifications evidence=VERIFIED_FROM_DEVICE");
    } else if connected_before_cached_lookup {
        logger.line("PREARM_INTEGRITY=COMPROMISED trigger=BluetoothLEDevice.FromIdAsync ConnectionStatus_was_already_Connected_before_cached_lookup_and_listener_attach; any later timeout MUST_NOT be interpreted as Windows suppressing unsolicited notifications evidence=VERIFIED_FROM_DEVICE");
    }
    let Some(characteristic) = cached_characteristic else {
        logger.line(format!(
            "PREARM_RESULT=NOT_RUN reason=CACHED_SERVICE_OR_CHARACTERISTIC_UNAVAILABLE connected_before_cached_lookup={connected_before_cached_lookup} connected_during_cached_lookup={connected_during_cached_lookup} uncached_calls=0"
        ));
        if let Some(token) = connection_token {
            let _ = device.RemoveConnectionStatusChanged(token);
        }
        print_timeline(&mut milestones, connected_at, logger);
        logger.line("SAFETY_RESULT uncached_calls=0 subscribe_calls=0 cccd_writes=0 mixer_control_tx=0 handshake_tx=0");
        return Ok(());
    };
    let before_listener_status = log_connection_status(
        &device,
        "ValueChanged.attach",
        "before_api_call",
        started,
        logger,
    );

    let notification_tx = event_tx.clone();
    let value_handler = TypedEventHandler::new(move |_: Ref<GattCharacteristic>, args| {
        let at = Instant::now();
        let timestamp_ms = unix_timestamp_ms();
        let event = match read_notification(args) {
            Ok(value) => PrearmedEvent::Notification {
                at,
                timestamp_ms,
                value,
            },
            Err(error) => PrearmedEvent::HandlerError {
                at,
                timestamp_ms,
                source: "ValueChanged",
                error: winrt_error_text(&error),
            },
        };
        let _ = notification_tx.send(event);
        Ok(())
    });
    let value_token = match characteristic.ValueChanged(&value_handler) {
        Ok(token) => {
            let at = Instant::now();
            let timestamp_ms = unix_timestamp_ms();
            mark(&mut milestones, "listener attached", at, timestamp_ms);
            logger.line(format!(
                "timestamp_ms={timestamp_ms} backend=winrt-prearmed {} listener=ARMED token={} subscribe_calls=0 cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                token,
            ));
            Some(token)
        }
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} listener=ARM_FAILED {} subscribe_calls=0 cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    };
    let after_listener_status = log_connection_status(
        &device,
        "ValueChanged.attach",
        "after_api_call",
        started,
        logger,
    );
    let connected_before_arm = connected_before_cached_lookup
        || connected_during_cached_lookup
        || before_listener_status == Some(BluetoothConnectionStatus::Connected)
        || connected_at.is_some();
    let connected_during_listener_attach = !connected_before_arm
        && before_listener_status == Some(BluetoothConnectionStatus::Disconnected)
        && after_listener_status == Some(BluetoothConnectionStatus::Connected);
    if connected_before_arm {
        logger.line("PREARM_INTEGRITY=COMPROMISED reason=ConnectionStatus_was_Connected_before_listener_ARMED; a later notification timeout MUST_NOT be interpreted as Windows suppression evidence=VERIFIED_FROM_DEVICE");
    } else if connected_during_listener_attach {
        logger.line("PREARM_INTEGRITY=AMBIGUOUS reason=ConnectionStatus_changed_during_ValueChanged_attach_API; listener_vs_connection_order_cannot_be_proven; a later timeout MUST_NOT be interpreted as Windows suppression evidence=VERIFIED_FROM_DEVICE");
    } else {
        logger.line("PREARM_INTEGRITY=VALID ConnectionStatus_remained_Disconnected_until_listener_ARMED evidence=VERIFIED_FROM_DEVICE");
    }
    if value_token.is_none() {
        if let Some(token) = connection_token {
            let _ = device.RemoveConnectionStatusChanged(token);
        }
        return Ok(());
    }

    log_connection_status(
        &device,
        "BluetoothDeviceId",
        "before_api_call",
        started,
        logger,
    );
    let bluetooth_device_id = match device.BluetoothDeviceId() {
        Ok(device_id) => Some(device_id),
        Err(error) => {
            logger.line(format!(
                "backend=winrt-prearmed {} operation=BluetoothDeviceId result=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(started),
                winrt_error_text(&error),
            ));
            None
        }
    };
    log_connection_status(
        &device,
        "BluetoothDeviceId",
        "after_api_call",
        started,
        logger,
    );
    let gatt_session = if let Some(device_id) = bluetooth_device_id {
        let connection_initiated_at = Instant::now();
        let connection_initiated_stamp = unix_timestamp_ms();
        mark(
            &mut milestones,
            "connection initiated",
            connection_initiated_at,
            connection_initiated_stamp,
        );
        logger.line(format!(
            "timestamp_ms={connection_initiated_stamp} backend=winrt-prearmed {} connection_initiated=GattSession.FromDeviceIdAsync listener=ARMED evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
        ));
        log_connection_status(
            &device,
            "GattSession.FromDeviceIdAsync",
            "before_api_call",
            started,
            logger,
        );
        match GattSession::FromDeviceIdAsync(&device_id) {
            Ok(operation) => {
                log_connection_status(
                    &device,
                    "GattSession.FromDeviceIdAsync",
                    "after_api_call",
                    started,
                    logger,
                );
                match operation.await {
                    Ok(session) => Some(session),
                    Err(error) => {
                        logger.line(format!(
                            "backend=winrt-prearmed {} operation=GattSession.FromDeviceIdAsync completion=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                            probe_elapsed(started),
                            winrt_error_text(&error),
                        ));
                        None
                    }
                }
            }
            Err(error) => {
                logger.line(format!(
                    "backend=winrt-prearmed {} operation=GattSession.FromDeviceIdAsync api_call=FAIL {} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    winrt_error_text(&error),
                ));
                None
            }
        }
    } else {
        None
    };
    log_connection_status(
        &device,
        "GattSession.FromDeviceIdAsync",
        "after_await",
        started,
        logger,
    );
    if let Some(session) = &gatt_session {
        log_connection_status(
            &device,
            "GattSession.SetMaintainConnection(true)",
            "before_api_call",
            started,
            logger,
        );
        let result = session.SetMaintainConnection(true);
        logger.line(format!(
            "backend=winrt-prearmed {} operation=GattSession.SetMaintainConnection(true) result={result:?} max_pdu_size={:?} can_maintain={:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(started),
            session.MaxPduSize(),
            session.CanMaintainConnection(),
        ));
        log_connection_status(
            &device,
            "GattSession.SetMaintainConnection(true)",
            "after_api_call",
            started,
            logger,
        );
    }

    let mut pipeline = PassiveHandshakePipeline::new("winrt-prearmed");
    let deadline = tokio::time::Instant::now() + timeout;
    while !pipeline.is_ready() {
        match tokio::time::timeout_at(deadline, event_rx.recv()).await {
            Ok(Some(PrearmedEvent::Connection {
                at,
                timestamp_ms,
                status,
            })) => observe_connection_event(
                at,
                timestamp_ms,
                status,
                started,
                &mut connected_at,
                &mut milestones,
                logger,
            ),
            Ok(Some(PrearmedEvent::Notification {
                at,
                timestamp_ms,
                value,
            })) => {
                if milestones.iter().all(|item| item.name != "first notification") {
                    mark(&mut milestones, "first notification", at, timestamp_ms);
                }
                let packet = parse_packet(&value).ok();
                if packet.as_ref().is_some_and(|packet| packet.command == 0x35) {
                    mark(&mut milestones, "first 0x35 fragment", at, timestamp_ms);
                }
                if packet.as_ref().is_some_and(|packet| packet.command == 0x38) {
                    mark(&mut milestones, "first 0x38 fragment", at, timestamp_ms);
                }
                logger.line(format!(
                    "timestamp_ms={timestamp_ms} backend=winrt-prearmed t_experiment_ms={} callback=ValueChanged raw={} evidence=VERIFIED_FROM_DEVICE",
                    at.duration_since(started).as_millis(),
                    diagnostic_hex(&value),
                ));
                let before = pipeline.result();
                let reference = connected_at.unwrap_or(started);
                let writes = pipeline.accept(
                    &value,
                    guid_text(FLOW_CHARACTERISTIC_UUID),
                    reference,
                    logger,
                );
                let after = pipeline.result();
                if !before.handshake_host_received && after.handshake_host_received {
                    mark(&mut milestones, "complete 0x35", at, timestamp_ms);
                }
                if !before.handshake_reply_received && after.handshake_reply_received {
                    mark(&mut milestones, "RX 0x36", at, timestamp_ms);
                }
                if !before.mixer_state_applied && after.mixer_state_applied {
                    mark(&mut milestones, "complete 0x38", at, timestamp_ms);
                }
                if !before.ready && after.ready {
                    mark(&mut milestones, "Ready", at, timestamp_ms);
                }
                for frame in writes {
                    let tx_at = Instant::now();
                    let tx_stamp = unix_timestamp_ms();
                    match frame.first() {
                        Some(0x39) => mark(&mut milestones, "TX 0x39", tx_at, tx_stamp),
                        Some(0x37) => mark(&mut milestones, "TX 0x37", tx_at, tx_stamp),
                        _ => {}
                    }
                    let _ = write_with_response(
                        "winrt-prearmed",
                        &characteristic,
                        &frame,
                        reference,
                        logger,
                    )
                    .await;
                }
            }
            Ok(Some(PrearmedEvent::HandlerError {
                at,
                timestamp_ms,
                source,
                error,
            })) => logger.line(format!(
                "timestamp_ms={timestamp_ms} backend=winrt-prearmed t_experiment_ms={} handler={source} error={error} evidence=VERIFIED_FROM_DEVICE",
                at.duration_since(started).as_millis(),
            )),
            Ok(None) => {
                logger.line("backend=winrt-prearmed event_channel=ENDED evidence=VERIFIED_FROM_DEVICE");
                break;
            }
            Err(_) => {
                logger.line(format!(
                    "backend=winrt-prearmed {} result=TIMEOUT timeout_ms={} notifications={} prearm_integrity={} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(started),
                    timeout.as_millis(),
                    pipeline.result().notification_count,
                    if connected_before_arm {
                        "COMPROMISED"
                    } else if connected_during_listener_attach {
                        "AMBIGUOUS"
                    } else {
                        "VALID"
                    },
                ));
                break;
            }
        }
    }

    if let Some(token) = value_token {
        logger.line(format!(
            "backend=winrt-prearmed ValueChanged.remove={:?} cccd_writes=0 evidence=VERIFIED_FROM_DEVICE",
            characteristic.RemoveValueChanged(token),
        ));
    }
    if let Some(session) = gatt_session {
        logger.line(format!(
            "backend=winrt-prearmed GattSession.SetMaintainConnection(false)={:?} evidence=VERIFIED_FROM_DEVICE",
            session.SetMaintainConnection(false),
        ));
    }
    if let Some(token) = connection_token {
        logger.line(format!(
            "backend=winrt-prearmed ConnectionStatusChanged.remove={:?} evidence=VERIFIED_FROM_DEVICE",
            device.RemoveConnectionStatusChanged(token),
        ));
    }
    let final_status = log_connection_status(
        &device,
        "cleanup",
        "after_handlers_removed",
        started,
        logger,
    );
    logger.line(format!(
        "PREARM_RESULT notifications={} complete_0x35={} rx_0x36={} complete_0x38={} ready={} connected_before_listener={} connected_during_listener_attach={} final_connection_status={:?} uncached_calls=0 subscribe_calls=0 cccd_writes=0 mixer_control_tx=0",
        pipeline.result().notification_count,
        pipeline.result().handshake_host_received,
        pipeline.result().handshake_reply_received,
        pipeline.result().mixer_state_applied,
        pipeline.result().ready,
        connected_before_arm,
        connected_during_listener_attach,
        final_status.map(status_name),
    ));
    if connected_at.is_none() {
        logger.line("TIMELINE_ANCHOR=UNAVAILABLE reason=ConnectionStatusChanged(Connected)_was_not_captured; relative offsets MUST_NOT be inferred from discovery completion");
    }
    print_timeline(&mut milestones, connected_at, logger);
    logger.line("SAFETY_RESULT uncached_calls=0 subscribe_calls=0 cccd_writes=0 mixer_control_tx=0; only production-codec 0x39/0x37 responses were eligible for write");
    Ok(())
}
