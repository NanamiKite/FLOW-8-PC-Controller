use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use btleplug::api::{CharPropFlags, WriteType, bleuuid::uuid_from_u16};
use clap::{Parser, Subcommand};
use flow8_ble::{
    BleTransport, CHARACTERISTIC_UUID, Flow8BleSession, Flow8Session, SERVICE_UUID, SessionAction,
    SessionPhase,
};
use flow8_core::Flow8Store;
use flow8_model::{EvidenceStatus, InputId, MixDestination};
use flow8_protocol::{CommandStreamDecoder, RxCommand, TxCommand, encode, parse_packet};
use futures_util::StreamExt;

#[cfg(target_os = "windows")]
mod winrt_gatt_probe;
#[cfg(target_os = "windows")]
mod winrt_passive_handshake;
#[cfg(target_os = "windows")]
mod winrt_prearmed_handshake;

#[derive(Parser)]
#[command(
    name = "flow8-hardware-bringup",
    about = "Manual FLOW 8 BLE bring-up and evidence capture"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Scan {
        #[arg(long, default_value_t = 5)]
        seconds: u64,
    },
    Inspect,
    /// Enumerate and safely read the complete discovered GATT hierarchy. This
    /// command never subscribes and never writes a characteristic or descriptor.
    InspectGatt {
        #[arg(long, default_value_t = 5_000)]
        descriptor_timeout_ms: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Windows-only direct WinRT cached/uncached GATT and CCCD diagnostic.
    WinrtGattProbe {
        #[arg(long, default_value = "FLOW 8 LE")]
        device_name: String,
        #[arg(long, default_value_t = 3)]
        observe_seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Connect,
    Subscribe {
        #[arg(long, default_value_t = 15)]
        seconds: u64,
    },
    Handshake {
        #[arg(long, default_value_t = 20)]
        seconds: u64,
    },
    RequestState {
        #[arg(long, default_value_t = 20)]
        seconds: u64,
    },
    Show {
        #[arg(long, default_value_t = 20)]
        seconds: u64,
    },
    Route {
        source: u8,
        destination: String,
        normalized: f32,
    },
    Gain {
        input: u8,
        db: f32,
    },
    Mute {
        endpoint: u8,
        enabled: bool,
    },
    Pan {
        input: u8,
        value: f32,
    },
    Solo {
        input: u8,
        enabled: bool,
    },
    Capture {
        path: PathBuf,
        #[arg(long, default_value_t = 30)]
        seconds: u64,
        #[arg(long)]
        handshake: bool,
    },
    /// Compare fresh-connection write/subscribe ordering without sending any
    /// mixer-control command.
    ProbeHandshakeOrder {
        #[arg(long, default_value_t = 250)]
        delay_ms: u64,
        #[arg(long, default_value_t = 2)]
        observe_seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Observe the device's unsolicited handshake without subscribing. On
    /// Windows, falls back to a direct WinRT ValueChanged handler that never
    /// writes a CCCD when btleplug exposes no unsolicited values.
    PassiveHandshake {
        #[arg(long, default_value = "FLOW 8 LE")]
        device_name: String,
        #[arg(long, default_value_t = 3)]
        btleplug_observe_seconds: u64,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Windows-only timing probe that resolves the target from cached GATT,
    /// arms WinRT ValueChanged, and only then initiates the connection.
    PrearmedHandshake {
        #[arg(long, default_value = "FLOW 8 LE")]
        device_name: String,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Default, Clone)]
struct PassiveAttemptResult {
    backend: &'static str,
    notification_count: usize,
    handshake_host_received: bool,
    handshake_reply_received: bool,
    mixer_state_applied: bool,
    ready: bool,
}

struct PassiveHandshakePipeline {
    coordinator: Flow8Session,
    store: Flow8Store,
    result: PassiveAttemptResult,
}

impl PassiveHandshakePipeline {
    fn new(backend: &'static str) -> Self {
        let mut coordinator = Flow8Session::new(*b"FLOW8-PC-RUST001");
        // FLOW 8 does not expose a CCCD. Handshaking begins when the receive
        // handler is attached, not after a synthetic subscription event.
        coordinator.transition(SessionPhase::Handshaking);
        Self {
            coordinator,
            store: Flow8Store::disconnected(),
            result: PassiveAttemptResult {
                backend,
                ..PassiveAttemptResult::default()
            },
        }
    }

    fn is_ready(&self) -> bool {
        self.result.ready
    }

    fn result(&self) -> PassiveAttemptResult {
        self.result.clone()
    }

    /// Feed one raw characteristic value through the production session
    /// decoder/reassembler and the production Flow8Store state applier.
    /// Returned frames are produced only by the existing handshake codec.
    fn accept(
        &mut self,
        raw: &[u8],
        characteristic_uuid: impl std::fmt::Display,
        connected_at: Instant,
        logger: &mut DiagnosticLogger,
    ) -> Vec<Vec<u8>> {
        self.result.notification_count += 1;
        let stamp = unix_timestamp_ms();
        logger.line(format!(
            "timestamp_ms={stamp} backend={} {} RX characteristic={} raw={} evidence=VERIFIED_FROM_DEVICE",
            self.result.backend,
            probe_elapsed(connected_at),
            characteristic_uuid,
            diagnostic_hex(raw),
        ));
        match parse_packet(raw) {
            Ok(packet) => logger.line(format!(
                "timestamp_ms={stamp} backend={} {} FLOW_FRAME command=0x{:02X} fragment_count={} sequence={} fragment_index={} fragment_payload_bytes={} raw_bytes={} evidence=VERIFIED_FROM_DEVICE",
                self.result.backend,
                probe_elapsed(connected_at),
                packet.command,
                packet.fragment_count,
                packet.sequence.map_or_else(|| "N/A".to_owned(), |value| value.to_string()),
                packet.fragment_index.map_or_else(|| "N/A".to_owned(), |value| value.to_string()),
                packet.payload.len(),
                raw.len(),
            )),
            Err(error) => logger.line(format!(
                "timestamp_ms={stamp} backend={} {} FLOW_FRAME parse=FAIL error={error} raw_retained=true evidence=VERIFIED_FROM_DEVICE",
                self.result.backend,
                probe_elapsed(connected_at),
            )),
        }

        let mut writes = Vec::new();
        for action in self.coordinator.notification(raw) {
            match action {
                SessionAction::Received(command) => {
                    match &command {
                        RxCommand::HandshakeHost { .. } => {
                            self.result.handshake_host_received = true;
                        }
                        RxCommand::HandshakeReply => {
                            self.result.handshake_reply_received = true;
                        }
                        _ => {}
                    }
                    logger.line(format!(
                        "timestamp_ms={stamp} backend={} {} decoded={} evidence=VERIFIED_FROM_DEVICE",
                        self.result.backend,
                        probe_elapsed(connected_at),
                        decoded_command_summary(&command),
                    ));
                    let is_mixer_state = matches!(command, RxCommand::MixerState(_));
                    match self
                        .store
                        .apply_rx(command, EvidenceStatus::VerifiedFromDevice)
                    {
                        Ok(()) => {
                            logger.line(format!(
                                "timestamp_ms={stamp} backend={} {} state_apply=SUCCESS confirmed_source=DEVICE atomic={} evidence=VERIFIED_FROM_DEVICE",
                                self.result.backend,
                                probe_elapsed(connected_at),
                                is_mixer_state,
                            ));
                            if is_mixer_state {
                                self.result.mixer_state_applied = true;
                                match self.coordinator.state_applied() {
                                    Ok(SessionAction::Phase(SessionPhase::Ready)) => {
                                        self.result.ready = true;
                                        logger.line(format!(
                                            "timestamp_ms={stamp} backend={} {} session=READY gate=COMPLETE_VALID_0x38_ATOMICALLY_APPLIED evidence=VERIFIED_FROM_DEVICE",
                                            self.result.backend,
                                            probe_elapsed(connected_at),
                                        ));
                                    }
                                    Ok(action) => logger.line(format!(
                                        "timestamp_ms={stamp} backend={} {} state_apply_ack=UNEXPECTED action={action:?}",
                                        self.result.backend,
                                        probe_elapsed(connected_at),
                                    )),
                                    Err(error) => logger.line(format!(
                                        "timestamp_ms={stamp} backend={} {} state_apply_ack=FAIL error={error}",
                                        self.result.backend,
                                        probe_elapsed(connected_at),
                                    )),
                                }
                            }
                        }
                        Err(error) => logger.line(format!(
                            "timestamp_ms={stamp} backend={} {} state_apply=FAIL error={error} atomic={} evidence=VERIFIED_FROM_DEVICE",
                            self.result.backend,
                            probe_elapsed(connected_at),
                            is_mixer_state,
                        )),
                    }
                }
                SessionAction::Send(frame) => {
                    // This diagnostic may emit only the protocol-defined 0x39
                    // response and 0x37 full-state request. It never serializes
                    // these bytes itself and never permits a mixer command.
                    if matches!(frame.first(), Some(0x39 | 0x37)) {
                        writes.push(frame);
                    } else {
                        logger.line(format!(
                            "timestamp_ms={stamp} backend={} {} TX=BLOCKED reason=NON_HANDSHAKE_COMMAND raw={} evidence=IMPLEMENTATION_SAFETY",
                            self.result.backend,
                            probe_elapsed(connected_at),
                            diagnostic_hex(&frame),
                        ));
                    }
                }
                SessionAction::Phase(phase) => {
                    if phase == SessionPhase::Ready {
                        if self.result.mixer_state_applied {
                            self.result.ready = true;
                            logger.line(format!(
                                "timestamp_ms={stamp} backend={} {} session=READY gate=COMPLETE_VALID_0x38_ATOMICALLY_APPLIED evidence=VERIFIED_FROM_DEVICE",
                                self.result.backend,
                                probe_elapsed(connected_at),
                            ));
                        } else {
                            logger.line(format!(
                                "timestamp_ms={stamp} backend={} {} session=READY_REJECTED reason=0x38_NOT_ATOMICALLY_APPLIED evidence=VERIFIED_FROM_DEVICE",
                                self.result.backend,
                                probe_elapsed(connected_at),
                            ));
                        }
                    } else {
                        logger.line(format!(
                            "timestamp_ms={stamp} backend={} {} session={phase:?} evidence=VERIFIED_FROM_DEVICE",
                            self.result.backend,
                            probe_elapsed(connected_at),
                        ));
                    }
                }
                SessionAction::Warning(warning) => logger.line(format!(
                    "timestamp_ms={stamp} backend={} {} decode_or_reassembly=WARNING warning={warning} raw_retained=true evidence=VERIFIED_FROM_DEVICE",
                    self.result.backend,
                    probe_elapsed(connected_at),
                )),
                SessionAction::Error(error) => logger.line(format!(
                    "timestamp_ms={stamp} backend={} {} decode_or_reassembly=FAIL error={error} raw_retained=true evidence=VERIFIED_FROM_DEVICE",
                    self.result.backend,
                    probe_elapsed(connected_at),
                )),
            }
        }
        writes
    }
}

#[derive(Debug, Clone)]
enum ProbeStep {
    Write {
        label: &'static str,
        command: TxCommand,
    },
    Delay(Duration),
}

#[derive(Debug, Clone)]
struct ProbeCase {
    number: usize,
    name: &'static str,
    steps: Vec<ProbeStep>,
}

#[derive(Debug)]
struct ProbeResult {
    number: usize,
    name: &'static str,
    connected: bool,
    write_success: Option<bool>,
    subscribe_success: bool,
    first_notification: bool,
    decoded_handshake_response: bool,
}

struct DiagnosticLogger {
    file: Option<File>,
}

impl DiagnosticLogger {
    fn new(path: Option<PathBuf>) -> Result<Self, std::io::Error> {
        let file = path
            .map(|path| {
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)?;
                }
                File::create(path)
            })
            .transpose()?;
        Ok(Self { file })
    }

    fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        println!("{text}");
        if let Some(file) = &mut self.file {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default();
            let _ = writeln!(file, "{timestamp} {text}");
            let _ = file.flush();
        }
    }
}

