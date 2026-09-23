//! btleplug transport boundary. The GUI does not depend on an adapter being
//! present; callers run these futures on a separate Tokio runtime.

use std::{
    collections::BTreeSet,
    fmt,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(not(target_os = "windows"))]
use btleplug::api::CharPropFlags;
use btleplug::{
    api::{
        Central, Characteristic, Descriptor, Manager as _, Peripheral as _, ScanFilter, Service,
        ValueNotification, WriteType,
    },
    platform::{Adapter, Manager, Peripheral},
};
use flow8_protocol::{CommandStreamDecoder, RxCommand, TxCommand, encode_frames};
use futures_util::Stream;
#[cfg(not(target_os = "windows"))]
use futures_util::StreamExt;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, info, trace};
use uuid::Uuid;

#[cfg(target_os = "windows")]
mod windows_native;

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x14839ad4_8d7e_415c_9a42_167340cf2339);
pub const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x0034594a_a8e7_4b1a_a6b1_cd5243059a57);

/// Owned messages crossing the platform callback/stream boundary. Native
/// callbacks never parse FLOW protocol data; they only copy bytes into this
/// channel and return.
#[derive(Debug)]
enum TransportRx {
    Packet {
        generation: u64,
        bytes: Vec<u8>,
    },
    #[cfg(target_os = "windows")]
    Connected {
        generation: u64,
    },
    Disconnected {
        generation: u64,
    },
    #[cfg(target_os = "windows")]
    Error {
        generation: u64,
        message: String,
    },
}

/// Session-generation gate shared by platform receive callbacks. Invalidating
/// a connection prevents late callbacks from an old native registration from
/// entering the current protocol session.
#[derive(Clone)]
struct RxIngress {
    generation: u64,
    active: Arc<AtomicBool>,
    tx: mpsc::UnboundedSender<TransportRx>,
}

impl RxIngress {
    fn new(generation: u64, tx: mpsc::UnboundedSender<TransportRx>) -> Self {
        Self {
            generation,
            active: Arc::new(AtomicBool::new(true)),
            tx,
        }
    }

    fn forward(&self, source: &[u8]) -> bool {
        if !self.active.load(Ordering::Acquire) {
            return false;
        }
        self.tx
            .send(TransportRx::Packet {
                generation: self.generation,
                bytes: source.to_vec(),
            })
            .is_ok()
    }

    #[cfg(target_os = "windows")]
    fn connected(&self) -> bool {
        self.send(TransportRx::Connected {
            generation: self.generation,
        })
    }

    fn disconnected(&self) -> bool {
        self.send(TransportRx::Disconnected {
            generation: self.generation,
        })
    }

    #[cfg(target_os = "windows")]
    fn error(&self, message: impl Into<String>) -> bool {
        self.send(TransportRx::Error {
            generation: self.generation,
            message: message.into(),
        })
    }

    fn send(&self, event: TransportRx) -> bool {
        self.active.load(Ordering::Acquire) && self.tx.send(event).is_ok()
    }

    fn invalidate(&self) {
        self.active.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Disconnected,
    Scanning,
    Connecting,
    GattReady,
    RxArming,
    Handshaking,
    StateSyncing,
    Ready,
    Error,
}

/// Fine-grained production connection stages. These are intentionally more
/// precise than the platform-neutral session phases so a native Windows
/// failure cannot disappear into a generic reconnect loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeConnectionStage {
    Scanning,
    DeviceFound,
    DeviceObjectCreated,
    NativeInterfaceEnumerating,
    NativeServiceHandleOpened,
    CharacteristicsEnumerated,
    TargetCharacteristicFound,
    NativeRxRegistering,
    NativeRxArmed,
    Handshaking,
}

impl NativeConnectionStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scanning => "Scanning",
            Self::DeviceFound => "DeviceFound",
            Self::DeviceObjectCreated => "DeviceObjectCreated",
            Self::NativeInterfaceEnumerating => "NativeInterfaceEnumerating",
            Self::NativeServiceHandleOpened => "NativeServiceHandleOpened",
            Self::CharacteristicsEnumerated => "CharacteristicsEnumerated",
            Self::TargetCharacteristicFound => "TargetCharacteristicFound",
            Self::NativeRxRegistering => "NativeRxRegistering",
            Self::NativeRxArmed => "NativeRxArmed",
            Self::Handshaking => "Handshaking",
        }
    }
}

