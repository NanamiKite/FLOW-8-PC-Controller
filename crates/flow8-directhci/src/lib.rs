//! Thin FLOW 8 profile adapter over the generic `directhci-ble` SDK. The SDK talks through `directhci-client` to the separately running `directhcid`; no daemon, driver, raw-HCI, or recovery implementation lives in this crate.
//!
//! This crate selects the FLOW 8 advertisement and GATT endpoint, chooses the
//! device-specific passive-listen behavior, and exposes raw characteristic
//! values. It contains no FLOW packet parser, handshake, fragmentation, or
//! mixer-state logic.

use std::time::Duration;

use directhci_ble::{
    BleAddress, BleCentralConfig, BleConnection, BleUuid, DirectHciBleCentral, GattCharacteristic,
    GattNotificationStream, WriteMode,
};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tracing::{debug, info, warn};

pub const FLOW_DEVICE_NAME: &str = "FLOW 8 LE";
pub const FLOW_SERVICE_UUID: BleUuid = BleUuid::Uuid128(0x14839ad4_8d7e_415c_9a42_167340cf2339);
pub const FLOW_CHARACTERISTIC_UUID: BleUuid =
    BleUuid::Uuid128(0x0034594a_a8e7_4b1a_a6b1_cd5243059a57);

const COMMAND_DEPTH: usize = 16;
const EVENT_DEPTH: usize = 64;

#[derive(Clone, Debug)]
pub struct Flow8DirectHciConfig {
    pub controller_id: Option<String>,
    pub preferred_address: Option<String>,
    pub scan_timeout: Duration,
    pub client_name: String,
    pub client_version: Option<String>,
}