fn handshake_host_probe() -> TxCommand {
    // This is the exact zero-object vector documented for the APK-confirmed
    // 0x35 descriptor. It is a diagnostic probe, not a claimed client role.
    TxCommand::HandshakeHostProbe {
        device_id: [0; 16],
        pairing_any: false,
        protocol_version: 0,
        firmware_build: 0,
    }
}

fn handshake_probe_cases(delay: Duration) -> Vec<ProbeCase> {
    vec![
        ProbeCase {
            number: 1,
            name: "connect -> discover -> subscribe",
            steps: Vec::new(),
        },
        ProbeCase {
            number: 2,
            name: "connect -> discover -> write 0x35 -> subscribe",
            steps: vec![ProbeStep::Write {
                label: "APK 0x35 HandshakeHost zero vector",
                command: handshake_host_probe(),
            }],
        },
        ProbeCase {
            number: 3,
            name: "connect -> discover -> write 0x36 -> subscribe",
            steps: vec![ProbeStep::Write {
                label: "APK 0x36 HandshakeReply",
                command: TxCommand::HandshakeReplyProbe,
            }],
        },
        ProbeCase {
            number: 4,
            name: "connect -> discover -> write 0x35 -> delay -> subscribe",
            steps: vec![
                ProbeStep::Write {
                    label: "APK 0x35 HandshakeHost zero vector",
                    command: handshake_host_probe(),
                },
                ProbeStep::Delay(delay),
            ],
        },
        ProbeCase {
            number: 5,
            name: "connect -> discover -> write 0x35 -> write 0x36 -> subscribe",
            steps: vec![
                ProbeStep::Write {
                    label: "APK 0x35 HandshakeHost zero vector",
                    command: handshake_host_probe(),
                },
                ProbeStep::Write {
                    label: "APK 0x36 HandshakeReply",
                    command: TxCommand::HandshakeReplyProbe,
                },
            ],
        },
    ]
}