/// Structured native failure retained across reconnect attempts and surfaced
/// verbatim in the GUI. The code is the HRESULT representation of the Win32
/// or WinRT error when Windows supplied one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeConnectionError {
    pub stage: NativeConnectionStage,
    pub api: &'static str,
    /// Stable application-level code for failures that do not originate from
    /// a failing Windows HRESULT (for example, insufficient optional PnP
    /// identity). Never synthesize an HRESULT for these failures.
    pub logical_code: Option<&'static str>,
    pub code: Option<u32>,
    pub retryable: bool,
    pub message: String,
}

impl fmt::Display for NativeConnectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "stage={} api={} logical_code={} code={} retryable={} message={}",
            self.stage.as_str(),
            self.api,
            self.logical_code.unwrap_or("none"),
            self.code
                .map(|value| format!("0x{value:08X}"))
                .unwrap_or_else(|| "unavailable".into()),
            self.retryable,
            self.message
        )
    }
}

impl std::error::Error for NativeConnectionError {}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionAction {
    Phase(SessionPhase),
    Send(Vec<u8>),
    Received(RxCommand),
    Error(String),
}

#[derive(Debug)]
pub enum DeviceCommand {
    Scan { duration: Duration },
    Connect,
    Disconnect,
    Send(TxCommand),
    StateApplied,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Phase(SessionPhase),
    ConnectionStage(NativeConnectionStage),
    ScanResults(Vec<DiscoveredDevice>),
    RawRx(Vec<u8>),
    RawTx(Vec<u8>),
    Received(RxCommand),
    WriteMode(WriteType),
    Backend(&'static str),
    Mtu(u16),
    Error(String),
}

pub struct DeviceRuntime {
    command_tx: mpsc::UnboundedSender<DeviceCommand>,
    event_rx: mpsc::UnboundedReceiver<DeviceEvent>,
}

impl DeviceRuntime {
    /// Starts the Tokio/BLE worker on its own thread. Constructing this handle
    /// does not touch a Bluetooth adapter, so simulator-only GUI startup works
    /// on hosts without Bluetooth.
    pub fn spawn(client_id: [u8; 16]) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("flow8-ble-runtime".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(1)
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(device_actor(command_rx, event_tx, client_id)),
                    Err(error) => {
                        let _ = event_tx.send(DeviceEvent::Error(error.to_string()));
                    }
                }
            })
            .expect("spawn FLOW 8 BLE runtime thread");
        Self {
            command_tx,
            event_rx,
        }
    }

    pub fn send(&self, command: DeviceCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|_| "BLE runtime stopped".to_owned())
    }

    pub fn try_recv(&mut self) -> Option<DeviceEvent> {
        self.event_rx.try_recv().ok()
    }
}

/// Deterministic protocol/session coordinator. It contains no Bluetooth API
/// calls and is therefore tested with the same captured frames on Linux and
/// Windows. The transport executes `Send` actions one frame at a time.
pub struct Flow8Session {
    phase: SessionPhase,
    client_id: [u8; 16],
    sequence: u8,
    decoder: CommandStreamDecoder,
    awaiting_state_apply: bool,
}

