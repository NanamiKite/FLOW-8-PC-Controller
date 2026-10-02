//! btleplug transport boundary. The GUI does not depend on an adapter being
//! present; callers run these futures on a separate Tokio runtime.

use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::sleep;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

mod client_identity;
#[cfg(target_os = "windows")]
mod directhci_backend;
#[cfg(target_os = "windows")]
mod windows_native;

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x14839ad4_8d7e_415c_9a42_167340cf2339);
pub const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x0034594a_a8e7_4b1a_a6b1_cd5243059a57);

const MAX_QUEUED_RX_PACKETS: usize = 256;
const MAX_PENDING_WRITES: usize = 256;

#[derive(Debug)]
struct RxQueuePermit(Arc<AtomicUsize>);

impl Drop for RxQueuePermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Owned messages crossing the platform callback/stream boundary. Native
/// callbacks never parse FLOW protocol data; they only copy bytes into this
/// channel and return.
#[derive(Debug)]
enum TransportRx {
    Packet {
        generation: u64,
        bytes: Vec<u8>,
        _permit: RxQueuePermit,
    },
    #[cfg(target_os = "windows")]
    Connected {
        generation: u64,
    },
    Disconnected {
        generation: u64,
    },
    Overflow {
        generation: u64,
    },
    #[cfg(target_os = "windows")]
    Error {
        generation: u64,
        message: String,
    },
    #[cfg(target_os = "windows")]
    HandshakeTimeout {
        generation: u64,
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
    pending: Arc<AtomicUsize>,
}

impl RxIngress {
    fn new(generation: u64, tx: mpsc::UnboundedSender<TransportRx>) -> Self {
        Self {
            generation,
            active: Arc::new(AtomicBool::new(true)),
            tx,
            pending: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn forward(&self, source: &[u8]) -> bool {
        if !self.active.load(Ordering::Acquire) {
            if source.first() == Some(&0x38) {
                warn!(
                    generation = self.generation,
                    "FLOW 0x38 ingress rejected: inactive session"
                );
            }
            return false;
        }
        if self
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_QUEUED_RX_PACKETS).then_some(count + 1)
            })
            .is_err()
        {
            if self.active.swap(false, Ordering::AcqRel) {
                let _ = self.tx.send(TransportRx::Overflow {
                    generation: self.generation,
                });
            }
            return false;
        }
        let forwarded = self
            .tx
            .send(TransportRx::Packet {
                generation: self.generation,
                bytes: source.to_vec(),
                _permit: RxQueuePermit(Arc::clone(&self.pending)),
            })
            .is_ok();
        if source.first() == Some(&0x38) {
            match flow8_protocol::parse_packet(source) {
                Ok(packet) => info!(
                    generation = self.generation,
                    sequence = ?packet.sequence,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    bytes = source.len(),
                    forwarded,
                    "FLOW 0x38 transport RX channel"
                ),
                Err(error) => warn!(generation = self.generation, bytes = source.len(), %error,
                    forwarded, "FLOW 0x38 transport RX channel: invalid frame"),
            }
        }
        forwarded
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
        if !self.active.swap(false, Ordering::AcqRel) {
            return false;
        }
        self.tx
            .send(TransportRx::Error {
                generation: self.generation,
                message: message.into(),
            })
            .is_ok()
    }