fn input(value: u8) -> Result<InputId, String> {
    InputId::ALL
        .into_iter()
        .find(|input| input.endpoint() == value)
        .ok_or_else(|| format!("invalid conventional input endpoint {value}"))
}

fn destination(value: &str) -> Result<MixDestination, String> {
    match value.to_ascii_lowercase().as_str() {
        "main" | "15" => Ok(MixDestination::Main),
        "mon1" | "10" => Ok(MixDestination::Monitor1),
        "mon2" | "11" => Ok(MixDestination::Monitor2),
        "fx1" | "12" => Ok(MixDestination::Fx1),
        "fx2" | "13" => Ok(MixDestination::Fx2),
        _ => Err(format!("invalid destination {value}")),
    }
}

fn write_type(session: &Flow8BleSession) -> WriteType {
    if session
        .characteristic()
        .properties
        .contains(CharPropFlags::WRITE)
    {
        WriteType::WithResponse
    } else {
        WriteType::WithoutResponse
    }
}

fn probe_elapsed(connected_at: Instant) -> String {
    format!("+{} ms", connected_at.elapsed().as_millis())
}

fn is_handshake_event(command: &RxCommand) -> bool {
    matches!(
        command,
        RxCommand::HandshakeHost { .. }
            | RxCommand::HandshakeReply
            | RxCommand::HandshakeClient { .. }
    )
}

async fn run_handshake_probe_case(
    case: &ProbeCase,
    observe_for: Duration,
    logger: &mut DiagnosticLogger,
) -> ProbeResult {
    logger.line("");
    logger.line(format!(
        "CASE {}: {} evidence=VERIFIED_FROM_DEVICE",
        case.number, case.name
    ));
    let mut result = ProbeResult {
        number: case.number,
        name: case.name,
        connected: false,
        write_success: None,
        subscribe_success: false,
        first_notification: false,
        decoded_handshake_response: false,
    };

    let connect_attempt = Instant::now();
    let transport = match BleTransport::new().await {
        Ok(transport) => transport,
        Err(error) => {
            logger.line(format!(
                "case={} connect=FAIL attempt_elapsed={} ms error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                case.number,
                connect_attempt.elapsed().as_millis(),
                error,
            ));
            return result;
        }
    };
    let session = match transport.connect_flow8().await {
        Ok(session) => session,
        Err(error) => {
            logger.line(format!(
                "case={} connect=FAIL attempt_elapsed={} ms error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                case.number,
                connect_attempt.elapsed().as_millis(),
                error,
            ));
            return result;
        }
    };
    let connected_at = Instant::now();
    result.connected = true;
    let characteristic = session.characteristic();
    let mode = write_type(&session);
    logger.line(format!(
        "case={} {} connect=SUCCESS evidence=VERIFIED_FROM_DEVICE",
        case.number,
        probe_elapsed(connected_at)
    ));
    logger.line(format!(
        "case={} {} discovered service={} characteristic={} evidence=VERIFIED_FROM_DEVICE",
        case.number,
        probe_elapsed(connected_at),
        characteristic.service_uuid,
        characteristic.uuid,
    ));
    logger.line(format!(
        "case={} {} properties={:?} mtu={} selected_write_type={mode:?} evidence=VERIFIED_FROM_DEVICE",
        case.number,
        probe_elapsed(connected_at),
        characteristic.properties,
        session.mtu(),
    ));

    let mut write_attempted = false;
    let mut every_write_succeeded = true;
    for step in &case.steps {
        match step {
            ProbeStep::Write { label, command } => {
                write_attempted = true;
                let frame = match encode(command) {
                    Ok(frame) => frame,
                    Err(error) => {
                        every_write_succeeded = false;
                        logger.line(format!(
                            "case={} {} encode=FAIL command={label:?} error={error} evidence=VERIFIED_FROM_APK",
                            case.number,
                            probe_elapsed(connected_at),
                        ));
                        continue;
                    }
                };
                logger.line(format!(
                    "case={} {} TX command={label:?} raw={} write_type={mode:?} schema_evidence=VERIFIED_FROM_APK",
                    case.number,
                    probe_elapsed(connected_at),
                    hex(&frame),
                ));
                match session.write(&frame, mode).await {
                    Ok(()) => logger.line(format!(
                        "case={} {} write=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                        case.number,
                        probe_elapsed(connected_at),
                    )),
                    Err(error) => {
                        every_write_succeeded = false;
                        logger.line(format!(
                            "case={} {} write=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                            case.number,
                            probe_elapsed(connected_at),
                            error,
                        ));
                    }
                }
            }
            ProbeStep::Delay(duration) => {
                logger.line(format!(
                    "case={} {} delay={} ms",
                    case.number,
                    probe_elapsed(connected_at),
                    duration.as_millis(),
                ));
                tokio::time::sleep(*duration).await;
            }
        }
    }
    result.write_success = write_attempted.then_some(every_write_succeeded);

    let mut notification_stream = match session.notifications().await {
        Ok(stream) => {
            logger.line(format!(
                "case={} {} notification_stream=READY before subscribe",
                case.number,
                probe_elapsed(connected_at),
            ));
            Some(stream)
        }
        Err(error) => {
            logger.line(format!(
                "case={} {} notification_stream=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                case.number,
                probe_elapsed(connected_at),
                error,
            ));
            None
        }
    };
    logger.line(format!(
        "case={} {} subscribe=ATTEMPT",
        case.number,
        probe_elapsed(connected_at),
    ));
    match tokio::time::timeout(Duration::from_secs(10), session.subscribe()).await {
        Ok(Ok(())) => {
            result.subscribe_success = true;
            logger.line(format!(
                "case={} {} subscribe=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                case.number,
                probe_elapsed(connected_at),
            ));
        }
        Ok(Err(error)) => logger.line(format!(
            "case={} {} subscribe=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
            case.number,
            probe_elapsed(connected_at),
            error,
        )),
        Err(error) => logger.line(format!(
            "case={} {} subscribe=TIMEOUT error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
            case.number,
            probe_elapsed(connected_at),
            error,
        )),
    }

    if result.subscribe_success {
        match notification_stream.as_mut() {
            Some(notifications) => {
                let observe_started = Instant::now();
                let mut decoder = CommandStreamDecoder::default();
                while let Some(remaining) = observe_for.checked_sub(observe_started.elapsed()) {
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(remaining, notifications.next()).await {
                        Ok(Some(notification)) => {
                            result.first_notification = true;
                            logger.line(format!(
                                "case={} {} RX raw={} evidence=VERIFIED_FROM_DEVICE",
                                case.number,
                                probe_elapsed(connected_at),
                                hex(&notification.value),
                            ));
                            match decoder.accept(&notification.value) {
                                Ok(Some(command)) => {
                                    if is_handshake_event(&command) {
                                        result.decoded_handshake_response = true;
                                    }
                                    logger.line(format!(
                                        "case={} {} decoded={command:?} evidence=VERIFIED_FROM_DEVICE",
                                        case.number,
                                        probe_elapsed(connected_at),
                                    ));
                                }
                                Ok(None) => logger.line(format!(
                                    "case={} {} decoded=FRAGMENT_BUFFERED evidence=VERIFIED_FROM_DEVICE",
                                    case.number,
                                    probe_elapsed(connected_at),
                                )),
                                Err(error) => logger.line(format!(
                                    "case={} {} decode=FAIL error={error} raw_retained=true evidence=VERIFIED_FROM_DEVICE",
                                    case.number,
                                    probe_elapsed(connected_at),
                                )),
                            }
                        }
                        Ok(None) => {
                            logger.line(format!(
                                "case={} {} notification_stream=ENDED evidence=VERIFIED_FROM_DEVICE",
                                case.number,
                                probe_elapsed(connected_at),
                            ));
                            break;
                        }
                        Err(_) => break,
                    }
                }
                if !result.first_notification {
                    logger.line(format!(
                        "case={} {} first_notification=NONE within={} ms evidence=VERIFIED_FROM_DEVICE",
                        case.number,
                        probe_elapsed(connected_at),
                        observe_for.as_millis(),
                    ));
                }
            }
            None => logger.line(format!(
                "case={} {} notification_observation=UNAVAILABLE evidence=VERIFIED_FROM_DEVICE",
                case.number,
                probe_elapsed(connected_at),
            )),
        }
    }

    if let Err(error) = session.disconnect().await {
        logger.line(format!(
            "case={} {} disconnect=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE; retrying once",
            case.number,
            probe_elapsed(connected_at),
            error,
        ));
        tokio::time::sleep(Duration::from_millis(250)).await;
        match session.disconnect().await {
            Ok(()) => logger.line(format!(
                "case={} {} disconnect_retry=SUCCESS evidence=VERIFIED_FROM_DEVICE",
                case.number,
                probe_elapsed(connected_at),
            )),
            Err(retry_error) => logger.line(format!(
                "case={} {} disconnect_retry=FAIL error={} debug={retry_error:?} evidence=VERIFIED_FROM_DEVICE; next case will still create a new btleplug session",
                case.number,
                probe_elapsed(connected_at),
                retry_error,
            )),
        }
    } else {
        logger.line(format!(
            "case={} {} disconnect=SUCCESS evidence=VERIFIED_FROM_DEVICE",
            case.number,
            probe_elapsed(connected_at),
        ));
    }
    result
}