impl Flow8Session {
    pub fn new(client_id: [u8; 16]) -> Self {
        Self {
            phase: SessionPhase::Disconnected,
            client_id,
            sequence: 0,
            decoder: CommandStreamDecoder::default(),
            awaiting_state_apply: false,
        }
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    pub fn transition(&mut self, phase: SessionPhase) -> SessionAction {
        self.phase = phase;
        if phase == SessionPhase::Disconnected {
            self.decoder.clear();
            self.sequence = 0;
            self.awaiting_state_apply = false;
        } else if phase == SessionPhase::StateSyncing {
            self.awaiting_state_apply = false;
        }
        SessionAction::Phase(phase)
    }

    /// Marks the platform receive path as armed. On standard BLE backends this
    /// follows subscription; on FLOW 8/Windows it follows native event
    /// registration and never implies a CCCD write.
    pub fn rx_armed(&mut self) -> SessionAction {
        self.transition(SessionPhase::Handshaking)
    }

    /// Completes the Ready gate only after the consumer has atomically
    /// applied the decoded composite state to its authoritative Store.
    pub fn state_applied(&mut self) -> Result<SessionAction, BleError> {
        if self.phase != SessionPhase::StateSyncing || !self.awaiting_state_apply {
            return Err(BleError::UnexpectedSessionState(
                "state apply acknowledgment without a decoded 0x38".into(),
            ));
        }
        self.awaiting_state_apply = false;
        Ok(self.transition(SessionPhase::Ready))
    }

    pub fn encode_command(&mut self, command: &TxCommand) -> Result<Vec<SessionAction>, BleError> {
        let frames = encode_frames(command, flow8_protocol::MAX_RAW_PACKET_SIZE, self.sequence)
            .map_err(BleError::Protocol)?;
        self.sequence = self.sequence.wrapping_add(1) & 0x03;
        Ok(frames.into_iter().map(SessionAction::Send).collect())
    }

    pub fn notification(&mut self, raw: &[u8]) -> Vec<SessionAction> {
        match self.decoder.accept(raw) {
            Ok(None) => Vec::new(),
            Err(error) => vec![SessionAction::Error(error.to_string())],
            Ok(Some(command)) => {
                let mut actions = vec![SessionAction::Received(command.clone())];
                match command {
                    RxCommand::HandshakeHost { .. } => {
                        actions.push(self.transition(SessionPhase::Handshaking));
                        match self.encode_command(&TxCommand::HandshakeClient {
                            client_id: self.client_id,
                        }) {
                            Ok(mut sends) => actions.append(&mut sends),
                            Err(error) => actions.push(SessionAction::Error(error.to_string())),
                        }
                    }
                    RxCommand::HandshakeReply => {
                        actions.push(self.transition(SessionPhase::StateSyncing));
                        match self.encode_command(&TxCommand::GetMixerState) {
                            Ok(mut sends) => actions.append(&mut sends),
                            Err(error) => actions.push(SessionAction::Error(error.to_string())),
                        }
                    }
                    RxCommand::MixerState(_) => {
                        self.awaiting_state_apply = true;
                    }
                    RxCommand::SnapshotLoad { .. } | RxCommand::FactoryReset => {
                        actions.push(self.transition(SessionPhase::StateSyncing));
                        match self.encode_command(&TxCommand::GetMixerState) {
                            Ok(mut sends) => actions.append(&mut sends),
                            Err(error) => actions.push(SessionAction::Error(error.to_string())),
                        }
                    }
                    _ => {}
                }
                actions
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum BleError {
    #[error(transparent)]
    Backend(#[from] btleplug::Error),
    #[error("no Bluetooth adapter is available")]
    NoAdapter,
    #[error("FLOW 8 was not found during the scan")]
    DeviceNotFound,
    #[error("the FLOW 8 characteristic was not discovered")]
    CharacteristicNotFound,
    #[error("Windows native FLOW 8 transport failed: {0}")]
    NativeWindows(#[from] NativeConnectionError),
    #[error("unexpected FLOW 8 session state: {0}")]
    UnexpectedSessionState(String),
    #[error(transparent)]
    Protocol(#[from] flow8_protocol::ProtocolError),
}

/// FLOW 8's real Windows transport does not expose a CCCD. This invariant is
/// public so integration tests and frontends can verify that Windows runtime
/// policy never regresses to standard subscription.
pub const WINDOWS_FLOW8_USES_STANDARD_SUBSCRIBE: bool = false;

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
    pub rssi: Option<i16>,
    pub services: Vec<Uuid>,
}

pub struct BleTransport {
    adapter: Adapter,
}

impl BleTransport {
    pub async fn new() -> Result<Self, BleError> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .next()
            .ok_or(BleError::NoAdapter)?;
        Ok(Self { adapter })
    }

    pub async fn scan(&self, duration: Duration) -> Result<Vec<DiscoveredDevice>, BleError> {
        self.adapter.start_scan(ScanFilter::default()).await?;
        sleep(duration).await;
        let peripherals = self.adapter.peripherals().await?;
        let mut result = Vec::with_capacity(peripherals.len());
        for peripheral in peripherals {
            let properties = peripheral.properties().await?;
            result.push(DiscoveredDevice {
                id: format!("{:?}", peripheral.id()),
                name: properties.as_ref().and_then(|item| item.local_name.clone()),
                address: peripheral.address().to_string(),
                rssi: properties.as_ref().and_then(|item| item.rssi),
                services: properties.map(|item| item.services).unwrap_or_default(),
            });
        }
        self.adapter.stop_scan().await?;
        Ok(result)
    }

    pub async fn connect_flow8(&self) -> Result<Flow8BleSession, BleError> {
        self.adapter.start_scan(ScanFilter::default()).await?;
        sleep(Duration::from_secs(3)).await;
        let peripherals = self.adapter.peripherals().await?;
        self.adapter.stop_scan().await?;
        for peripheral in peripherals {
            let properties = peripheral.properties().await?;
            let named_flow8 = properties
                .as_ref()
                .and_then(|item| item.local_name.as_deref())
                .is_some_and(|name| name.to_ascii_uppercase().contains("FLOW 8"));
            let advertises_service = properties
                .as_ref()
                .is_some_and(|item| item.services.contains(&SERVICE_UUID));
            if named_flow8 || advertises_service {
                return Flow8BleSession::connect(peripheral).await;
            }
        }
        Err(BleError::DeviceNotFound)
    }
}

pub struct Flow8BleSession {
    peripheral: Peripheral,
    characteristic: Characteristic,
}

impl Flow8BleSession {
    async fn connect(peripheral: Peripheral) -> Result<Self, BleError> {
        if !peripheral.is_connected().await? {
            peripheral.connect().await?;
        }
        peripheral.discover_services().await?;
        let characteristic = peripheral
            .characteristics()
            .into_iter()
            .find(|item| item.service_uuid == SERVICE_UUID && item.uuid == CHARACTERISTIC_UUID)
            .ok_or(BleError::CharacteristicNotFound)?;
        Ok(Self {
            peripheral,
            characteristic,
        })
    }

    pub fn mtu(&self) -> u16 {
        self.peripheral.mtu()
    }
    pub fn characteristic(&self) -> &Characteristic {
        &self.characteristic
    }
    pub async fn device_info(&self) -> Result<DiscoveredDevice, BleError> {
        let properties = self.peripheral.properties().await?;
        Ok(DiscoveredDevice {
            id: format!("{:?}", self.peripheral.id()),
            name: properties.as_ref().and_then(|item| item.local_name.clone()),
            address: self.peripheral.address().to_string(),
            rssi: properties.as_ref().and_then(|item| item.rssi),
            services: properties.map(|item| item.services).unwrap_or_default(),
        })
    }
    /// Returns the complete hierarchy cached by btleplug's public API after
    /// service discovery. `Service` contains its characteristics, and each
    /// `Characteristic` contains its descriptors.
    pub fn discovered_services(&self) -> BTreeSet<Service> {
        self.peripheral.services()
    }
    /// Performs a descriptor read through btleplug. This is exposed for the
    /// read-only GATT diagnostic; no descriptor-write wrapper is provided.
    pub async fn read_descriptor(&self, descriptor: &Descriptor) -> Result<Vec<u8>, BleError> {
        self.peripheral
            .read_descriptor(descriptor)
            .await
            .map_err(Into::into)
    }
    pub async fn subscribe(&self) -> Result<(), BleError> {
        self.peripheral
            .subscribe(&self.characteristic)
            .await
            .map_err(Into::into)
    }
    pub async fn write(&self, frame: &[u8], write_type: WriteType) -> Result<(), BleError> {
        self.peripheral
            .write(&self.characteristic, frame, write_type)
            .await
            .map_err(Into::into)
    }
    pub async fn notifications(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = ValueNotification> + Send>>, BleError> {
        self.peripheral.notifications().await.map_err(Into::into)
    }
    pub async fn disconnect(&self) -> Result<(), BleError> {
        self.peripheral.disconnect().await.map_err(Into::into)
    }
}

#[cfg(not(target_os = "windows"))]
struct RuntimeSession {
    session: Flow8BleSession,
    ingress: RxIngress,
    reader: tokio::task::JoinHandle<()>,
    write_type: WriteType,
}

#[cfg(not(target_os = "windows"))]
impl RuntimeSession {
    async fn connect(
        generation: u64,
        _events: mpsc::UnboundedSender<DeviceEvent>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportRx>), BleError> {
        let transport = BleTransport::new().await?;
        let session = transport.connect_flow8().await?;
        let properties = session.characteristic().properties;
        let write_type = if properties.contains(CharPropFlags::WRITE) {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        };
        session.subscribe().await?;
        let mut notifications = session.notifications().await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let ingress = RxIngress::new(generation, tx);
        let callback_ingress = ingress.clone();
        let reader = tokio::spawn(async move {
            while let Some(notification) = notifications.next().await {
                callback_ingress.forward(&notification.value);
            }
            callback_ingress.disconnected();
        });
        Ok((
            Self {
                session,
                ingress,
                reader,
                write_type,
            },
            rx,
        ))
    }

    fn backend_name(&self) -> &'static str {
        "btleplug"
    }

    fn mtu(&self) -> u16 {
        self.session.mtu()
    }

    fn write_type(&self) -> WriteType {
        self.write_type
    }

    async fn write(&self, frame: &[u8]) -> Result<(), BleError> {
        self.session.write(frame, self.write_type).await
    }

    async fn disconnect(&mut self) -> Result<(), BleError> {
        self.ingress.invalidate();
        self.reader.abort();
        self.session.disconnect().await
    }
}

#[cfg(target_os = "windows")]
struct RuntimeSession {
    session: windows_native::WindowsNativeSession,
}

#[cfg(target_os = "windows")]
impl RuntimeSession {
    async fn connect(
        generation: u64,
        events: mpsc::UnboundedSender<DeviceEvent>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportRx>), BleError> {
        let (session, rx) =
            windows_native::WindowsNativeSession::connect(generation, events).await?;
        Ok((Self { session }, rx))
    }

    fn backend_name(&self) -> &'static str {
        "windows-native-gatt"
    }

    fn mtu(&self) -> u16 {
        self.session.mtu()
    }

    fn write_type(&self) -> WriteType {
        WriteType::WithResponse
    }

    async fn write(&self, frame: &[u8]) -> Result<(), BleError> {
        self.session.write(frame).await
    }

    async fn disconnect(&mut self) -> Result<(), BleError> {
        self.session.disconnect();
        Ok(())
    }
}

async fn execute_actions(
    actions: Vec<SessionAction>,
    session: Option<&RuntimeSession>,
    events: &mpsc::UnboundedSender<DeviceEvent>,
) {
    for action in actions {
        match action {
            SessionAction::Phase(phase) => {
                let _ = events.send(DeviceEvent::Phase(phase));
            }
            SessionAction::Received(command) => {
                let _ = events.send(DeviceEvent::Received(command));
            }
            SessionAction::Error(error) => {
                let _ = events.send(DeviceEvent::Error(error));
            }
            SessionAction::Send(frame) => {
                let Some(session) = session else {
                    let _ = events.send(DeviceEvent::Error(
                        "protocol requested a write without a GATT session".into(),
                    ));
                    continue;
                };
                debug!(
                    backend = session.backend_name(),
                    command = frame.first().copied().unwrap_or_default(),
                    bytes = frame.len(),
                    "FLOW TX"
                );
                trace!(raw = %hex_bytes(&frame), "FLOW TX raw packet");
                match frame.first().copied() {
                    Some(0x39) => info!("sending FLOW 8 handshake response 0x39"),
                    Some(0x37) => info!("requesting FLOW 8 full mixer state with 0x37"),
                    _ => {}
                }
                match session.write(&frame).await {
                    Ok(()) => {
                        let _ = events.send(DeviceEvent::RawTx(frame));
                    }
                    Err(error) => {
                        let _ = events.send(DeviceEvent::Error(error.to_string()));
                    }
                }
            }
        }
    }
}

async fn handle_device_command(
    command: DeviceCommand,
    session: &mut Option<RuntimeSession>,
    transport_rx: &mut Option<mpsc::UnboundedReceiver<TransportRx>>,
    coordinator: &mut Flow8Session,
    generation: &mut u64,
    events: &mpsc::UnboundedSender<DeviceEvent>,
) -> bool {
    match command {
        DeviceCommand::Shutdown => {
            if let Some(mut active) = session.take()
                && let Err(error) = active.disconnect().await
            {
                let _ = events.send(DeviceEvent::Error(error.to_string()));
            }
            *transport_rx = None;
            coordinator.transition(SessionPhase::Disconnected);
            return false;
        }
        DeviceCommand::Scan { duration } => {
            let _ = events.send(DeviceEvent::Phase(SessionPhase::Scanning));
            match BleTransport::new().await {
                Ok(transport) => match transport.scan(duration).await {
                    Ok(devices) => {
                        let _ = events.send(DeviceEvent::ScanResults(devices));
                        let _ = events.send(DeviceEvent::Phase(coordinator.phase()));
                    }
                    Err(error) => {
                        let _ = events.send(DeviceEvent::Error(error.to_string()));
                    }
                },
                Err(error) => {
                    let _ = events.send(DeviceEvent::Error(error.to_string()));
                }
            }
        }
        DeviceCommand::Connect => {
            if let Some(mut active) = session.take()
                && let Err(error) = active.disconnect().await
            {
                let _ = events.send(DeviceEvent::Error(error.to_string()));
            }
            *transport_rx = None;
            coordinator.transition(SessionPhase::Disconnected);
            *generation = generation.wrapping_add(1);
            let _ = events.send(DeviceEvent::Phase(SessionPhase::Connecting));
            coordinator.transition(SessionPhase::Connecting);
            match RuntimeSession::connect(*generation, events.clone()).await {
                Ok((connected, rx)) => {
                    coordinator.transition(SessionPhase::GattReady);
                    let _ = events.send(DeviceEvent::Phase(SessionPhase::GattReady));
                    let _ = events.send(DeviceEvent::Backend(connected.backend_name()));
                    let _ = events.send(DeviceEvent::Mtu(connected.mtu()));
                    let _ = events.send(DeviceEvent::WriteMode(connected.write_type()));
                    coordinator.transition(SessionPhase::RxArming);
                    let _ = events.send(DeviceEvent::Phase(SessionPhase::RxArming));
                    info!(
                        backend = connected.backend_name(),
                        mtu = connected.mtu(),
                        write_type = ?connected.write_type(),
                        "FLOW 8 RX path armed"
                    );
                    *transport_rx = Some(rx);
                    *session = Some(connected);
                    let action = coordinator.rx_armed();
                    execute_actions(vec![action], session.as_ref(), events).await;
                }
                Err(error) => {
                    tracing::error!(
                        reconnect_reason = %error,
                        "FLOW 8 connection failed; the native failure remains visible until the next successful RX arm"
                    );
                    coordinator.transition(SessionPhase::Error);
                    let _ = events.send(DeviceEvent::Phase(SessionPhase::Error));
                    let _ = events.send(DeviceEvent::Error(error.to_string()));
                }
            }
        }
        DeviceCommand::Disconnect => {
            if let Some(mut active) = session.take()
                && let Err(error) = active.disconnect().await
            {
                let _ = events.send(DeviceEvent::Error(error.to_string()));
            }
            *transport_rx = None;
            let action = coordinator.transition(SessionPhase::Disconnected);
            execute_actions(vec![action], None, events).await;
        }
        DeviceCommand::Send(command) => {
            if coordinator.phase() != SessionPhase::Ready {
                let _ = events.send(DeviceEvent::Error(format!(
                    "FLOW 8 command rejected while session is {:?}; wait for Ready",
                    coordinator.phase()
                )));
                return true;
            }
            match coordinator.encode_command(&command) {
                Ok(actions) => {
                    execute_actions(actions, session.as_ref(), events).await;
                }
                Err(error) => {
                    let _ = events.send(DeviceEvent::Error(error.to_string()));
                }
            }
        }
        DeviceCommand::StateApplied => match coordinator.state_applied() {
            Ok(action) => {
                info!("FLOW 8 Store apply acknowledged; session reached Ready");
                execute_actions(vec![action], session.as_ref(), events).await;
            }
            Err(error) => {
                let _ = events.send(DeviceEvent::Error(error.to_string()));
            }
        },
    }
    true
}

async fn handle_transport_rx(
    event: TransportRx,
    active_generation: u64,
    session: &mut Option<RuntimeSession>,
    transport_rx: &mut Option<mpsc::UnboundedReceiver<TransportRx>>,
    coordinator: &mut Flow8Session,
    events: &mpsc::UnboundedSender<DeviceEvent>,
) {
    let generation = match &event {
        TransportRx::Packet { generation, .. } | TransportRx::Disconnected { generation } => {
            *generation
        }
        #[cfg(target_os = "windows")]
        TransportRx::Connected { generation } | TransportRx::Error { generation, .. } => {
            *generation
        }
    };
    if generation != active_generation {
        debug!(
            generation,
            active_generation, "ignored stale FLOW 8 transport callback"
        );
        return;
    }

    match event {
        TransportRx::Packet { bytes, .. } => {
            trace!(raw = %hex_bytes(&bytes), "FLOW RX raw packet");
            if let Ok(packet) = flow8_protocol::parse_packet(&bytes) {
                debug!(
                    command = packet.command,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    bytes = bytes.len(),
                    "FLOW RX"
                );
            }
            let _ = events.send(DeviceEvent::RawRx(bytes.clone()));
            let actions = coordinator.notification(&bytes);
            for action in &actions {
                match action {
                    SessionAction::Received(RxCommand::HandshakeHost { .. }) => {
                        info!("FLOW 8 handshake 0x35 received");
                    }
                    SessionAction::Received(RxCommand::HandshakeReply) => {
                        info!("FLOW 8 handshake 0x36 received");
                    }
                    SessionAction::Received(RxCommand::MixerState(_)) => {
                        info!("FLOW 8 0x38 complete and decoded; awaiting atomic Store apply");
                    }
                    _ => {}
                }
            }
            execute_actions(actions, session.as_ref(), events).await;
        }
        #[cfg(target_os = "windows")]
        TransportRx::Connected { .. } => {
            info!("FLOW 8 physical connection established");
        }
        TransportRx::Disconnected { .. } => {
            info!("FLOW 8 disconnected; invalidating session state");
            if let Some(mut active) = session.take()
                && let Err(error) = active.disconnect().await
            {
                let _ = events.send(DeviceEvent::Error(error.to_string()));
            }
            *transport_rx = None;
            let action = coordinator.transition(SessionPhase::Disconnected);
            execute_actions(vec![action], None, events).await;
        }
        #[cfg(target_os = "windows")]
        TransportRx::Error { message, .. } => {
            let _ = events.send(DeviceEvent::Error(message));
        }
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

async fn device_actor(
    mut commands: mpsc::UnboundedReceiver<DeviceCommand>,
    events: mpsc::UnboundedSender<DeviceEvent>,
    client_id: [u8; 16],
) {
    let mut coordinator = Flow8Session::new(client_id);
    let mut session: Option<RuntimeSession> = None;
    let mut transport_rx: Option<mpsc::UnboundedReceiver<TransportRx>> = None;
    let mut generation = 0u64;

    loop {
        if let Some(rx) = transport_rx.as_mut() {
            tokio::select! {
                command = commands.recv() => {
                    let Some(command) = command else { break; };
                    if !handle_device_command(
                        command,
                        &mut session,
                        &mut transport_rx,
                        &mut coordinator,
                        &mut generation,
                        &events,
                    ).await {
                        break;
                    }
                }
                notification = rx.recv() => {
                    match notification {
                        Some(event) => {
                            handle_transport_rx(
                                event,
                                generation,
                                &mut session,
                                &mut transport_rx,
                                &mut coordinator,
                                &events,
                            ).await;
                        }
                        None => {
                            transport_rx = None;
                            let action = coordinator.transition(SessionPhase::Disconnected);
                            execute_actions(vec![action], None, &events).await;
                        }
                    }
                }
            }
        } else {
            let Some(command) = commands.recv().await else {
                break;
            };
            if !handle_device_command(
                command,
                &mut session,
                &mut transport_rx,
                &mut coordinator,
                &mut generation,
                &events,
            )
            .await
            {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow8_protocol::{
        ChannelLabel, FxState, InputState, MixerState, OutputState, frame_single,
    };

    fn zero_input(id: u8) -> InputState {
        InputState {
            id,
            flags: 0,
            gain_db: 0.0,
            compressor_amount: 0.0,
            high_pass_hz: 80,
            balance: 0.0,
            eq_gain_db: [0.0; 4],
            eq_frequency_hz: [80, 400, 2_500, 10_000],
            eq_q: [1.0; 4],
            label: ChannelLabel {
                endpoint: id,
                icon: 0,
                text: String::new(),
            },
        }
    }

    fn zero_output(id: u8) -> OutputState {
        OutputState {
            id,
            flags: 0,
            volume_db: 0.0,
            pan: 0.0,
            limiter_db: -3.0,
            input_gains_db: [0.0; 7],
            eq_gain_db: [0.0; 9],
            eq_frequency_hz: [63, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000],
            eq_q: [1.0; 9],
            delay_ticks: 0,
        }
    }

    fn zero_fx(id: u8) -> FxState {
        FxState {
            id,
            flags: 0,
            volume_db: 0.0,
            pan: 0.0,
            values: [0; 3],
            preset: 0,
            input_gains_db: [0.0; 7],
            aux_gains_db: [0.0; 2],
        }
    }

    fn mixer() -> MixerState {
        MixerState {
            inputs: std::array::from_fn(|index| zero_input(index as u8)),
            outputs: [zero_output(10), zero_output(11), zero_output(15)],
            effects: [zero_fx(12), zero_fx(13)],
            headphone_volume_db: 0.0,
            flags: [false; 13],
            tempo_bpm: 120,
            selected_output: 15,
            last_snapshot: 0,
            monitor_routing: 0,
            snapshot_scope: 0,
        }
    }

    #[test]
    fn apk_handshake_progresses_to_state_syncing() {
        let client_id = *b"FLOW8-PC-RUST001";
        let mut session = Flow8Session::new(client_id);
        session.rx_armed();
        let mut hello = vec![0x11; 16];
        hello.extend([0, 0, 1, 0, 2]);
        let actions = session.notification(&frame_single(0x35, &hello));
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame[0] == 0x39))
        );
        let actions = session.notification(&frame_single(0x36, &[]));
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame[0] == 0x37))
        );
    }

    #[test]
    fn complete_fragmented_mixer_state_is_the_ready_gate() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        session.transition(SessionPhase::StateSyncing);
        let frames = encode_frames(&TxCommand::MixerState(Box::new(mixer())), 128, 2).unwrap();
        assert_eq!(frames.len(), 4);
        for frame in &frames[..3] {
            assert!(session.notification(frame).is_empty());
            assert_eq!(session.phase(), SessionPhase::StateSyncing);
        }
        let actions = session.notification(&frames[3]);
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, SessionAction::Received(RxCommand::MixerState(_))))
        );
        assert_eq!(
            session.state_applied().unwrap(),
            SessionAction::Phase(SessionPhase::Ready)
        );
        assert_eq!(session.phase(), SessionPhase::Ready);
    }

