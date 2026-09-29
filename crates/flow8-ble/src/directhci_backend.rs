//! Runtime bridge from the generic FLOW BLE session to `flow8-directhci`.
//!
//! FLOW target selection and passive-listener lifecycle live in the thin
//! adapter crate. Packet parsing, handshake decisions, reassembly, and Store
//! mutation remain in the existing `flow8-ble` runtime and upper layers.

use std::time::Duration;

use flow8_directhci::{
    DirectHciTransport, Flow8DirectHciConfig, Flow8DirectHciEvent, Flow8DirectHciStage,
};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{debug, info, warn};

use super::{BleError, DeviceEvent, NativeConnectionStage, RxIngress, TransportRx};

const SCAN_TIMEOUT: Duration = Duration::from_secs(12);

pub(super) struct DirectHciSession {
    transport: Option<DirectHciTransport>,
    reader: Option<JoinHandle<()>>,
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
        let stage_events = events.clone();
        let config = Flow8DirectHciConfig {
            controller_id: std::env::var("DIRECTHCI_CONTROLLER_ID").ok(),
            preferred_address: std::env::var("FLOW8_DIRECTHCI_ADDRESS").ok(),
            scan_timeout: SCAN_TIMEOUT,
            client_name: "flow8-pc-controller".into(),
            client_version: Some(env!("CARGO_PKG_VERSION").into()),
        };
        let (transport, mut direct_events) = DirectHciTransport::connect(config, move |stage| {
            let _ = stage_events.send(DeviceEvent::ConnectionStage(map_stage(stage)));
        })
        .await
        .map_err(|error| BleError::Transport(format!("DirectHCI FLOW transport: {error}")))?;

        let info = transport.info().clone();
        ingress.connected();
        info!(
            backend = "directhci",
            address = %info.peer,
            mtu = info.att_mtu,
            value_handle = info.value_handle,
            cccd_handle = ?info.cccd_handle,
            "FLOW 8 DirectHCI transport connected and passive RX armed"
        );

        let reader_ingress = ingress.clone();
        let reader = tokio::spawn(async move {
            while let Some(event) = direct_events.recv().await {
                match event {
                    Flow8DirectHciEvent::Notification(value) => {
                        debug!(
                            backend = "directhci",
                            bytes = value.len(),
                            "forwarding FLOW 8 characteristic value"
                        );
                        if !reader_ingress.forward(&value) {
                            break;
                        }
                    }
                    Flow8DirectHciEvent::Error(message) => {
                        warn!(backend = "directhci", %message, "DirectHCI transport error");
                        reader_ingress.error(message);
                    }
                    Flow8DirectHciEvent::Disconnected => {
                        info!(backend = "directhci", "DirectHCI FLOW session disconnected");
                        reader_ingress.disconnected();
                        break;
                    }
                }
            }
        });

        Ok((
            Self {
                transport: Some(transport),
                reader: Some(reader),
                ingress,
                mtu: info.att_mtu,
            },
            rx,
        ))
    }

    pub(super) fn mtu(&self) -> u16 {
        self.mtu
    }

    pub(super) async fn write(&self, frame: &[u8]) -> Result<(), BleError> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| BleError::Transport("DirectHCI connection is closed".into()))?;
        transport
            .write(frame)
            .await
            .map_err(|error| BleError::Transport(format!("DirectHCI FLOW write: {error}")))
    }

    pub(super) fn mark_handshake_rx(&self) {}

    pub(super) async fn disconnect(&mut self) -> Result<(), BleError> {
        self.ingress.invalidate();
        let disconnect_result = if let Some(transport) = self.transport.take() {
            transport
                .disconnect()
                .await
                .map_err(|error| BleError::Transport(format!("DirectHCI disconnect: {error}")))
        } else {
            Ok(())
        };
        if let Some(reader) = self.reader.take() {
            reader
                .await
                .map_err(|error| BleError::Transport(format!("join DirectHCI RX task: {error}")))?;
        }
        disconnect_result
    }
}

impl Drop for DirectHciSession {
    fn drop(&mut self) {
        self.ingress.invalidate();
        if let Some(reader) = self.reader.take() {
            reader.abort();
        }
        // Dropping DirectHciTransport sends a best-effort disconnect to
        // its worker. Normal runtime shutdown always uses `disconnect` above.
    }
}

fn map_stage(stage: Flow8DirectHciStage) -> NativeConnectionStage {
    match stage {
        Flow8DirectHciStage::Scanning => NativeConnectionStage::Scanning,
        Flow8DirectHciStage::DeviceFound => NativeConnectionStage::DeviceFound,
        Flow8DirectHciStage::Connected => NativeConnectionStage::DeviceObjectCreated,
        Flow8DirectHciStage::ServiceFound => NativeConnectionStage::FlowServiceSelected,
        Flow8DirectHciStage::CharacteristicFound => {
            NativeConnectionStage::TargetCharacteristicFound
        }
        Flow8DirectHciStage::ListenerReady => NativeConnectionStage::NativeRxArmed,
    }
}