fn probe_status(value: bool) -> &'static str {
    if value { "YES" } else { "NO" }
}

fn probe_write_status(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "YES",
        Some(false) => "NO",
        None => "N/A",
    }
}

async fn probe_handshake_order(
    delay: Duration,
    observe_for: Duration,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut logger = DiagnosticLogger::new(output)?;
    logger.line("FLOW 8 handshake-order probe");
    logger.line("Purpose: evidence collection only; no mixer-control command is sent.");
    logger.line("APK descriptor bytes are VERIFIED_FROM_APK; Windows/FLOW 8 outcomes, including failures, are VERIFIED_FROM_DEVICE observations.");
    let mut results = Vec::new();
    for case in handshake_probe_cases(delay) {
        results.push(run_handshake_probe_case(&case, observe_for, &mut logger).await);
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    logger.line("");
    logger.line("COMPARISON");
    logger.line(format!(
        "{:<4} {:<64} {:<9} {:<7} {:<9} {:<12} {:<18}",
        "Case", "Sequence", "Connected", "Write", "Subscribe", "First RX", "Handshake decoded"
    ));
    for result in results {
        logger.line(format!(
            "{:<4} {:<64} {:<9} {:<7} {:<9} {:<12} {:<18}",
            result.number,
            result.name,
            probe_status(result.connected),
            probe_write_status(result.write_success),
            probe_status(result.subscribe_success),
            probe_status(result.first_notification),
            probe_status(result.decoded_handshake_response),
        ));
    }
    logger.line("Comparison rows are retained hardware evidence for this device/backend run; they do not alter protocol semantics.");
    Ok(())
}

fn characteristic_property_names(properties: CharPropFlags) -> String {
    const FLAGS: &[(CharPropFlags, &str)] = &[
        (CharPropFlags::BROADCAST, "BROADCAST"),
        (CharPropFlags::READ, "READ"),
        (
            CharPropFlags::WRITE_WITHOUT_RESPONSE,
            "WRITE_WITHOUT_RESPONSE",
        ),
        (CharPropFlags::WRITE, "WRITE"),
        (CharPropFlags::NOTIFY, "NOTIFY"),
        (CharPropFlags::INDICATE, "INDICATE"),
        (
            CharPropFlags::AUTHENTICATED_SIGNED_WRITES,
            "AUTHENTICATED_SIGNED_WRITES",
        ),
        (CharPropFlags::EXTENDED_PROPERTIES, "EXTENDED_PROPERTIES"),
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

fn diagnostic_hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        "<empty>".to_owned()
    } else {
        hex(bytes)
    }
}

async fn inspect_gatt(
    descriptor_timeout: Duration,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut logger = DiagnosticLogger::new(output)?;
    let cccd_uuid = uuid_from_u16(0x2902);
    logger.line("FLOW 8 read-only GATT inspection");
    logger.line(format!(
        "backend={} policy=CONNECT+DISCOVER+READ_DESCRIPTORS_ONLY subscribe=NEVER characteristic_write=NEVER descriptor_write=NEVER protocol_tx=NEVER",
        std::env::consts::OS,
    ));
    logger.line("btleplug_public_api descriptor_enumeration=YES descriptor_read=YES descriptor_handle_or_backend_identifier=NOT_EXPOSED");
    logger.line("API_LIMITATION: btleplug public Descriptor exposes uuid, service_uuid, and characteristic_uuid only; Windows GATT handles and WinRT descriptor identifiers cannot be reported without a backend-specific API.");

    let attempt_started = Instant::now();
    let transport = match BleTransport::new().await {
        Ok(transport) => transport,
        Err(error) => {
            logger.line(format!(
                "connect=FAIL elapsed={} ms error={} debug={error:?}",
                attempt_started.elapsed().as_millis(),
                error,
            ));
            return Ok(());
        }
    };
    let session = match transport.connect_flow8().await {
        Ok(session) => session,
        Err(error) => {
            logger.line(format!(
                "connect=FAIL elapsed={} ms error={} debug={error:?}",
                attempt_started.elapsed().as_millis(),
                error,
            ));
            return Ok(());
        }
    };
    let connected_at = Instant::now();
    logger.line(format!(
        "{} connect=SUCCESS mtu={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(connected_at),
        session.mtu(),
    ));
    match session.device_info().await {
        Ok(device) => logger.line(format!(
            "{} device name={:?} id={} address={} rssi={:?} advertised_services={:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            device.name,
            device.id,
            device.address,
            device.rssi,
            device.services,
        )),
        Err(error) => logger.line(format!(
            "{} device_properties=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            error,
        )),
    }

    let services = session.discovered_services();
    let characteristic_count = services
        .iter()
        .map(|service| service.characteristics.len())
        .sum::<usize>();
    let descriptor_count = services
        .iter()
        .flat_map(|service| &service.characteristics)
        .map(|characteristic| characteristic.descriptors.len())
        .sum::<usize>();
    logger.line(format!(
        "{} hierarchy services={} characteristics={} descriptors={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(connected_at),
        services.len(),
        characteristic_count,
        descriptor_count,
    ));

    let mut target_characteristic_exposed = false;
    let mut target_cccd_exposed = false;
    for (service_index, service) in services.iter().enumerate() {
        logger.line(format!(
            "{} SERVICE index={} uuid={} primary={} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            service_index,
            service.uuid,
            service.primary,
        ));
        for (characteristic_index, characteristic) in service.characteristics.iter().enumerate() {
            let is_target = characteristic.service_uuid == SERVICE_UUID
                && characteristic.uuid == CHARACTERISTIC_UUID;
            target_characteristic_exposed |= is_target;
            logger.line(format!(
                "{}   CHARACTERISTIC index={} uuid={} service_uuid={} properties={} properties_debug={:?} target={} evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                characteristic_index,
                characteristic.uuid,
                characteristic.service_uuid,
                characteristic_property_names(characteristic.properties),
                characteristic.properties,
                is_target,
            ));
            if characteristic.descriptors.is_empty() {
                logger.line(format!(
                    "{}     DESCRIPTORS count=0 evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(connected_at),
                ));
            }
            for (descriptor_index, descriptor) in characteristic.descriptors.iter().enumerate() {
                let is_target_cccd = is_target && descriptor.uuid == cccd_uuid;
                target_cccd_exposed |= is_target_cccd;
                logger.line(format!(
                    "{}     DESCRIPTOR index={} uuid={} service_uuid={} characteristic_uuid={} target_cccd={} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(connected_at),
                    descriptor_index,
                    descriptor.uuid,
                    descriptor.service_uuid,
                    descriptor.characteristic_uuid,
                    is_target_cccd,
                ));
                logger.line(format!(
                    "{}       backend_handle_or_identifier=NOT_EXPOSED_BY_BTLEPLUG_PUBLIC_API evidence=LOCAL_API_CAPABILITY",
                    probe_elapsed(connected_at),
                ));
                logger.line(format!(
                    "{}       READ_DESCRIPTOR attempt timeout_ms={} evidence=VERIFIED_FROM_DEVICE",
                    probe_elapsed(connected_at),
                    descriptor_timeout.as_millis(),
                ));
                match tokio::time::timeout(
                    descriptor_timeout,
                    session.read_descriptor(descriptor),
                )
                .await
                {
                    Ok(Ok(value)) => logger.line(format!(
                        "{}       READ_DESCRIPTOR success raw={} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(connected_at),
                        diagnostic_hex(&value),
                    )),
                    Ok(Err(error)) => logger.line(format!(
                        "{}       READ_DESCRIPTOR failure error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(connected_at),
                        error,
                    )),
                    Err(error) => logger.line(format!(
                        "{}       READ_DESCRIPTOR timeout error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(connected_at),
                        error,
                    )),
                }
            }
        }
    }

    logger.line(format!(
        "{} TARGET characteristic_uuid={} service_uuid={} exposed_by_btleplug={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(connected_at),
        CHARACTERISTIC_UUID,
        SERVICE_UUID,
        probe_status(target_characteristic_exposed),
    ));
    logger.line(format!(
        "{} TARGET_CCCD uuid={} backend={} exposed_by_btleplug_current_backend={} evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(connected_at),
        cccd_uuid,
        std::env::consts::OS,
        probe_status(target_cccd_exposed),
    ));
    if !target_cccd_exposed {
        logger.line("TARGET_CCCD interpretation=NOT_PRESENT_IN_BTLEPLUG_DISCOVERED_HIERARCHY; this does not prove the physical GATT database lacks a CCCD because the public Windows backend may omit it.");
    }
    logger.line("SAFETY_RESULT subscribe_calls=0 characteristic_writes=0 descriptor_writes=0 protocol_packets_sent=0");

    match session.disconnect().await {
        Ok(()) => logger.line(format!(
            "{} disconnect=SUCCESS evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
        )),
        Err(error) => logger.line(format!(
            "{} disconnect=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            error,
        )),
    }
    Ok(())
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn decoded_command_summary(command: &RxCommand) -> String {
    match command {
        RxCommand::HandshakeHost {
            device_id,
            pairing_any,
            protocol_version,
            firmware_build,
        } => format!(
            "HandshakeHost(0x35 device_id={} pairing_any={pairing_any} protocol_version={protocol_version} firmware_build={firmware_build})",
            diagnostic_hex(device_id),
        ),
        RxCommand::HandshakeReply => "HandshakeReply(0x36)".to_owned(),
        RxCommand::MixerState(state) => format!(
            "MixerState(0x38 inputs={} outputs={} effects={} selected_output={} tempo_bpm={})",
            state.inputs.len(),
            state.outputs.len(),
            state.effects.len(),
            state.selected_output,
            state.tempo_bpm,
        ),
        other => format!("{other:?}"),
    }
}

async fn run_btleplug_passive_handshake(
    initial_observation: Duration,
    handshake_timeout: Duration,
    logger: &mut DiagnosticLogger,
) -> PassiveAttemptResult {
    let mut pipeline = PassiveHandshakePipeline::new("btleplug");
    let connect_attempt = Instant::now();
    logger.line("backend=btleplug connect=ATTEMPT subscribe=NEVER cccd_write=NEVER");
    let transport = match BleTransport::new().await {
        Ok(transport) => transport,
        Err(error) => {
            logger.line(format!(
                "backend=btleplug connect=FAIL elapsed_ms={} error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                connect_attempt.elapsed().as_millis(),
                error,
            ));
            return pipeline.result();
        }
    };
    let session = match transport.connect_flow8().await {
        Ok(session) => session,
        Err(error) => {
            logger.line(format!(
                "backend=btleplug connect=FAIL elapsed_ms={} error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                connect_attempt.elapsed().as_millis(),
                error,
            ));
            return pipeline.result();
        }
    };
    let connected_at = Instant::now();
    logger.line(format!(
        "backend=btleplug {} connect=SUCCESS service={} characteristic={} properties={:?} mtu={} subscribe_calls=0 evidence=VERIFIED_FROM_DEVICE",
        probe_elapsed(connected_at),
        session.characteristic().service_uuid,
        session.characteristic().uuid,
        session.characteristic().properties,
        session.mtu(),
    ));
    let mut notifications = match session.notifications().await {
        Ok(stream) => {
            logger.line(format!(
                "backend=btleplug {} notifications_stream=READY subscribe_calls=0 note=PUBLIC_STREAM_RECEIVER_ONLY evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
            ));
            Some(stream)
        }
        Err(error) => {
            logger.line(format!(
                "backend=btleplug {} notifications_stream=FAIL error={} debug={error:?} subscribe_calls=0 evidence=VERIFIED_FROM_DEVICE",
                probe_elapsed(connected_at),
                error,
            ));
            None
        }
    };

    if let Some(stream) = notifications.as_mut() {
        let overall_deadline = tokio::time::Instant::now() + handshake_timeout;
        let first_rx_deadline = tokio::time::Instant::now() + initial_observation;
        loop {
            if pipeline.is_ready() {
                break;
            }
            let deadline = if pipeline.result.notification_count == 0 {
                first_rx_deadline.min(overall_deadline)
            } else {
                overall_deadline
            };
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(notification)) => {
                    if notification.uuid != CHARACTERISTIC_UUID {
                        logger.line(format!(
                            "timestamp_ms={} backend=btleplug {} RX characteristic={} raw={} decode=SKIPPED_NON_TARGET evidence=VERIFIED_FROM_DEVICE",
                            unix_timestamp_ms(),
                            probe_elapsed(connected_at),
                            notification.uuid,
                            diagnostic_hex(&notification.value),
                        ));
                        continue;
                    }
                    let writes = pipeline.accept(
                        &notification.value,
                        notification.uuid,
                        connected_at,
                        logger,
                    );
                    for frame in writes {
                        let stamp = unix_timestamp_ms();
                        logger.line(format!(
                            "timestamp_ms={stamp} backend=btleplug {} TX raw={} write_type=WithResponse codec=PRODUCTION evidence=VERIFIED_FROM_APK",
                            probe_elapsed(connected_at),
                            diagnostic_hex(&frame),
                        ));
                        match session.write(&frame, WriteType::WithResponse).await {
                            Ok(()) => logger.line(format!(
                                "timestamp_ms={stamp} backend=btleplug {} write=SUCCESS write_type=WithResponse evidence=VERIFIED_FROM_DEVICE",
                                probe_elapsed(connected_at),
                            )),
                            Err(error) => logger.line(format!(
                                "timestamp_ms={stamp} backend=btleplug {} write=FAIL write_type=WithResponse error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
                                probe_elapsed(connected_at),
                                error,
                            )),
                        }
                    }
                }
                Ok(None) => {
                    logger.line(format!(
                        "backend=btleplug {} notifications_stream=ENDED subscribe_calls=0 evidence=VERIFIED_FROM_DEVICE",
                        probe_elapsed(connected_at),
                    ));
                    break;
                }
                Err(_) => {
                    if pipeline.result.notification_count == 0 {
                        logger.line(format!(
                            "backend=btleplug {} unsolicited_notifications=NONE observation_ms={} subscribe_calls=0 interpretation=BTLEPLUG_WINDOWS_PUBLIC_STREAM_DID_NOT_DELIVER_WITHOUT_SUBSCRIBE evidence=VERIFIED_FROM_DEVICE",
                            probe_elapsed(connected_at),
                            initial_observation.as_millis(),
                        ));
                    } else {
                        logger.line(format!(
                            "backend=btleplug {} passive_handshake=TIMEOUT total_timeout_ms={} notifications={} evidence=VERIFIED_FROM_DEVICE",
                            probe_elapsed(connected_at),
                            handshake_timeout.as_millis(),
                            pipeline.result.notification_count,
                        ));
                    }
                    break;
                }
            }
        }
    }

    match session.disconnect().await {
        Ok(()) => logger.line(format!(
            "backend=btleplug {} disconnect=SUCCESS evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
        )),
        Err(error) => logger.line(format!(
            "backend=btleplug {} disconnect=FAIL error={} debug={error:?} evidence=VERIFIED_FROM_DEVICE",
            probe_elapsed(connected_at),
            error,
        )),
    }
    pipeline.result()
}