impl Default for Flow8DirectHciConfig {
    fn default() -> Self {
        Self {
            controller_id: None,
            preferred_address: None,
            scan_timeout: Duration::from_secs(12),
            client_name: "flow8-pc-controller".into(),
            client_version: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flow8DirectHciStage {
    Scanning,
    DeviceFound,
    Connected,
    ServiceFound,
    CharacteristicFound,
    ListenerReady,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Flow8DirectHciInfo {
    pub peer: BleAddress,
    pub att_mtu: u16,
    pub value_handle: u16,
    pub cccd_handle: Option<u16>,
}

#[derive(Debug)]
pub enum Flow8DirectHciEvent {
    Notification(Vec<u8>),
    Disconnected,
    Error(String),
}

#[derive(Debug, Error)]
pub enum Flow8DirectHciError {
    #[error("{operation}: {source}")]
    DirectHci {
        operation: &'static str,
        #[source]
        source: directhci_ble::BleError,
    },
    #[error("FLOW 8 LE was not found during DirectHCI scan")]
    DeviceNotFound,
    #[error("multiple FLOW 8 LE devices were found; configure a preferred address")]
    AmbiguousDevice,
    #[error("invalid preferred FLOW 8 address: {0}")]
    InvalidAddress(String),
    #[error("FLOW service {FLOW_SERVICE_UUID} was not discovered")]
    ServiceNotFound,
    #[error(
        "FLOW characteristic {FLOW_CHARACTERISTIC_UUID} was not discovered in service {FLOW_SERVICE_UUID}"
    )]
    CharacteristicNotFound,
    #[error("DirectHCI transport worker stopped")]
    WorkerStopped,
    #[error("DirectHCI transport worker panicked: {0}")]
    WorkerPanicked(String),
}

impl Flow8DirectHciError {
    fn directhci(operation: &'static str, source: directhci_ble::BleError) -> Self {
        Self::DirectHci { operation, source }
    }
}

enum TransportCommand {
    Write {
        value: Vec<u8>,
        reply: oneshot::Sender<Result<(), Flow8DirectHciError>>,
    },
    Disconnect {
        source: &'static str,
        reply: Option<oneshot::Sender<Result<(), Flow8DirectHciError>>>,
    },
}

pub struct DirectHciTransport {
    commands: mpsc::Sender<TransportCommand>,
    worker: Option<JoinHandle<()>>,
    info: Flow8DirectHciInfo,
    disconnect_requested: bool,
}

impl DirectHciTransport {
    /// Connects, performs generic GATT discovery, and arms the passive FLOW
    /// listener before returning. No FLOW protocol bytes are sent here.
    pub async fn connect(
        config: Flow8DirectHciConfig,
        mut on_stage: impl FnMut(Flow8DirectHciStage),
    ) -> Result<(Self, mpsc::Receiver<Flow8DirectHciEvent>), Flow8DirectHciError> {
        let central = DirectHciBleCentral::connect(BleCentralConfig {
            controller_id: config.controller_id,
            client_name: config.client_name,
            client_version: config.client_version,
        })
        .await
        .map_err(|error| Flow8DirectHciError::directhci("connect DirectHCI runtime", error))?;

        on_stage(Flow8DirectHciStage::Scanning);
        let devices = match central.scan(config.scan_timeout).await {
            Ok(devices) => devices,
            Err(error) => {
                return Err(shutdown_central_after_error(
                    central,
                    Flow8DirectHciError::directhci("scan for FLOW 8 LE", error),
                )
                .await);
            }
        };
        let peer = match select_flow8(&devices, config.preferred_address.as_deref()) {
            Ok(peer) => peer,
            Err(error) => return Err(shutdown_central_after_error(central, error).await),
        };
        on_stage(Flow8DirectHciStage::DeviceFound);
        info!(address = %peer, device_name = FLOW_DEVICE_NAME, "FLOW 8 LE selected");

        let connection = central.connect_device(peer).await.map_err(|error| {
            Flow8DirectHciError::directhci("connect FLOW 8 LE through DirectHCI", error)
        })?;
        on_stage(Flow8DirectHciStage::Connected);
        let info = Flow8DirectHciInfo {
            peer,
            att_mtu: connection.info().att_mtu,
            value_handle: 0,
            cccd_handle: None,
        };

        let services = match connection.discover().await {
            Ok(services) => services,
            Err(error) => {
                return Err(disconnect_after_setup_error(
                    connection,
                    Flow8DirectHciError::directhci("discover FLOW 8 GATT database", error),
                )
                .await);
            }
        };
        let service = match services
            .into_iter()
            .find(|service| service.uuid == FLOW_SERVICE_UUID)
        {
            Some(service) => service,
            None => {
                return Err(disconnect_after_setup_error(
                    connection,
                    Flow8DirectHciError::ServiceNotFound,
                )
                .await);
            }
        };
        on_stage(Flow8DirectHciStage::ServiceFound);
        let characteristic = match service
            .characteristics
            .into_iter()
            .find(|characteristic| characteristic.uuid == FLOW_CHARACTERISTIC_UUID)
        {
            Some(characteristic) => characteristic,
            None => {
                return Err(disconnect_after_setup_error(
                    connection,
                    Flow8DirectHciError::CharacteristicNotFound,
                )
                .await);
            }
        };
        on_stage(Flow8DirectHciStage::CharacteristicFound);

        // FLOW 8 advertises Notify but exposes no CCCD. `listen` is the
        // DirectHCI passive-listener API and is fully armed when it returns.
        let notifications = match connection.listen(&characteristic).await {
            Ok(notifications) => notifications,
            Err(error) => {
                return Err(disconnect_after_setup_error(
                    connection,
                    Flow8DirectHciError::directhci("arm FLOW 8 passive listener", error),
                )
                .await);
            }
        };
        on_stage(Flow8DirectHciStage::ListenerReady);

        let info = Flow8DirectHciInfo {
            value_handle: characteristic.value_handle,
            cccd_handle: characteristic.cccd_handle,
            ..info
        };
        info!(
            address = %info.peer,
            att_mtu = info.att_mtu,
            service = %FLOW_SERVICE_UUID,
            characteristic = %FLOW_CHARACTERISTIC_UUID,
            value_handle = info.value_handle,
            cccd_handle = ?info.cccd_handle,
            "FLOW 8 DirectHCI transport ready"
        );

        let (commands_tx, commands_rx) = mpsc::channel(COMMAND_DEPTH);
        let (events_tx, events_rx) = mpsc::channel(EVENT_DEPTH);
        let worker = tokio::spawn(transport_loop(
            connection,
            characteristic,
            notifications,
            commands_rx,
            events_tx,
        ));
        Ok((
            Self {
                commands: commands_tx,
                worker: Some(worker),
                info,
                disconnect_requested: false,
            },
            events_rx,
        ))
    }

    pub fn info(&self) -> &Flow8DirectHciInfo {
        &self.info
    }

    pub async fn write(&self, value: &[u8]) -> Result<(), Flow8DirectHciError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.commands
            .send(TransportCommand::Write {
                value: value.to_vec(),
                reply: reply_tx,
            })
            .await
            .map_err(|_| Flow8DirectHciError::WorkerStopped)?;
        reply_rx
            .await
            .map_err(|_| Flow8DirectHciError::WorkerStopped)?
    }

    pub async fn disconnect(self) -> Result<(), Flow8DirectHciError> {
        self.disconnect_with_reason("explicit_disconnect").await
    }

    pub async fn disconnect_with_reason(
        mut self,
        source: &'static str,
    ) -> Result<(), Flow8DirectHciError> {
        self.disconnect_inner(source).await
    }

    async fn disconnect_inner(&mut self, source: &'static str) -> Result<(), Flow8DirectHciError> {
        if self.disconnect_requested {
            return Ok(());
        }
        self.disconnect_requested = true;
        info!(source, "DirectHCI disconnect requested");
        let (reply_tx, reply_rx) = oneshot::channel();
        let sent = self
            .commands
            .send(TransportCommand::Disconnect {
                source,
                reply: Some(reply_tx),
            })
            .await
            .is_ok();
        let result = if sent {
            reply_rx
                .await
                .map_err(|_| Flow8DirectHciError::WorkerStopped)?
        } else {
            Ok(())
        };
        if let Some(worker) = self.worker.take()
            && let Err(error) = worker.await
        {
            return Err(Flow8DirectHciError::WorkerPanicked(error.to_string()));
        }
        result
    }
}

impl Drop for DirectHciTransport {
    fn drop(&mut self) {
        if self.disconnect_requested {
            return;
        }
        self.disconnect_requested = true;
        warn!(
            source = "runtime_session_drop",
            "DirectHCI disconnect requested"
        );
        let _ = self.commands.try_send(TransportCommand::Disconnect {
            source: "runtime_session_drop",
            reply: None,
        });
    }
}

async fn transport_loop(
    connection: BleConnection,
    characteristic: GattCharacteristic,
    mut notifications: GattNotificationStream,
    mut commands: mpsc::Receiver<TransportCommand>,
    events: mpsc::Sender<Flow8DirectHciEvent>,
) {
    loop {
        tokio::select! {
            biased;
            command = commands.recv() => {
                match command {
                    Some(TransportCommand::Write { value, reply }) => {
                        let byte_count = value.len();
                        info!(bytes = byte_count, "DirectHCI write begin");
                        let result = connection
                            .write(&characteristic, value, WriteMode::WithResponse)
                            .await
                            .map_err(|error| Flow8DirectHciError::directhci(
                                "write FLOW 8 characteristic",
                                error,
                            ));
                        match &result {
                            Ok(()) => info!(bytes = byte_count, "DirectHCI write complete"),
                            Err(error) => {
                                warn!(bytes = byte_count, %error, "DirectHCI write failed");
                                let _ = events
                                    .send(Flow8DirectHciEvent::Error(error.to_string()))
                                    .await;
                            }
                        }
                        let _ = reply.send(result);
                    }
                    Some(TransportCommand::Disconnect { source, reply }) => {
                        let result = close_and_disconnect(notifications, connection, source).await;
                        if let Some(reply) = reply {
                            let _ = reply.send(result);
                        }
                        let _ = events.send(Flow8DirectHciEvent::Disconnected).await;
                        return;
                    }
                    None => {
                        let source = "transport_command_channel_closed";
                        let _ = close_and_disconnect(notifications, connection, source).await;
                        return;
                    }
                }
            }
            received = notifications.recv() => {
                match received {
                    Ok(Some(event)) if event.handle == characteristic.value_handle => {
                        debug!(
                            handle = event.handle,
                            bytes = event.value.len(),
                            "forwarding FLOW 8 characteristic value"
                        );
                        if events
                            .send(Flow8DirectHciEvent::Notification(event.value))
                            .await
                            .is_err()
                        {
                            let source = "rx_event_consumer_closed";
                            let _ = close_and_disconnect(notifications, connection, source).await;
                            return;
                        }
                    }
                    Ok(Some(event)) => {
                        warn!(
                            handle = event.handle,
                            expected_handle = characteristic.value_handle,
                            "ignored notification from unexpected handle"
                        );
                    }
                    Ok(None) => {
                        let source = "ble_peer_disconnected_or_sdk_stream_ended";
                        warn!(source, "DirectHCI session shutdown begin");
                        let _ = events.send(Flow8DirectHciEvent::Disconnected).await;
                        let result = connection.disconnect().await;
                        info!(source, success = result.is_ok(), "DirectHCI session shutdown complete");
                        return;
                    }
                    Err(error) => {
                        let source = "transport_error";
                        warn!(source, %error, "DirectHCI session shutdown begin");
                        let message = format!("DirectHCI notification stream: {error}");
                        let _ = events.send(Flow8DirectHciEvent::Error(message)).await;
                        let _ = events.send(Flow8DirectHciEvent::Disconnected).await;
                        let result = connection.disconnect().await;
                        info!(source, success = result.is_ok(), "DirectHCI session shutdown complete");
                        return;
                    }
                }
            }
        }
    }
}

async fn close_and_disconnect(
    notifications: GattNotificationStream,
    connection: BleConnection,
    reason: &'static str,
) -> Result<(), Flow8DirectHciError> {
    info!(reason, "DirectHCI session shutdown begin");
    let close = notifications
        .close()
        .await
        .map_err(|error| Flow8DirectHciError::directhci("close FLOW 8 passive listener", error));
    let disconnect = connection
        .disconnect()
        .await
        .map_err(|error| Flow8DirectHciError::directhci("disconnect FLOW 8", error));
    close?;
    disconnect?;
    info!(reason, "DirectHCI session shutdown complete");
    Ok(())
}

async fn shutdown_central_after_error(
    central: DirectHciBleCentral,
    primary: Flow8DirectHciError,
) -> Flow8DirectHciError {
    if let Err(cleanup) = central.shutdown().await {
        warn!(%cleanup, %primary, "DirectHCI cleanup failed after FLOW setup error");
    }
    primary
}

async fn disconnect_after_setup_error(
    connection: BleConnection,
    primary: Flow8DirectHciError,
) -> Flow8DirectHciError {
    if let Err(cleanup) = connection.disconnect().await {
        warn!(%cleanup, %primary, "DirectHCI disconnect failed after FLOW setup error");
    }
    primary
}

fn select_flow8(
    devices: &[directhci_ble::ScanResult],
    preferred_address: Option<&str>,
) -> Result<BleAddress, Flow8DirectHciError> {
    let preferred = preferred_address
        .map(str::parse::<BleAddress>)
        .transpose()
        .map_err(|error| Flow8DirectHciError::InvalidAddress(error.to_string()))?;
    let mut matches = devices.iter().filter(|device| {
        device.local_name.as_deref() == Some(FLOW_DEVICE_NAME)
            && preferred.is_none_or(|address| device.address == address)
    });
    let first = matches.next().ok_or(Flow8DirectHciError::DeviceNotFound)?;
    if matches.next().is_some() {
        return Err(Flow8DirectHciError::AmbiguousDevice);
    }
    Ok(first.address)
}
