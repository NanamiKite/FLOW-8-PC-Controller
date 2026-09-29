//! FLOW 8 profile adapter for the generic DirectHCI BLE library.
//!
//! DirectHCI owns controller takeover, raw HCI, TrouBLE and GATT lifecycle.
//! This module owns only FLOW-specific discovery policy and forwards owned
//! characteristic values to the existing protocol/session RX channel.

use std::time::Duration;

use directhci_ble::{
    BleAddress, BleCentralConfig, BleConnection, BleUuid, DirectHciBleCentral, GattCharacteristic,
    WriteMode,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tracing::{debug, info, warn};

use super::{
    BleError, CHARACTERISTIC_UUID, DeviceEvent, NativeConnectionStage, RxIngress, SERVICE_UUID,
    TransportRx,
};

const FLOW_NAME: &str = "FLOW 8 LE";
const SCAN_TIMEOUT: Duration = Duration::from_secs(12);

pub(super) struct DirectHciSession {
    connection: Option<BleConnection>,
    characteristic: GattCharacteristic,
    reader: Option<JoinHandle<()>>,
    reader_stop: Option<oneshot::Sender<()>>,
    ingress: RxIngress,
    mtu: u16,
}

impl DirectHciSession {
    pub(super) async fn connect(
        generation: u64,
        events: mpsc::UnboundedSender<DeviceEvent>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportRx>), BleError> {
        let (rx_tx, rx) = mpsc::unbounded_channel();
        let ingress = RxIngress::new(generation, rx_tx);
        let _ = events.send(DeviceEvent::ConnectionStage(
            NativeConnectionStage::Scanning,
        ));

        let central = DirectHciBleCentral::connect(BleCentralConfig {
            controller_id: std::env::var("DIRECTHCI_CONTROLLER_ID").ok(),
            client_name: "flow8-pc-controller".into(),
            client_version: Some(env!("CARGO_PKG_VERSION").into()),
        })
        .await
        .map_err(transport_error("connect DirectHCI BLE runtime"))?;
        let devices = central
            .scan(SCAN_TIMEOUT)
            .await
            .map_err(transport_error("scan with DirectHCI"))?;
        let peer = select_flow8(&devices)?;
        let _ = events.send(DeviceEvent::ConnectionStage(
            NativeConnectionStage::DeviceFound,
        ));
        info!(
            backend = "directhci",
            device_name = FLOW_NAME,
            address = %peer,
            "FLOW 8 LE found"
        );

        let connection = central
            .connect_device(peer)
            .await
            .map_err(transport_error("connect FLOW 8 through DirectHCI"))?;
        let mtu = connection.info().att_mtu;
        ingress.connected();
        info!(backend = "directhci", mtu, "FLOW 8 BLE connected");

        let services = connection
            .discover()
            .await
            .map_err(transport_error("discover FLOW 8 GATT database"))?;
        let service_uuid = BleUuid::Uuid128(SERVICE_UUID.as_u128());
        let characteristic_uuid = BleUuid::Uuid128(CHARACTERISTIC_UUID.as_u128());
        let service = services
            .into_iter()
            .find(|service| service.uuid == service_uuid)
            .ok_or_else(|| {
                BleError::Transport(format!(
                    "DirectHCI did not discover FLOW service {SERVICE_UUID}"
                ))
            })?;
        let _ = events.send(DeviceEvent::ConnectionStage(
            NativeConnectionStage::FlowServiceSelected,
        ));
        let characteristic = service
            .characteristics
            .into_iter()
            .find(|characteristic| characteristic.uuid == characteristic_uuid)
            .ok_or_else(|| {
                BleError::Transport(format!(
                    "DirectHCI did not discover FLOW characteristic {CHARACTERISTIC_UUID} in service {SERVICE_UUID}"
                ))
            })?;
        let _ = events.send(DeviceEvent::ConnectionStage(
            NativeConnectionStage::TargetCharacteristicFound,
        ));

        // FLOW 8 has Notify set but no CCCD. DirectHCI's generic passive
        // listener registers for the value handle and deliberately performs no
        // subscribe/unsubscribe or descriptor write.
        let mut notifications = connection
            .listen(&characteristic)
            .await
            .map_err(transport_error("arm DirectHCI passive listener"))?;
        let _ = events.send(DeviceEvent::ConnectionStage(
            NativeConnectionStage::NativeRxArmed,
        ));
        info!(
            backend = "directhci",
            mtu,
            service = %SERVICE_UUID,
            characteristic = %CHARACTERISTIC_UUID,
            value_handle = characteristic.value_handle,
            cccd_handle = ?characteristic.cccd_handle,
            "FLOW 8 DirectHCI passive RX armed"
        );

        let reader_ingress = ingress.clone();
        let (reader_stop, mut stop_requested) = oneshot::channel();
        let reader = tokio::spawn(async move {
            let close_gracefully = loop {
                tokio::select! {
                    _ = &mut stop_requested => break true,
                    received = notifications.recv() => {
                        match received {
                            Ok(Some(event)) => {
                                debug!(
                                    backend = "directhci",
                                    handle = event.handle,
                                    bytes = event.value.len(),
                                    "FLOW characteristic notification"
                                );
                                if !reader_ingress.forward(&event.value) {
                                    break true;
                                }
                            }
                            Ok(None) => {
                                info!(
                                    backend = "directhci",
                                    "DirectHCI notification stream closed"
                                );
                                reader_ingress.disconnected();
                                break false;
                            }
                            Err(error) => {
                                warn!(backend = "directhci", %error, "DirectHCI notification stream failed");
                                reader_ingress.error(format!("DirectHCI notification stream: {error}"));
                                reader_ingress.disconnected();
                                break false;
                            }
                        }
                    }
                }
            };
            if close_gracefully && let Err(error) = notifications.close().await {
                warn!(backend = "directhci", %error, "DirectHCI passive listener close failed");
                reader_ingress.error(format!("DirectHCI listener close: {error}"));
            }
        });

        Ok((
            Self {
                connection: Some(connection),
                characteristic,
                reader: Some(reader),
                reader_stop: Some(reader_stop),
                ingress,
                mtu,
            },
            rx,
        ))
    }

    pub(super) fn mtu(&self) -> u16 {
        self.mtu
    }

    pub(super) async fn write(&self, frame: &[u8]) -> Result<(), BleError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| BleError::Transport("DirectHCI connection is closed".into()))?;
        if frame.first() == Some(&0x39) {
            println!(
                "TX_HEX={}",
                frame
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
        }
        connection
            .write(
                &self.characteristic,
                frame.to_vec(),
                WriteMode::WithResponse,
            )
            .await
            .map_err(transport_error(
                "write FLOW characteristic through DirectHCI",
            ))
    }

    pub(super) fn mark_handshake_rx(&self) {}

    pub(super) async fn disconnect(&mut self) -> Result<(), BleError> {
        self.ingress.invalidate();
        if let Some(stop) = self.reader_stop.take() {
            let _ = stop.send(());
        }
        let reader_result = if let Some(reader) = self.reader.take() {
            reader.await.map_err(|error| {
                BleError::Transport(format!("join DirectHCI listener task: {error}"))
            })
        } else {
            Ok(())
        };
        let disconnect_result = if let Some(connection) = self.connection.take() {
            connection
                .disconnect()
                .await
                .map_err(transport_error("disconnect DirectHCI FLOW session"))
        } else {
            Ok(())
        };
        reader_result?;
        disconnect_result
    }
}