async fn passive_handshake(
    device_name: String,
    initial_observation: Duration,
    handshake_timeout: Duration,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut logger = DiagnosticLogger::new(output)?;
    logger.line("FLOW 8 passive handshake diagnostic");
    logger.line("policy=NO_SUBSCRIBE NO_CCCD_WRITE NO_MIXER_CONTROL; permitted FLOW TX is production-codec 0x39 and 0x37 only");
    logger.line("ANDROID_HCI evidence=VERIFIED_FROM_DEVICE value_handle=0x000B declaration_handle=0x000A next_declaration_handle=0x000C cccd_present=false");
    logger.line("ANDROID_HCI evidence=VERIFIED_FROM_DEVICE sequence=RX_0x35->TX_0x39->RX_0x36->TX_0x37->RX_0x38 notifications_without_cccd=true mtu=131 mixer_state_fragments=4");

    let btleplug =
        run_btleplug_passive_handshake(initial_observation, handshake_timeout, &mut logger).await;
    let fallback_required = btleplug.notification_count == 0;
    if fallback_required {
        logger.line("btleplug_fallback=REQUIRED reason=NO_UNSOLICITED_NOTIFICATION_WITHOUT_SUBSCRIBE; opening a fresh Windows WinRT connection and attaching ValueChanged without CCCD");
        #[cfg(not(target_os = "windows"))]
        logger.line(format!(
            "winrt_value_changed_fallback=UNAVAILABLE platform={} required_platform=windows device_name={device_name:?}; no notification was fabricated",
            std::env::consts::OS,
        ));
    } else {
        logger.line("btleplug_fallback=NOT_NEEDED reason=UNSOLICITED_NOTIFICATION_DELIVERED");
    }
    #[cfg(target_os = "windows")]
    let winrt_result = if fallback_required {
        logger.line("winrt_fallback fresh_connection_delay_ms=250 after_btleplug_disconnect");
        tokio::time::sleep(Duration::from_millis(250)).await;
        Some(winrt_passive_handshake::run(&device_name, handshake_timeout, &mut logger).await?)
    } else {
        None
    };
    #[cfg(not(target_os = "windows"))]
    let winrt_result: Option<PassiveAttemptResult> = None;
    let results = std::iter::once(btleplug)
        .chain(winrt_result)
        .collect::<Vec<_>>();

    logger.line("");
    logger.line("PASSIVE HANDSHAKE SUMMARY");
    logger.line(format!(
        "{:<12} {:<14} {:<8} {:<8} {:<14} {:<8}",
        "Backend", "Notifications", "0x35", "0x36", "0x38 applied", "Ready"
    ));
    for result in results {
        logger.line(format!(
            "{:<12} {:<14} {:<8} {:<8} {:<14} {:<8}",
            result.backend,
            result.notification_count,
            probe_status(result.handshake_host_received),
            probe_status(result.handshake_reply_received),
            probe_status(result.mixer_state_applied),
            probe_status(result.ready),
        ));
    }
    logger.line("All real backend observations, including absence/timeouts, are retained as VERIFIED_FROM_DEVICE for this run. APK codec generation remains VERIFIED_FROM_APK until accepted by the device.");
    Ok(())
}