    #[test]
    fn broken_notification_never_advances_session() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        session.transition(SessionPhase::StateSyncing);
        let actions = session.notification(&[0x36, 0x01, 0x00]);
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
        assert!(matches!(actions.as_slice(), [SessionAction::Error(_)]));
    }

    #[test]
    fn snapshot_load_event_requests_a_fresh_composite_state() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        session.transition(SessionPhase::Ready);
        let actions = session.notification(&frame_single(0x20, &[3]));
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame[0] == 0x37))
        );
    }

    #[test]
    fn reconnect_discards_incomplete_state_from_the_previous_session() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        session.transition(SessionPhase::StateSyncing);
        let frames = encode_frames(
            &TxCommand::MixerState(Box::new(mixer())),
            flow8_protocol::MAX_RAW_PACKET_SIZE,
            1,
        )
        .unwrap();

        assert!(session.notification(&frames[0]).is_empty());
        session.transition(SessionPhase::Disconnected);
        session.rx_armed();
        session.transition(SessionPhase::StateSyncing);

        assert!(session.notification(&frames[1]).is_empty());
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
        let actions = session.notification(&frames[0]);
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, SessionAction::Received(RxCommand::MixerState(_))))
        );
        session.state_applied().unwrap();
        assert_eq!(session.phase(), SessionPhase::Ready);
    }

    #[test]
    fn ready_cannot_be_acknowledged_before_a_complete_state() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        session.transition(SessionPhase::StateSyncing);
        assert!(matches!(
            session.state_applied(),
            Err(BleError::UnexpectedSessionState(_))
        ));
        assert_eq!(session.phase(), SessionPhase::StateSyncing);
    }

    #[test]
    fn transport_callback_copies_bytes_before_forwarding() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let ingress = RxIngress::new(7, tx);
        let mut callback_buffer = vec![0x35, 0x01, 0x36];

        assert!(ingress.forward(&callback_buffer));
        callback_buffer.fill(0xff);

        assert!(matches!(
            rx.try_recv(),
            Ok(TransportRx::Packet { generation: 7, bytes })
                if bytes == vec![0x35, 0x01, 0x36]
        ));
    }

    #[test]
    fn invalidated_transport_ignores_late_callbacks() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let ingress = RxIngress::new(11, tx);
        ingress.invalidate();

        assert!(!ingress.forward(&[0x35]));
        assert!(matches!(
            rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn reconnect_uses_a_fresh_callback_generation() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let old = RxIngress::new(20, tx.clone());
        old.invalidate();
        let current = RxIngress::new(21, tx);

        assert!(!old.forward(&[0x35]));
        assert!(current.forward(&[0x35]));
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportRx::Packet { generation: 21, .. })
        ));
    }

    #[test]
    fn windows_flow8_policy_never_uses_standard_subscription() {
        assert!(!WINDOWS_FLOW8_USES_STANDARD_SUBSCRIBE);
    }

    #[test]
    fn native_connection_errors_retain_stage_api_hresult_and_retry_policy() {
        let error = NativeConnectionError {
            stage: NativeConnectionStage::NativeServiceHandleOpened,
            api: "CreateFileW(FLOW GATT service interface)",
            logical_code: None,
            code: Some(0x8007_0005),
            retryable: true,
            message: "access denied".into(),
        };
        let display = error.to_string();
        assert!(display.contains("stage=NativeServiceHandleOpened"));
        assert!(display.contains("api=CreateFileW(FLOW GATT service interface)"));
        assert!(display.contains("logical_code=none"));
        assert!(display.contains("code=0x80070005"));
        assert!(display.contains("retryable=true"));
        assert!(display.contains("message=access denied"));
    }

    #[test]
    fn logical_native_failures_do_not_fabricate_an_hresult() {
        let error = NativeConnectionError {
            stage: NativeConnectionStage::DeviceFound,
            api: "FlowDeviceIdentity",
            logical_code: Some("identity_insufficient"),
            code: None,
            retryable: true,
            message: "no reliable identity fields are available".into(),
        };
        let display = error.to_string();
        assert!(display.contains("logical_code=identity_insufficient"));
        assert!(display.contains("code=unavailable"));
        assert!(!display.contains("0x00000000"));
    }

    #[test]
    fn a_native_connect_failure_does_not_poison_the_next_session_generation() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let failed = RxIngress::new(30, tx.clone());
        failed.invalidate();
        let retry = RxIngress::new(31, tx);

        assert!(!failed.forward(&[0x35]));
        assert!(retry.forward(&[0x35]));
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportRx::Packet { generation: 31, bytes }) if bytes == vec![0x35]
        ));
    }
}