impl Drop for DirectHciSession {
    fn drop(&mut self) {
        self.ingress.invalidate();
        if let Some(stop) = self.reader_stop.take() {
            let _ = stop.send(());
        }
    }
}

fn select_flow8(devices: &[directhci_ble::ScanResult]) -> Result<BleAddress, BleError> {
    let requested = std::env::var("FLOW8_DIRECTHCI_ADDRESS")
        .ok()
        .map(|value| value.parse::<BleAddress>())
        .transpose()
        .map_err(|error| BleError::Transport(format!("FLOW8_DIRECTHCI_ADDRESS: {error}")))?;
    let mut matches = devices.iter().filter(|device| {
        device.local_name.as_deref() == Some(FLOW_NAME)
            && requested.is_none_or(|address| device.address == address)
    });
    let first = matches
        .next()
        .ok_or_else(|| BleError::Transport("FLOW 8 LE was not found by DirectHCI scan".into()))?;
    if matches.next().is_some() {
        return Err(BleError::Transport(
            "multiple FLOW 8 LE devices were found; set FLOW8_DIRECTHCI_ADDRESS".into(),
        ));
    }
    Ok(first.address)
}

fn transport_error(operation: &'static str) -> impl FnOnce(directhci_ble::BleError) -> BleError {
    move |error| BleError::Transport(format!("{operation}: {error}"))
}