async fn subscribed_session() -> Result<Flow8BleSession, Box<dyn std::error::Error>> {
    let transport = BleTransport::new().await?;
    let session = transport.connect_flow8().await?;
    println!(
        "GATT service={SERVICE_UUID} characteristic={CHARACTERISTIC_UUID} mtu={} properties={:?}",
        session.mtu(),
        session.characteristic().properties
    );
    session.subscribe().await?;
    println!("CCCD/subscribe completed");
    Ok(session)
}

async fn send_one(command: TxCommand) -> Result<(), Box<dyn std::error::Error>> {
    let session = subscribed_session().await?;
    let mode = write_type(&session);
    let mut stream = session.notifications().await?;
    let mut coordinator = Flow8Session::new(*b"FLOW8-PC-RUST001");
    coordinator.rx_armed();
    let mut store = Flow8Store::disconnected();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while coordinator.phase() != SessionPhase::Ready {
        let notification = tokio::time::timeout_at(deadline, stream.next())
            .await
            .map_err(|_| "timed out before complete 0x38/Ready")?
            .ok_or("notification stream ended before Ready")?;
        println!("RX {}", hex(&notification.value));
        for action in coordinator.notification(&notification.value) {
            match action {
                SessionAction::Send(frame) => {
                    println!("TX handshake/state {}", hex(&frame));
                    session.write(&frame, mode).await?;
                }
                SessionAction::Received(command) => {
                    println!("decoded={command:?}");
                    let is_mixer_state = matches!(&command, RxCommand::MixerState(_));
                    store.apply_rx(command, EvidenceStatus::VerifiedFromDevice)?;
                    if is_mixer_state {
                        println!("session={:?}", coordinator.state_applied()?);
                    }
                }
                SessionAction::Phase(phase) => println!("session={phase:?}"),
                SessionAction::Warning(warning) => eprintln!("decode/session warning: {warning}"),
                SessionAction::Error(error) => return Err(error.into()),
            }
        }
    }
    let frame = encode(&command)?;
    println!("TX control mode={mode:?} bytes={}", hex(&frame));
    session.write(&frame, mode).await?;
    let response_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while let Ok(Some(notification)) =
        tokio::time::timeout_at(response_deadline, stream.next()).await
    {
        println!("RX {}", hex(&notification.value));
        for action in coordinator.notification(&notification.value) {
            if let SessionAction::Received(command) = action {
                println!("decoded={command:?}");
            }
        }
    }
    println!("control write submitted; acceptance still requires observed device behavior");
    session.disconnect().await?;
    Ok(())
}