    #[cfg(target_os = "windows")]
    fn handshake_timeout(&self) -> bool {
        self.send(TransportRx::HandshakeTimeout {
            generation: self.generation,
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
    FlowServiceSelected,
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
            Self::FlowServiceSelected => "FlowServiceSelected",
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
    Warning(String),
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
    ProtocolWarning(String),
    Error(String),
}

const MAX_QUEUED_GUI_EVENTS: usize = 512;
const MAX_QUEUED_USER_COMMANDS: usize = 256;

#[derive(Clone)]
struct EventSender {
    tx: mpsc::Sender<DeviceEvent>,
    overflowed: Arc<AtomicBool>,
    overflow_notify: Arc<Notify>,
}

impl EventSender {
    fn send(&self, event: DeviceEvent) -> Result<(), ()> {
        if self.overflowed.load(Ordering::Acquire) {
            return Err(());
        }
        match self.tx.try_send(event) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                if !self.overflowed.swap(true, Ordering::AcqRel) {
                    warn!(
                        limit = MAX_QUEUED_GUI_EVENTS,
                        "FLOW GUI event queue exceeded capacity; stopping BLE session"
                    );
                    self.overflow_notify.notify_one();
                }
                Err(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(()),
        }
    }

    fn overflowed(&self) -> bool {
        self.overflowed.load(Ordering::Acquire)
    }
}

pub struct DeviceRuntime {
    command_tx: mpsc::UnboundedSender<DeviceCommand>,
    user_command_tx: mpsc::Sender<TxCommand>,
    event_rx: mpsc::Receiver<DeviceEvent>,
    event_overflowed: Arc<AtomicBool>,
    event_overflow_reported: bool,
}

impl DeviceRuntime {
    /// Starts the Tokio/BLE worker on its own thread. Constructing this handle
    /// does not touch a Bluetooth adapter, so simulator-only GUI startup works
    /// on hosts without Bluetooth.
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (user_command_tx, user_command_rx) = mpsc::channel(MAX_QUEUED_USER_COMMANDS);
        let (event_tx, event_rx) = mpsc::channel(MAX_QUEUED_GUI_EVENTS);
        let event_overflowed = Arc::new(AtomicBool::new(false));
        let events = EventSender {
            tx: event_tx,
            overflowed: Arc::clone(&event_overflowed),
            overflow_notify: Arc::new(Notify::new()),
        };
        std::thread::Builder::new()
            .name("flow8-ble-runtime".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(2)
                    .build();
                match runtime {
                    Ok(runtime) => {
                        runtime.block_on(device_actor(command_rx, user_command_rx, events))
                    }
                    Err(error) => {
                        let _ = events.send(DeviceEvent::Error(error.to_string()));
                    }
                }
            })
            .expect("spawn FLOW 8 BLE runtime thread");
        Self {
            command_tx,
            user_command_tx,
            event_rx,
            event_overflowed,
            event_overflow_reported: false,
        }
    }

    pub fn send(&self, command: DeviceCommand) -> Result<(), String> {
        if let DeviceCommand::Send(command) = command {
            return self
                .user_command_tx
                .try_send(command)
                .map_err(|error| match error {
                    mpsc::error::TrySendError::Full(_) => {
                        "BLE user command queue is full; the command was not sent".to_owned()
                    }
                    mpsc::error::TrySendError::Closed(_) => "BLE runtime stopped".to_owned(),
                });
        }
        self.command_tx
            .send(command)
            .map_err(|_| "BLE runtime stopped".to_owned())
    }

    pub fn try_recv(&mut self) -> Option<DeviceEvent> {
        if self.event_overflowed.load(Ordering::Acquire) {
            if self.event_overflow_reported {
                return None;
            }
            self.event_overflow_reported = true;
            return Some(DeviceEvent::Error(
                "FLOW 8 event backlog exceeded capacity; connection stopped. Restart the application before reconnecting.".into(),
            ));
        }
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
    received_host_hello: bool,
    handshake_response_pending_or_sent: bool,
    state_request_pending_or_sent: bool,
}

impl Flow8Session {
    pub fn new(client_id: [u8; 16]) -> Self {
        Self {
            phase: SessionPhase::Disconnected,
            client_id,
            sequence: 0,
            decoder: CommandStreamDecoder::default(),
            awaiting_state_apply: false,
            received_host_hello: false,
            handshake_response_pending_or_sent: false,
            state_request_pending_or_sent: false,
        }
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    fn set_client_id(&mut self, client_id: [u8; 16]) {
        self.client_id = client_id;
    }

    pub fn transition(&mut self, phase: SessionPhase) -> SessionAction {
        self.phase = phase;
        if phase == SessionPhase::Disconnected {
            self.decoder.clear();
            self.sequence = 0;
            self.awaiting_state_apply = false;
            self.received_host_hello = false;
            self.handshake_response_pending_or_sent = false;
            self.state_request_pending_or_sent = false;
        } else if phase == SessionPhase::StateSyncing {
            self.awaiting_state_apply = false;
        }
        SessionAction::Phase(phase)
    }

    /// Marks the platform receive path as armed. On standard BLE backends this
    /// follows subscription; on FLOW 8/Windows it follows native event
    /// registration and never implies a CCCD write.
    pub fn rx_armed(&mut self) -> SessionAction {
        self.received_host_hello = false;
        self.handshake_response_pending_or_sent = false;
        self.state_request_pending_or_sent = false;
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
        self.state_request_pending_or_sent = false;
        Ok(self.transition(SessionPhase::Ready))
    }

    pub fn encode_command(&mut self, command: &TxCommand) -> Result<Vec<SessionAction>, BleError> {
        let frames = encode_frames(command, flow8_protocol::MAX_RAW_PACKET_SIZE, self.sequence)
            .map_err(BleError::Protocol)?;
        self.sequence = self.sequence.wrapping_add(1) & 0x03;
        Ok(frames.into_iter().map(SessionAction::Send).collect())
    }

    pub fn notification(&mut self, raw: &[u8]) -> Vec<SessionAction> {
        let state_fragment = if raw.first() == Some(&0x38) {
            flow8_protocol::parse_packet(raw).ok()
        } else {
            None
        };
        let decoded = self.decoder.accept(raw);
        if let Some(packet) = state_fragment {
            match &decoded {
                Ok(None) => info!(
                    sequence = ?packet.sequence,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    pending_slots = self.decoder.pending_count(),
                    "FLOW 0x38 reassembler accepted fragment; state incomplete"
                ),
                Ok(Some(_)) => info!(
                    sequence = ?packet.sequence,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    "FLOW 0x38 decoder completed state"
                ),
                Err(flow8_protocol::ProtocolError::InvalidPayload(0x38)) => warn!(
                    sequence = ?packet.sequence,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    "FLOW 0x38 reassembly completed but MixerState payload parser rejected it"
                ),
                Err(error) => warn!(
                    sequence = ?packet.sequence,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    pending_slots = self.decoder.pending_count(),
                    %error,
                    "FLOW 0x38 frame or reassembler rejected fragment"
                ),
            }
        }
        match decoded {
            Ok(None) => Vec::new(),
            Err(flow8_protocol::ProtocolError::UnsupportedCommand(command)) => {
                warn!(command, phase = ?self.phase, "ignoring unsupported FLOW 8 RX command without ending the session");
                Vec::new()
            }
            Err(error) => {
                warn!(phase = ?self.phase, %error, "FLOW 8 RX frame rejected; session remains active");
                vec![SessionAction::Warning(error.to_string())]
            }
            Ok(Some(command)) => {
                let mut actions = vec![SessionAction::Received(command.clone())];
                match command {
                    RxCommand::HandshakeHost { .. } => {
                        if self.phase != SessionPhase::Handshaking
                            && !(self.phase == SessionPhase::StateSyncing
                                && self.received_host_hello)
                        {
                            return vec![SessionAction::Warning(format!(
                                "ignored FLOW 8 0x35 while RX was not armed (phase={:?})",
                                self.phase
                            ))];
                        }
                        self.received_host_hello = true;
                        if self.handshake_response_pending_or_sent {
                            debug!("duplicate FLOW 8 0x35; 0x39 already queued or sent");
                            return actions;
                        }
                        if self.phase != SessionPhase::StateSyncing {
                            actions.push(self.transition(SessionPhase::StateSyncing));
                        }
                        match self.encode_command(&TxCommand::HandshakeClient {
                            client_id: self.client_id,
                        }) {
                            Ok(mut sends) => {
                                self.handshake_response_pending_or_sent = true;
                                actions.append(&mut sends);
                            }
                            Err(error) => actions.push(SessionAction::Error(error.to_string())),
                        }
                    }
                    RxCommand::HandshakeReply => {
                        if self.phase != SessionPhase::StateSyncing || !self.received_host_hello {
                            return vec![SessionAction::Warning(format!(
                                "ignored FLOW 8 0x36 before a valid 0x35 (phase={:?})",
                                self.phase
                            ))];
                        }
                        if self.state_request_pending_or_sent {
                            debug!("duplicate FLOW 8 0x36; 0x37 already queued or sent");
                            return actions;
                        }
                        info!(source = "rx_0x36", phase = ?self.phase, "FLOW 0x37 action source");
                        match self.encode_command(&TxCommand::GetMixerState) {
                            Ok(mut sends) => {
                                self.state_request_pending_or_sent = true;
                                actions.append(&mut sends);
                            }
                            Err(error) => actions.push(SessionAction::Error(error.to_string())),
                        }
                    }
                    RxCommand::MixerState(_) => {
                        self.awaiting_state_apply = true;
                    }
                    event @ (RxCommand::SnapshotLoad { .. } | RxCommand::FactoryReset) => {
                        let source = if matches!(event, RxCommand::FactoryReset) {
                            "rx_factory_reset"
                        } else {
                            "rx_snapshot_load"
                        };
                        info!(source, phase = ?self.phase, "FLOW 0x37 action source");
                        actions.push(self.transition(SessionPhase::StateSyncing));
                        self.state_request_pending_or_sent = false;
                        match self.encode_command(&TxCommand::GetMixerState) {
                            Ok(mut sends) => {
                                self.state_request_pending_or_sent = true;
                                actions.append(&mut sends);
                            }
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
    #[error("FLOW 8 transport failed: {0}")]
    Transport(String),
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
/// A failed production connection remains in Error until the user explicitly
/// requests another connection attempt. This keeps the first native failure
/// and its logs intact instead of hiding it behind a scan/reconnect loop.
pub const AUTO_RECONNECT_AFTER_FAILURE: bool = false;

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
        _events: EventSender,
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

    fn mark_handshake_rx(&self) {}

    async fn disconnect(&mut self, _source: &'static str) -> Result<(), BleError> {
        self.ingress.invalidate();
        self.reader.abort();
        self.session.disconnect().await
    }
}

#[cfg(target_os = "windows")]
enum RuntimeSession {
    WindowsNative(windows_native::WindowsNativeSession),
    DirectHci(directhci_backend::DirectHciSession),
}

#[cfg(target_os = "windows")]
impl RuntimeSession {
    async fn connect(
        generation: u64,
        events: EventSender,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportRx>), BleError> {
        if directhci_requested() {
            let (session, rx) =
                directhci_backend::DirectHciSession::connect(generation, events).await?;
            Ok((Self::DirectHci(session), rx))
        } else {
            let (session, rx) =
                windows_native::WindowsNativeSession::connect(generation, events).await?;
            Ok((Self::WindowsNative(session), rx))
        }
    }

    fn backend_name(&self) -> &'static str {
        match self {
            Self::WindowsNative(_) => "windows-native-gatt",
            Self::DirectHci(_) => "directhci",
        }
    }

    fn mtu(&self) -> u16 {
        match self {
            Self::WindowsNative(session) => session.mtu(),
            Self::DirectHci(session) => session.mtu(),
        }
    }

    fn write_type(&self) -> WriteType {
        WriteType::WithResponse
    }

    async fn write(&mut self, frame: &[u8]) -> Result<(), BleError> {
        match self {
            Self::WindowsNative(session) => session.write(frame).await,
            Self::DirectHci(session) => session.write(frame).await,
        }
    }

    fn mark_handshake_rx(&self) {
        match self {
            Self::WindowsNative(session) => session.mark_handshake_rx(),
            Self::DirectHci(session) => session.mark_handshake_rx(),
        }
    }

    async fn disconnect(&mut self, source: &'static str) -> Result<(), BleError> {
        match self {
            Self::WindowsNative(session) => {
                session.disconnect();
                Ok(())
            }
            Self::DirectHci(session) => session.disconnect(source).await,
        }
    }
}

#[cfg(target_os = "windows")]
fn directhci_requested() -> bool {
    !std::env::var("FLOW8_BLE_BACKEND").is_ok_and(|value| {
        value.eq_ignore_ascii_case("windows-native")
            || value.eq_ignore_ascii_case("native")
            || value.eq_ignore_ascii_case("legacy")
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContinuousKey {
    Pan(u8),
    EndpointPan(u8),
    Gain(u8),
    Route(u8, u8),
    Master(u8),
    GraphicEq(u8, u8),
    ParametricEq(u8, u8),
    Compressor(u8),
    Limiter(u8),
    FxSetup(u8, u8),
    Tempo,
    Delay(u8),
}

impl ContinuousKey {
    fn for_command(command: &TxCommand) -> Option<Self> {
        match command {
            TxCommand::Pan { input, .. } => Some(Self::Pan(input.endpoint())),
            TxCommand::EndpointPan { endpoint, .. } => Some(Self::EndpointPan(*endpoint)),
            TxCommand::Gain { input, .. } => Some(Self::Gain(input.endpoint())),
            TxCommand::RouteLevel {
                source,
                destination,
                ..
            } => Some(Self::Route(source.endpoint(), destination.endpoint())),
            TxCommand::DestinationMaster { destination, .. } => {
                Some(Self::Master(destination.endpoint()))
            }
            TxCommand::GraphicEq { endpoint, band, .. } => Some(Self::GraphicEq(*endpoint, *band)),
            TxCommand::ParametricEq { input, band, .. } => {
                Some(Self::ParametricEq(input.endpoint(), *band))
            }
            TxCommand::Compressor { input, .. } => Some(Self::Compressor(input.endpoint())),
            TxCommand::Limiter { endpoint, .. } => Some(Self::Limiter(*endpoint)),
            TxCommand::FxSetup {
                endpoint,
                route_flags,
                ..
            } => Some(Self::FxSetup(*endpoint, *route_flags)),
            TxCommand::FxTempo { .. } => Some(Self::Tempo),
            TxCommand::ChannelDelay { endpoint, .. } => Some(Self::Delay(*endpoint)),
            // Other discrete commands remain FIFO.
            _ => None,
        }
    }
}

enum WriterCommand {
    Write {
        frame: Vec<u8>,
        key: Option<ContinuousKey>,
    },
    MarkHandshakeRx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueDisposition {
    Queued,
    Replaced,
}

struct WriterMailboxState {
    pending: VecDeque<WriterCommand>,
    open: bool,
}

struct WriterMailbox {
    state: Mutex<WriterMailboxState>,
    ready: Notify,
}

impl WriterMailbox {
    fn new() -> Self {
        Self {
            state: Mutex::new(WriterMailboxState {
                pending: VecDeque::new(),
                open: true,
            }),
            ready: Notify::new(),
        }
    }

    fn push(&self, command: WriterCommand) -> Result<QueueDisposition, BleError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BleError::Transport("FLOW 8 TX mailbox poisoned".into()))?;
        if !state.open {
            return Err(BleError::Transport("FLOW 8 TX writer stopped".into()));
        }
        match command {
            WriterCommand::Write {
                frame,
                key: Some(key),
            } => {
                // Never coalesce across a discrete operation or handshake marker:
                // their ordering relative to control changes must be preserved.
                for pending in state.pending.iter_mut().rev() {
                    match pending {
                        WriterCommand::Write {
                            frame: queued,
                            key: Some(existing),
                        } if *existing == key => {
                            *queued = frame;
                            return Ok(QueueDisposition::Replaced);
                        }
                        WriterCommand::Write {
                            key: Some(ContinuousKey::FxSetup(endpoint, route_flags)),
                            ..
                        } if matches!(key, ContinuousKey::FxSetup(target, flags)
                            if *endpoint == target && *route_flags != flags) =>
                        {
                            // Preserve every FX return-routing transition. Parameter
                            // drags may replace only writes with the same route flags.
                            break;
                        }
                        WriterCommand::Write { key: None, .. } | WriterCommand::MarkHandshakeRx => {
                            break;
                        }
                        _ => {}
                    }
                }
                if state.pending.len() >= MAX_PENDING_WRITES {
                    return Err(BleError::Transport(
                        "FLOW 8 TX queue capacity exceeded".into(),
                    ));
                }
                state.pending.push_back(WriterCommand::Write {
                    frame,
                    key: Some(key),
                });
            }
            other => {
                if state.pending.len() >= MAX_PENDING_WRITES {
                    return Err(BleError::Transport(
                        "FLOW 8 TX queue capacity exceeded".into(),
                    ));
                }
                state.pending.push_back(other);
            }
        }
        drop(state);
        self.ready.notify_one();
        Ok(QueueDisposition::Queued)
    }

    async fn recv(&self) -> Option<WriterCommand> {
        loop {
            let notified = self.ready.notified();
            {
                let mut state = self.state.lock().ok()?;
                if let Some(command) = state.pending.pop_front() {
                    return Some(command);
                }
                if !state.open {
                    return None;
                }
            }
            notified.await;
        }
    }

    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.open = false;
            state.pending.clear();
        }
        self.ready.notify_one();
    }
}

struct ActiveSession {
    backend: &'static str,
    mtu: u16,
    write_type: WriteType,
    mailbox: Arc<WriterMailbox>,
    shutdown_tx: Option<oneshot::Sender<&'static str>>,
    writer: tokio::task::JoinHandle<()>,
}

impl ActiveSession {
    async fn new(session: RuntimeSession, events: EventSender) -> Result<Self, BleError> {
        let backend = session.backend_name();
        let mtu = session.mtu();
        let write_type = session.write_type();
        let mailbox = Arc::new(WriterMailbox::new());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (started_tx, started_rx) = oneshot::channel();
        // The RX actor never awaits a GATT write. Only the one session writer
        // may take a frame out of the pending mailbox and write it.
        let writer = tokio::spawn(run_writer(
            session,
            Arc::clone(&mailbox),
            shutdown_rx,
            started_tx,
            events,
        ));
        started_rx
            .await
            .map_err(|_| BleError::Transport("FLOW 8 TX writer exited before startup".into()))?;
        Ok(Self {
            backend,
            mtu,
            write_type,
            mailbox,
            shutdown_tx: Some(shutdown_tx),
            writer,
        })
    }

    fn queue(
        &self,
        frame: Vec<u8>,
        key: Option<ContinuousKey>,
    ) -> Result<QueueDisposition, BleError> {
        self.mailbox.push(WriterCommand::Write { frame, key })
    }

    fn mark_handshake_rx(&self) {
        let _ = self.mailbox.push(WriterCommand::MarkHandshakeRx);
    }

    async fn disconnect(mut self, source: &'static str) {
        if let Some(shutdown) = self.shutdown_tx.take() {
            let _ = shutdown.send(source);
        }
        let _ = self.writer.await;
    }
}

async fn run_writer(
    mut session: RuntimeSession,
    mailbox: Arc<WriterMailbox>,
    mut shutdown: oneshot::Receiver<&'static str>,
    started: oneshot::Sender<()>,
    events: EventSender,
) {
    info!(backend = session.backend_name(), "FLOW TX writer started");
    let _ = started.send(());
    let reason = loop {
        let request = tokio::select! {
            biased;
            reason = &mut shutdown => break reason.unwrap_or("runtime_session_drop"),
            request = mailbox.recv() => request,
        };
        match request {
            Some(WriterCommand::MarkHandshakeRx) => session.mark_handshake_rx(),
            Some(WriterCommand::Write { frame, .. }) => {
                let command = frame.first().copied().unwrap_or_default();
                info!(
                    backend = session.backend_name(),
                    command, "FLOW TX writer received"
                );
                info!(
                    backend = session.backend_name(),
                    command,
                    bytes = frame.len(),
                    "FLOW TX begin"
                );
                let result = tokio::select! {
                    biased;
                    reason = &mut shutdown => break reason.unwrap_or("runtime_session_drop"),
                    result = session.write(&frame) => result,
                };
                match result {
                    Ok(()) => {
                        info!(
                            backend = session.backend_name(),
                            command, "FLOW TX complete"
                        );
                        let _ = events.send(DeviceEvent::RawTx(frame));
                    }
                    Err(error) => {
                        let _ = events.send(DeviceEvent::Error(error.to_string()));
                    }
                }
            }
            None => break "tx_mailbox_closed",
        }
    };
    mailbox.close();
    info!(
        backend = session.backend_name(),
        reason, "FLOW TX writer shutdown"
    );
    if matches!(reason, "tx_mailbox_closed" | "runtime_session_drop") {
        let _ = events.send(DeviceEvent::Error(format!(
            "FLOW 8 TX writer stopped unexpectedly: {reason}"
        )));
    }
    if let Err(error) = session.disconnect(reason).await {
        let _ = events.send(DeviceEvent::Error(error.to_string()));
    }
}

fn execute_actions(
    actions: Vec<SessionAction>,
    session: Option<&ActiveSession>,
    events: &EventSender,
) {
    execute_actions_with_key(actions, session, events, None);
}

fn execute_actions_with_key(
    actions: Vec<SessionAction>,
    session: Option<&ActiveSession>,
    events: &EventSender,
    key: Option<ContinuousKey>,
) {
    for action in actions {
        match action {
            SessionAction::Phase(phase) => {
                let _ = events.send(DeviceEvent::Phase(phase));
            }
            SessionAction::Received(command) => {
                let _ = events.send(DeviceEvent::Received(command));
            }
            SessionAction::Warning(message) => {
                let _ = events.send(DeviceEvent::ProtocolWarning(message));
            }
            SessionAction::Error(error) => {
                let _ = events.send(DeviceEvent::Error(error));
            }
            SessionAction::Send(frame) => {
                let command = frame.first().copied().unwrap_or_default();
                info!(command, "FLOW TX action generated");
                let Some(session) = session else {
                    let _ = events.send(DeviceEvent::Error(
                        "protocol requested a write without a GATT session".into(),
                    ));
                    continue;
                };
                let bytes = frame.len();
                trace!(raw = %hex_bytes(&frame), "FLOW TX raw packet");
                match session.queue(frame, key) {
                    Ok(QueueDisposition::Queued) => {
                        info!(backend = session.backend, command, bytes, "FLOW TX queued");
                        match command {
                            0x39 => info!("sending FLOW 8 handshake response 0x39"),
                            0x37 => info!("requesting FLOW 8 full mixer state with 0x37"),
                            _ => {}
                        }
                    }
                    Ok(QueueDisposition::Replaced) => {
                        debug!(
                            backend = session.backend,
                            command,
                            ?key,
                            "FLOW TX replaced unsent continuous value"
                        );
                    }
                    Err(error) => {
                        let _ = events.send(DeviceEvent::Error(error.to_string()));
                    }
                }
            }
        }
    }
}

fn scan_allowed(phase: SessionPhase, session_active: bool) -> bool {
    !session_active && matches!(phase, SessionPhase::Disconnected | SessionPhase::Error)
}

async fn scan_devices(duration: Duration) -> Result<Vec<DiscoveredDevice>, BleError> {
    #[cfg(target_os = "windows")]
    if directhci_requested() {
        return directhci_backend::scan(duration).await;
    }
    BleTransport::new().await?.scan(duration).await
}

async fn handle_device_command(
    command: DeviceCommand,
    session: &mut Option<ActiveSession>,
    transport_rx: &mut Option<mpsc::UnboundedReceiver<TransportRx>>,
    coordinator: &mut Flow8Session,
    generation: &mut u64,
    events: &EventSender,
) -> bool {
    match command {
        DeviceCommand::Shutdown => {
            if let Some(active) = session.take() {
                active.disconnect("runtime_shutdown").await;
            }
            *transport_rx = None;
            coordinator.transition(SessionPhase::Disconnected);
            return false;
        }
        DeviceCommand::Scan { duration } => {
            if !scan_allowed(coordinator.phase(), session.is_some()) {
                info!(phase = ?coordinator.phase(), "FLOW scan ignored while a session is active");
                return true;
            }
            let _ = events.send(DeviceEvent::Phase(SessionPhase::Scanning));
            match scan_devices(duration).await {
                Ok(devices) => {
                    let _ = events.send(DeviceEvent::ScanResults(devices));
                    let _ = events.send(DeviceEvent::Phase(coordinator.phase()));
                }
                Err(error) => {
                    let _ = events.send(DeviceEvent::Error(error.to_string()));
                }
            }
        }
        DeviceCommand::Connect => {
            if session.is_some()
                || !matches!(
                    coordinator.phase(),
                    SessionPhase::Disconnected | SessionPhase::Error
                )
            {
                info!(phase = ?coordinator.phase(), "FLOW connect ignored while a session is active");
                return true;
            }
            match client_identity::load_or_create() {
                Ok(client_id) => coordinator.set_client_id(client_id),
                Err(error) => {
                    coordinator.transition(SessionPhase::Error);
                    let _ = events.send(DeviceEvent::Phase(SessionPhase::Error));
                    let _ = events.send(DeviceEvent::Error(error));
                    return true;
                }
            }
            if let Some(active) = session.take() {
                active.disconnect("connection_replaced").await;
            }
            *transport_rx = None;
            coordinator.transition(SessionPhase::Disconnected);
            *generation = generation.wrapping_add(1);
            let _ = events.send(DeviceEvent::Phase(SessionPhase::Connecting));
            coordinator.transition(SessionPhase::Connecting);
            match RuntimeSession::connect(*generation, events.clone()).await {
                Ok((connected, rx)) => {
                    let connected = match ActiveSession::new(connected, events.clone()).await {
                        Ok(connected) => connected,
                        Err(error) => {
                            coordinator.transition(SessionPhase::Error);
                            let _ = events.send(DeviceEvent::Phase(SessionPhase::Error));
                            let _ = events.send(DeviceEvent::Error(error.to_string()));
                            return true;
                        }
                    };
                    coordinator.transition(SessionPhase::GattReady);
                    let _ = events.send(DeviceEvent::Phase(SessionPhase::GattReady));
                    let _ = events.send(DeviceEvent::Backend(connected.backend));
                    let _ = events.send(DeviceEvent::Mtu(connected.mtu));
                    let _ = events.send(DeviceEvent::WriteMode(connected.write_type));
                    coordinator.transition(SessionPhase::RxArming);
                    let _ = events.send(DeviceEvent::Phase(SessionPhase::RxArming));
                    info!(
                        backend = connected.backend,
                        mtu = connected.mtu,
                        write_type = ?connected.write_type,
                        "FLOW 8 RX path armed"
                    );
                    *transport_rx = Some(rx);
                    *session = Some(connected);
                    let action = coordinator.rx_armed();
                    execute_actions(vec![action], session.as_ref(), events);
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
            if let Some(active) = session.take() {
                active.disconnect("explicit_user_disconnect").await;
            }
            *transport_rx = None;
            let action = coordinator.transition(SessionPhase::Disconnected);
            execute_actions(vec![action], None, events);
        }
        DeviceCommand::Send(command) => {
            if coordinator.phase() != SessionPhase::Ready {
                warn!(command = ?command, phase = ?coordinator.phase(), "FLOW user command ignored before Ready");
                let _ = events.send(DeviceEvent::Error(format!(
                    "FLOW 8 command rejected while session is {:?}; wait for Ready",
                    coordinator.phase()
                )));
                return true;
            }
            if matches!(command, TxCommand::GetMixerState) {
                info!(source = "user_semantic_request", phase = ?coordinator.phase(), "FLOW 0x37 action source");
            }
            let key = ContinuousKey::for_command(&command);
            match coordinator.encode_command(&command) {
                Ok(actions) => {
                    if matches!(command, TxCommand::GetMixerState) {
                        let phase = coordinator.transition(SessionPhase::StateSyncing);
                        coordinator.state_request_pending_or_sent = true;
                        execute_actions(vec![phase], session.as_ref(), events);
                    }
                    execute_actions_with_key(actions, session.as_ref(), events, key);
                }
                Err(error) => {
                    let _ = events.send(DeviceEvent::Error(error.to_string()));
                }
            }
        }
        DeviceCommand::StateApplied => match coordinator.state_applied() {
            Ok(action) => {
                info!("FLOW 8 Store apply acknowledged; session reached Ready");
                execute_actions(vec![action], session.as_ref(), events);
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
    session: &mut Option<ActiveSession>,
    transport_rx: &mut Option<mpsc::UnboundedReceiver<TransportRx>>,
    coordinator: &mut Flow8Session,
    events: &EventSender,
) {
    let generation = match &event {
        TransportRx::Packet { generation, .. }
        | TransportRx::Disconnected { generation }
        | TransportRx::Overflow { generation } => *generation,
        #[cfg(target_os = "windows")]
        TransportRx::Connected { generation }
        | TransportRx::Error { generation, .. }
        | TransportRx::HandshakeTimeout { generation } => *generation,
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
            if bytes.first() == Some(&0x38) {
                info!(
                    generation = active_generation,
                    bytes = bytes.len(),
                    "FLOW 0x38 runtime ingress"
                );
            }
            if let Ok(packet) = flow8_protocol::parse_packet(&bytes) {
                info!(
                    command = packet.command,
                    fragment_index = ?packet.fragment_index,
                    fragment_count = packet.fragment_count,
                    bytes = bytes.len(),
                    "FLOW RX"
                );
                if packet.command == 0x35
                    && let Some(active) = session.as_ref()
                {
                    active.mark_handshake_rx();
                }
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
            execute_actions(actions, session.as_ref(), events);
            debug!(
                command = bytes.first().copied().unwrap_or_default(),
                "FLOW RX processed"
            );
        }
        #[cfg(target_os = "windows")]
        TransportRx::Connected { .. } => {
            info!("FLOW 8 physical connection established");
        }
        TransportRx::Overflow { .. } => {
            warn!(
                limit = MAX_QUEUED_RX_PACKETS,
                "FLOW 8 RX queue exceeded capacity; closing session to avoid losing protocol state"
            );
            if let Some(active) = session.take() {
                active.disconnect("rx_queue_overflow").await;
            }
            *transport_rx = None;
            coordinator.transition(SessionPhase::Disconnected);
            execute_actions(
                vec![
                    coordinator.transition(SessionPhase::Error),
                    SessionAction::Error(
                        "FLOW 8 RX queue overflow; reconnect to resynchronize device state".into(),
                    ),
                ],
                None,
                events,
            );
        }
        TransportRx::Disconnected { .. } => {
            info!("FLOW 8 disconnected; invalidating session state");
            if let Some(active) = session.take() {
                active.disconnect("transport_rx_disconnected").await;
            }
            *transport_rx = None;
            let action = coordinator.transition(SessionPhase::Disconnected);
            execute_actions(vec![action], None, events);
        }
        #[cfg(target_os = "windows")]
        TransportRx::Error { message, .. } => {
            if let Some(active) = session.take() {
                active.disconnect("transport_error").await;
            }
            *transport_rx = None;
            coordinator.transition(SessionPhase::Disconnected);
            execute_actions(
                vec![
                    coordinator.transition(SessionPhase::Error),
                    SessionAction::Error(message),
                ],
                None,
                events,
            );
        }
        #[cfg(target_os = "windows")]
        TransportRx::HandshakeTimeout { .. } => {
            let message = "FLOW 8 RX timeout: native RX remains armed, but no 0x35 was observed; the connection is being kept open for a hardware retransmission";
            tracing::warn!(
                stage = NativeConnectionStage::Handshaking.as_str(),
                api = "BluetoothGATTRegisterEvent callback",
                logical_code = "handshake_rx_timeout",
                retry = false,
                "{message}"
            );
            let _ = events.send(DeviceEvent::Error(message.into()));
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
    mut user_commands: mpsc::Receiver<TxCommand>,
    events: EventSender,
) {
    let startup_client_id = match client_identity::load_or_create() {
        Ok(client_id) => client_id,
        Err(error) => {
            let _ = events.send(DeviceEvent::Error(error));
            // Connect retries loading the identity and refuses to send if it still fails.
            [0; 16]
        }
    };
    let mut coordinator = Flow8Session::new(startup_client_id);
    let mut session: Option<ActiveSession> = None;
    let mut transport_rx: Option<mpsc::UnboundedReceiver<TransportRx>> = None;
    let mut generation = 0u64;

    loop {
        if events.overflowed() {
            warn!("FLOW GUI event queue overflow; closing the active transport session");
            break;
        }
        if let Some(rx) = transport_rx.as_mut() {
            tokio::select! {
                _ = events.overflow_notify.notified() => continue,
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
                            if let Some(active) = session.take() {
                                active.disconnect("rx_channel_closed").await;
                            }
                            let action = coordinator.transition(SessionPhase::Disconnected);
                            execute_actions(vec![action], None, &events);
                        }
                    }
                }
                command = user_commands.recv() => {
                    let Some(command) = command else { break; };
                    if !handle_device_command(
                        DeviceCommand::Send(command),
                        &mut session,
                        &mut transport_rx,
                        &mut coordinator,
                        &mut generation,
                        &events,
                    ).await {
                        break;
                    }
                }
            }
        } else {
            let command = tokio::select! {
                biased;
                _ = events.overflow_notify.notified() => continue,
                command = commands.recv() => command,
                command = user_commands.recv() => command.map(DeviceCommand::Send),
            };
            let Some(command) = command else {
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
    if let Some(active) = session.take() {
        active.disconnect("runtime_actor_exit").await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow8_core::Flow8Store;
    use flow8_model::EvidenceStatus;
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
    fn scan_cannot_preempt_an_active_session() {
        assert!(!scan_allowed(SessionPhase::Ready, true));
        assert!(!scan_allowed(SessionPhase::StateSyncing, true));
        assert!(!scan_allowed(SessionPhase::Error, true));
        assert!(!scan_allowed(SessionPhase::Connecting, false));
        assert!(scan_allowed(SessionPhase::Disconnected, false));
        assert!(scan_allowed(SessionPhase::Error, false));
    }

    fn queue_protocol_command(mailbox: &WriterMailbox, command: TxCommand) {
        let key = ContinuousKey::for_command(&command);
        let frame = flow8_protocol::encode(&command).unwrap();
        mailbox.push(WriterCommand::Write { frame, key }).unwrap();
    }

    #[test]
    fn pending_continuous_route_keeps_latest_value_per_source_and_destination() {
        use flow8_model::{InputId, MixDestination};

        let mailbox = WriterMailbox::new();
        let route = |destination, normalized| TxCommand::RouteLevel {
            source: InputId::Input1,
            destination,
            normalized,
        };
        queue_protocol_command(&mailbox, route(MixDestination::Main, 0.2));
        queue_protocol_command(&mailbox, route(MixDestination::Monitor1, 0.4));
        queue_protocol_command(&mailbox, route(MixDestination::Main, 0.8));

        let state = mailbox.state.lock().unwrap();
        assert_eq!(state.pending.len(), 2);
        let frames: Vec<_> = state
            .pending
            .iter()
            .map(|command| match command {
                WriterCommand::Write { frame, .. } => frame.clone(),
                WriterCommand::MarkHandshakeRx => panic!("unexpected marker"),
            })
            .collect();
        assert_eq!(
            frames,
            vec![
                flow8_protocol::encode(&route(MixDestination::Main, 0.8)).unwrap(),
                flow8_protocol::encode(&route(MixDestination::Monitor1, 0.4)).unwrap(),
            ]
        );
    }

    #[test]
    fn discrete_write_is_fifo_barrier_for_continuous_coalescing() {
        use flow8_model::{InputId, MixDestination};

        let mailbox = WriterMailbox::new();
        let route = |normalized| TxCommand::RouteLevel {
            source: InputId::Input1,
            destination: MixDestination::Main,
            normalized,
        };
        let mute = TxCommand::Mute {
            endpoint: InputId::Input1.endpoint(),
            enabled: true,
        };
        queue_protocol_command(&mailbox, route(0.2));
        queue_protocol_command(&mailbox, mute.clone());
        queue_protocol_command(&mailbox, route(0.8));

        let state = mailbox.state.lock().unwrap();
        let frames: Vec<_> = state
            .pending
            .iter()
            .map(|command| match command {
                WriterCommand::Write { frame, .. } => frame.clone(),
                WriterCommand::MarkHandshakeRx => panic!("unexpected marker"),
            })
            .collect();
        assert_eq!(
            frames,
            vec![
                flow8_protocol::encode(&route(0.2)).unwrap(),
                flow8_protocol::encode(&mute).unwrap(),
                flow8_protocol::encode(&route(0.8)).unwrap(),
            ]
        );
    }

    #[test]
    fn captured_host_hello_uses_the_configured_client_uuid_verbatim() {
        let client_id = [
            0x83, 0x76, 0xf6, 0xa4, 0xb8, 0x55, 0x78, 0xd1, 0xc7, 0x76, 0xf2, 0x7a, 0xc9, 0xd3,
            0xb4, 0x76,
        ];
        let captured_hello = [
            0x35, 0x01, 0xf4, 0x89, 0x52, 0xf2, 0xf3, 0x30, 0xac, 0x22, 0x38, 0xc8, 0xba, 0x48,
            0x70, 0x44, 0x13, 0x7c, 0x00, 0x00, 0x09, 0x2d, 0xe5, 0x48,
        ];
        let mut session = Flow8Session::new(client_id);
        session.rx_armed();
        let actions = session.notification(&captured_hello);

        assert!(actions.iter().any(|action| matches!(
            action,
            SessionAction::Received(RxCommand::HandshakeHost {
                device_id,
                pairing_any: false,
                protocol_version: 9,
                firmware_build: 0x2de5,
            }) if *device_id == [
                0xf4, 0x89, 0x52, 0xf2, 0xf3, 0x30, 0xac, 0x22,
                0x38, 0xc8, 0xba, 0x48, 0x70, 0x44, 0x13, 0x7c,
            ]
        )));
        let expected = flow8_protocol::encode(&TxCommand::HandshakeClient { client_id }).unwrap();
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame == &expected))
        );
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
    fn handshake_responses_are_gated_by_rx_arm_and_received_order() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        let mut hello = vec![0x11; 16];
        hello.extend([0, 0, 1, 0, 2]);

        let before_arm = session.notification(&frame_single(0x35, &hello));
        assert!(
            !before_arm
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame[0] == 0x39))
        );

        session.rx_armed();
        let reply_before_hello = session.notification(&frame_single(0x36, &[]));
        assert!(
            !reply_before_hello
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame[0] == 0x37))
        );

        let hello_actions = session.notification(&frame_single(0x35, &hello));
        assert!(
            hello_actions
                .iter()
                .any(|action| matches!(action, SessionAction::Send(frame) if frame[0] == 0x39))
        );
        let reply_actions = session.notification(&frame_single(0x36, &[]));
        assert!(
            reply_actions
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
    fn four_fragment_state_is_atomically_applied_before_ready() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        let mut store = Flow8Store::disconnected();
        session.transition(SessionPhase::StateSyncing);
        let frames = encode_frames(&TxCommand::MixerState(Box::new(mixer())), 128, 3).unwrap();
        assert_eq!(frames.len(), 4);

        for frame in &frames[..3] {
            assert!(session.notification(frame).is_empty());
            assert_eq!(session.phase(), SessionPhase::StateSyncing);
        }
        let actions = session.notification(&frames[3]);
        let command = actions.into_iter().find_map(|action| match action {
            SessionAction::Received(command @ RxCommand::MixerState(_)) => Some(command),
            _ => None,
        });
        store
            .apply_rx(
                command.expect("complete four-fragment MixerState"),
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();

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
        assert!(matches!(actions.as_slice(), [SessionAction::Warning(_)]));
    }

    #[test]
    fn unsupported_fragmented_rx_does_not_disable_ready_session() {
        let mut session = Flow8Session::new(*b"FLOW8-PC-RUST001");
        session.transition(SessionPhase::Ready);
        let frames = flow8_protocol::frame_command(0x45, &[0; 130], 128, 2).unwrap();
        assert_eq!(frames.len(), 2);
        for frame in frames {
            assert!(session.notification(&frame).is_empty());
            assert_eq!(session.phase(), SessionPhase::Ready);
        }
        assert!(matches!(
            session
                .notification(&frame_single(0x31, &[0x0c, 0x00]))
                .as_slice(),
            [SessionAction::Received(RxCommand::FxPreset { .. })]
        ));
        assert!(matches!(
            session
                .notification(&frame_single(0x11, &[0x0c, 0x12, 0, 0, 1]))
                .as_slice(),
            [SessionAction::Received(RxCommand::FxSetup { .. })]
        ));
        assert_eq!(session.phase(), SessionPhase::Ready);
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
            Ok(TransportRx::Packet { generation: 7, bytes, .. })
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
    fn connection_failure_requires_an_explicit_manual_retry() {
        assert!(!AUTO_RECONNECT_AFTER_FAILURE);
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
            Ok(TransportRx::Packet { generation: 31, bytes, .. }) if bytes == vec![0x35]
        ));
    }
}