async fn observe(
    seconds: u64,
    handshake: bool,
    apply_state: bool,
    mut capture: Option<&mut File>,
) -> Result<(), Box<dyn std::error::Error>> {
    let session = subscribed_session().await?;
    let mode = write_type(&session);
    println!("write_type={mode:?}");
    if let Some(file) = capture.as_deref_mut() {
        writeln!(
            file,
            "# service={SERVICE_UUID} characteristic={CHARACTERISTIC_UUID} mtu={} properties={:?} write_type={mode:?}",
            session.mtu(),
            session.characteristic().properties,
        )?;
    }
    let mut stream = session.notifications().await?;
    let mut coordinator = Flow8Session::new(*b"FLOW8-PC-RUST001");
    coordinator.rx_armed();
    let mut decoder = CommandStreamDecoder::default();
    let mut store = Flow8Store::disconnected();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let notification = tokio::time::timeout_at(deadline, stream.next()).await;
        let Ok(Some(notification)) = notification else {
            break;
        };
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        println!("{stamp} RX {}", hex(&notification.value));
        if let Some(file) = capture.as_deref_mut() {
            writeln!(file, "{stamp} RX {}", hex(&notification.value))?;
        }
        if handshake {
            for action in coordinator.notification(&notification.value) {
                match action {
                    SessionAction::Send(frame) => {
                        println!("{stamp} TX {}", hex(&frame));
                        if let Some(file) = capture.as_deref_mut() {
                            writeln!(file, "{stamp} TX {}", hex(&frame))?;
                        }
                        session.write(&frame, mode).await?;
                    }
                    SessionAction::Received(command) => {
                        println!("decoded={command:?}");
                        let is_mixer_state = matches!(&command, RxCommand::MixerState(_));
                        if let Some(file) = capture.as_deref_mut() {
                            writeln!(
                                file,
                                "{stamp} DECODE {command:?} evidence=VERIFIED_FROM_DEVICE"
                            )?;
                        }
                        if apply_state {
                            store.apply_rx(command, EvidenceStatus::VerifiedFromDevice)?;
                            if let Some(file) = capture.as_deref_mut() {
                                writeln!(file, "{stamp} STATE_APPLIED confirmed=device")?;
                            }
                            if is_mixer_state {
                                let ready = coordinator.state_applied()?;
                                println!("session={ready:?}");
                                if let Some(file) = capture.as_deref_mut() {
                                    writeln!(file, "{stamp} SESSION {ready:?}")?;
                                }
                            }
                        }
                    }
                    SessionAction::Phase(phase) => {
                        println!("session={phase:?}");
                        if let Some(file) = capture.as_deref_mut() {
                            writeln!(file, "{stamp} SESSION {phase:?}")?;
                        }
                    }
                    SessionAction::Warning(warning) => {
                        eprintln!("decode/session warning: {warning}");
                        if let Some(file) = capture.as_deref_mut() {
                            writeln!(file, "{stamp} WARNING {warning} raw_retained=true")?;
                        }
                    }
                    SessionAction::Error(error) => eprintln!("decode/session error: {error}"),
                }
            }
        } else {
            match decoder.accept(&notification.value) {
                Ok(Some(command)) => {
                    println!("decoded={command:?}");
                    if let Some(file) = capture.as_deref_mut() {
                        writeln!(
                            file,
                            "{stamp} DECODE {command:?} evidence=VERIFIED_FROM_DEVICE"
                        )?;
                    }
                }
                Ok(None) => println!("fragment buffered"),
                Err(error) => eprintln!("decode error: {error}"),
            }
        }
    }
    if apply_state {
        println!(
            "state session={:?} CH1 gain={:?} MAIN master={:?} tempo={:?}",
            store.state.session,
            store.state.channels[0].gain_db.confirmed,
            store.state.buses[0].master_level.confirmed,
            store.state.global_tempo_bpm.confirmed
        );
    }
    session.disconnect().await?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    match Cli::parse().command {
        Command::Scan { seconds } => {
            let transport = BleTransport::new().await?;
            for device in transport.scan(Duration::from_secs(seconds)).await? {
                println!(
                    "name={:?} id={} address={} rssi={:?} services={:?}",
                    device.name, device.id, device.address, device.rssi, device.services
                );
            }
        }
        Command::Inspect | Command::Connect => {
            let transport = BleTransport::new().await?;
            let session = transport.connect_flow8().await?;
            println!(
                "connected service={SERVICE_UUID} characteristic={CHARACTERISTIC_UUID} mtu={} properties={:?} selected_write_type={:?}",
                session.mtu(),
                session.characteristic().properties,
                write_type(&session),
            );
            println!("No protocol packet was sent.");
            session.disconnect().await?;
        }
        Command::InspectGatt {
            descriptor_timeout_ms,
            output,
        } => {
            inspect_gatt(Duration::from_millis(descriptor_timeout_ms), output).await?;
        }
        Command::WinrtGattProbe {
            device_name,
            observe_seconds,
            output,
        } => {
            let mut logger = DiagnosticLogger::new(output)?;
            #[cfg(target_os = "windows")]
            winrt_gatt_probe::run(
                &device_name,
                Duration::from_secs(observe_seconds),
                &mut logger,
            )
            .await?;
            #[cfg(not(target_os = "windows"))]
            logger.line(format!(
                "winrt-gatt-probe=UNAVAILABLE platform={} required_platform=windows device_name={device_name:?} observe_seconds={observe_seconds}; no BLE operation was attempted",
                std::env::consts::OS,
            ));
        }
        Command::Subscribe { seconds } => observe(seconds, false, false, None).await?,
        Command::Handshake { seconds } | Command::RequestState { seconds } => {
            observe(seconds, true, false, None).await?
        }
        Command::Show { seconds } => observe(seconds, true, true, None).await?,
        Command::Route {
            source,
            destination: target,
            normalized,
        } => {
            send_one(TxCommand::RouteLevel {
                source: input(source)?,
                destination: destination(&target)?,
                normalized,
            })
            .await?;
        }
        Command::Gain { input: id, db } => {
            let input = input(id)?;
            if !input.has_analog_gain() {
                return Err("BT/USB has no normal Gain capability".into());
            }
            send_one(TxCommand::Gain { input, db }).await?;
        }
        Command::Mute { endpoint, enabled } => {
            send_one(TxCommand::Mute { endpoint, enabled }).await?;
        }
        Command::Pan { input: id, value } => {
            send_one(TxCommand::Pan {
                input: input(id)?,
                value,
            })
            .await?;
        }
        Command::Solo { input: id, enabled } => {
            send_one(TxCommand::Solo {
                input: input(id)?,
                enabled,
            })
            .await?;
        }
        Command::Capture {
            path,
            seconds,
            handshake,
        } => {
            let mut file = File::create(&path)?;
            writeln!(
                file,
                "# FLOW 8 hardware capture; raw bytes retained; APK evidence is not device evidence"
            )?;
            observe(seconds, handshake, handshake, Some(&mut file)).await?;
            println!("capture saved to {}", path.display());
        }
        Command::ProbeHandshakeOrder {
            delay_ms,
            observe_seconds,
            output,
        } => {
            probe_handshake_order(
                Duration::from_millis(delay_ms),
                Duration::from_secs(observe_seconds),
                output,
            )
            .await?;
        }
        Command::PassiveHandshake {
            device_name,
            btleplug_observe_seconds,
            timeout_seconds,
            output,
        } => {
            passive_handshake(
                device_name,
                Duration::from_secs(btleplug_observe_seconds),
                Duration::from_secs(timeout_seconds),
                output,
            )
            .await?;
        }
        Command::PrearmedHandshake {
            device_name,
            timeout_seconds,
            output,
        } => {
            let mut logger = DiagnosticLogger::new(output)?;
            #[cfg(target_os = "windows")]
            winrt_prearmed_handshake::run(
                &device_name,
                Duration::from_secs(timeout_seconds),
                &mut logger,
            )
            .await?;
            #[cfg(not(target_os = "windows"))]
            logger.line(format!(
                "prearmed-handshake=UNAVAILABLE platform={} required_platform=windows device_name={device_name:?} timeout_seconds={timeout_seconds}; no BLE operation was attempted",
                std::env::consts::OS,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gatt_inspection_formats_all_reported_characteristic_properties() {
        assert_eq!(
            characteristic_property_names(CharPropFlags::WRITE | CharPropFlags::NOTIFY),
            "WRITE | NOTIFY"
        );
        assert_eq!(
            uuid_from_u16(0x2902).to_string(),
            "00002902-0000-1000-8000-00805f9b34fb"
        );
    }

    #[test]
    fn handshake_order_plan_uses_only_exact_companion_frames() {
        let cases = handshake_probe_cases(Duration::from_millis(250));
        assert_eq!(cases.len(), 5);
        assert!(cases[0].steps.is_empty());

        let mut writes = Vec::new();
        let mut delays = 0;
        for case in &cases {
            for step in &case.steps {
                match step {
                    ProbeStep::Write { command, .. } => {
                        assert!(matches!(
                            command,
                            TxCommand::HandshakeHostProbe { .. } | TxCommand::HandshakeReplyProbe
                        ));
                        writes.push(encode(command).unwrap());
                    }
                    ProbeStep::Delay(duration) => {
                        delays += 1;
                        assert_eq!(*duration, Duration::from_millis(250));
                    }
                }
            }
        }

        assert_eq!(delays, 1);
        assert_eq!(writes.len(), 5);
        assert_eq!(
            writes[0],
            vec![
                0x35, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x36,
            ]
        );
        assert_eq!(writes[1], vec![0x36, 0x01, 0x37]);
        assert_eq!(writes[2], writes[0]);
        assert_eq!(writes[3], writes[0]);
        assert_eq!(writes[4], writes[1]);
    }

    #[test]
    fn passive_pipeline_uses_production_handshake_codecs_without_subscribe_state() {
        let connected_at = Instant::now();
        let mut logger = DiagnosticLogger::new(None).unwrap();
        let mut pipeline = PassiveHandshakePipeline::new("test");
        let mut host_payload = vec![0x11; 16];
        host_payload.extend([0, 0, 1, 0, 2]);
        let host = flow8_protocol::frame_single(0x35, &host_payload);
        let writes = pipeline.accept(&host, CHARACTERISTIC_UUID, connected_at, &mut logger);
        assert_eq!(writes.len(), 1);
        assert_eq!(
            writes[0],
            encode(&TxCommand::HandshakeClient {
                client_id: *b"FLOW8-PC-RUST001",
            })
            .unwrap()
        );
        assert!(pipeline.result().handshake_host_received);

        let reply = flow8_protocol::frame_single(0x36, &[]);
        let writes = pipeline.accept(&reply, CHARACTERISTIC_UUID, connected_at, &mut logger);
        assert_eq!(writes, vec![encode(&TxCommand::GetMixerState).unwrap()]);
        assert!(pipeline.result().handshake_reply_received);
        assert!(!pipeline.result().ready);
    }

    #[test]
    fn prearmed_handshake_cli_defaults_to_safe_windows_probe_settings() {
        let cli = Cli::try_parse_from(["flow8-hardware-bringup", "prearmed-handshake"])
            .expect("prearmed-handshake must remain available to the Windows bring-up flow");
        match cli.command {
            Command::PrearmedHandshake {
                device_name,
                timeout_seconds,
                output,
            } => {
                assert_eq!(device_name, "FLOW 8 LE");
                assert_eq!(timeout_seconds, 30);
                assert!(output.is_none());
            }
            _ => panic!("unexpected command parsed"),
        }
    }
}
