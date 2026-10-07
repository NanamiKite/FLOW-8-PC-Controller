//! Store, semantic commands, queueing, protocol state application and simulator.

use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use flow8_model::{
    DeviceSettingsState, EqBandState, EqState, EvidenceStatus, FxId, FxState,
    HeadphoneRoutingState, HeadphoneSource, InputChannelState, InputId, LimiterState, MeterState,
    MixBusId, MixBusState, MixDestination, MonitorLinkState, MonitorRoutingSource, RoutingState,
    SnapshotSlotState, SnapshotState, StateValue, TapPoint, UsbAudioEndpointId,
    UsbAudioEndpointState, specs,
};
use flow8_protocol::{RxCommand, TxCommand};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuteTarget {
    Input(InputId),
    Bus(MixBusId),
    Fx(FxId),
}

impl MuteTarget {
    pub const fn endpoint(self) -> u8 {
        match self {
            Self::Input(input) => input.endpoint(),
            Self::Bus(MixBusId::Main) => 15,
            Self::Bus(MixBusId::Monitor1) => 10,
            Self::Bus(MixBusId::Monitor2) => 11,
            Self::Fx(FxId::Fx1) => 12,
            Self::Fx(FxId::Fx2) => 13,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownSetting {
    BtUsbSwitch(bool),
    PhonesOnly(bool),
    FootswitchFxMode(bool),
    DeviceName(String),
    UsbStreaming(bool),
    MonitorRouting(MonitorRoutingSource),
    Input56FromUsb12(bool),
    Input78FromUsb34(bool),
    MonitorStereoLink(bool),
    HeadphonesUseMonitor(bool),
    HeadphonesPostFader(bool),
    MainMinus10Dbv(bool),
    MonitorMinus10Dbv(bool),
    SnapshotScope(u8),
    MonitorPostFader(bool),
    DeviceLinkedSelection(bool),
}

impl KnownSetting {
    fn wire(&self) -> (u8, Vec<u8>) {
        let boolean = |value: bool| vec![value as u8];
        match self {
            Self::BtUsbSwitch(value) => (0x01, boolean(*value)),
            Self::PhonesOnly(value) => (0x02, boolean(*value)),
            Self::FootswitchFxMode(value) => (0x03, boolean(*value)),
            Self::DeviceName(value) => (0x05, value.as_bytes().to_vec()),
            Self::UsbStreaming(value) => (0x07, boolean(*value)),
            Self::MonitorRouting(source) => (
                0x08,
                vec![match source {
                    MonitorRoutingSource::MonitorMix => 0,
                    MonitorRoutingSource::Usb12 => 1,
                    MonitorRoutingSource::Usb34 => 2,
                    MonitorRoutingSource::Unknown(raw) => *raw,
                }],
            ),
            Self::Input56FromUsb12(value) => (0x09, boolean(*value)),
            Self::Input78FromUsb34(value) => (0x0a, boolean(*value)),
            Self::MonitorStereoLink(value) => (0x0b, boolean(*value)),
            Self::HeadphonesUseMonitor(value) => (0x0c, boolean(*value)),
            Self::HeadphonesPostFader(value) => (0x0d, boolean(*value)),
            Self::MainMinus10Dbv(value) => (0x0e, boolean(*value)),
            Self::MonitorMinus10Dbv(value) => (0x0f, boolean(*value)),
            Self::SnapshotScope(value) => (0x10, vec![*value]),
            Self::MonitorPostFader(value) => (0x11, boolean(*value)),
            Self::DeviceLinkedSelection(value) => (0xb0, boolean(*value)),
        }
    }
}

/// Endpoint class for a read-only 0x16 channel-state request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStateTarget {
    Input(InputId),
    Destination(MixDestination),
}

impl ChannelStateTarget {
    pub const fn endpoint(self) -> u8 {
        match self {
            Self::Input(input) => input.endpoint(),
            Self::Destination(destination) => destination.endpoint(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticCommand {
    RequestChannelState {
        target: ChannelStateTarget,
    },
    SetGain {
        input: InputId,
        db: f32,
    },
    SetRouteLevel {
        source: InputId,
        destination: MixDestination,
        normalized: f32,
    },
    SetDestinationMaster {
        destination: MixDestination,
        normalized: f32,
    },
    SetPan {
        input: InputId,
        value: f32,
    },
    SetBusBalance {
        bus: MixBusId,
        value: f32,
    },
    SetFxPan {
        fx: FxId,
        value: f32,
    },
    SetMute {
        input: InputId,
        enabled: bool,
    },
    SetSolo {
        input: InputId,
        enabled: bool,
    },
    SetPhase {
        input: InputId,
        inverted: bool,
    },
    SetPhantom {
        input: InputId,
        enabled: bool,
    },
    SetHighPass {
        input: InputId,
        enabled: bool,
        frequency_hz: u16,
    },
    SetPeqBand {
        input: InputId,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
    },
    SetGeqBand {
        bus: MixBusId,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
    },
    SetCompressorAmount {
        input: InputId,
        amount: f32,
    },
    SetLimiter {
        bus: MixBusId,
        threshold_db: f32,
    },
    SetLabel {
        input: InputId,
        icon: u16,
        name: String,
    },
    SetMuteTarget {
        target: MuteTarget,
        enabled: bool,
    },
    SetFxSetup {
        fx: FxId,
        values: [u8; 3],
        return_to_main: bool,
        return_to_mon1: bool,
        return_to_mon2: bool,
    },
    SetFxPreset {
        fx: FxId,
        preset: u8,
    },
    SetTempo {
        bpm: u16,
    },
    SetDelay {
        bus: MixBusId,
        ticks: u32,
    },
    SetSetting(KnownSetting),
    SaveSnapshot {
        slot: u8,
        name: String,
    },
    LoadSnapshot {
        slot: u8,
    },
    DeleteSnapshot {
        slot: u8,
    },
    RenameSnapshot {
        slot: u8,
        name: String,
    },
    SelectDeviceOutput {
        destination: MixDestination,
    },
    RequestMeters {
        destination: MixDestination,
    },
    RequestChannelLabels,
    RequestFullState,
    RequestSnapshotNames,
    FactoryReset,
}

impl SemanticCommand {
    pub fn to_protocol(&self) -> TxCommand {
        match self {
            Self::RequestChannelState { target } => TxCommand::GetChannelState {
                endpoint: target.endpoint(),
            },
            Self::SetGain { input, db } => TxCommand::Gain {
                input: *input,
                db: *db,
            },
            Self::SetRouteLevel {
                source,
                destination,
                normalized,
            } => TxCommand::RouteLevel {
                source: *source,
                destination: *destination,
                normalized: *normalized,
            },
            Self::SetDestinationMaster {
                destination,
                normalized,
            } => TxCommand::DestinationMaster {
                destination: *destination,
                normalized: *normalized,
            },
            Self::SetPan { input, value } => TxCommand::Pan {
                input: *input,
                value: *value,
            },
            Self::SetBusBalance { bus, value } => TxCommand::EndpointPan {
                endpoint: bus_endpoint(*bus),
                value: *value,
            },
            Self::SetFxPan { fx, value } => TxCommand::EndpointPan {
                endpoint: fx_endpoint(*fx),
                value: *value,
            },
            Self::SetMute { input, enabled } => TxCommand::Mute {
                endpoint: input.endpoint(),
                enabled: *enabled,
            },
            Self::SetSolo { input, enabled } => TxCommand::Solo {
                input: *input,
                enabled: *enabled,
            },
            Self::SetPhase { input, inverted } => TxCommand::Phase {
                input: *input,
                inverted: *inverted,
            },
            Self::SetPhantom { input, enabled } => TxCommand::Phantom {
                input: *input,
                enabled: *enabled,
            },
            Self::SetHighPass {
                input,
                enabled,
                frequency_hz,
            } => TxCommand::HighPassFilter {
                input: *input,
                enabled: *enabled,
                frequency_hz: *frequency_hz,
            },
            Self::SetPeqBand {
                input,
                band,
                frequency_hz,
                q,
                gain_db,
            } => TxCommand::ParametricEq {
                input: *input,
                band: *band,
                frequency_hz: *frequency_hz,
                q: *q,
                gain_db: *gain_db,
            },
            Self::SetGeqBand {
                bus,
                band,
                frequency_hz,
                q,
                gain_db,
            } => TxCommand::GraphicEq {
                endpoint: bus_endpoint(*bus),
                band: *band,
                frequency_hz: *frequency_hz,
                q: *q,
                gain_db: *gain_db,
            },
            Self::SetCompressorAmount { input, amount } => TxCommand::Compressor {
                input: *input,
                amount: *amount,
            },
            Self::SetLimiter { bus, threshold_db } => TxCommand::Limiter {
                endpoint: bus_endpoint(*bus),
                threshold_db: *threshold_db,
            },
            Self::SetLabel { input, icon, name } => {
                TxCommand::Label(flow8_protocol::ChannelLabel {
                    endpoint: input.endpoint(),
                    icon: *icon,
                    text: name.clone(),
                })
            }
            Self::SetMuteTarget { target, enabled } => TxCommand::Mute {
                endpoint: target.endpoint(),
                enabled: *enabled,
            },
            Self::SetFxSetup {
                fx,
                values,
                return_to_main,
                return_to_mon1,
                return_to_mon2,
            } => TxCommand::FxSetup {
                endpoint: fx_endpoint(*fx),
                values: *values,
                route_flags: (*return_to_main as u8)
                    | ((*return_to_mon1 as u8) << 1)
                    | ((*return_to_mon2 as u8) << 2),
            },
            Self::SetFxPreset { fx, preset } => TxCommand::FxPreset {
                endpoint: fx_endpoint(*fx),
                preset: *preset,
            },
            Self::SetTempo { bpm } => TxCommand::FxTempo { bpm: *bpm },
            Self::SetDelay { bus, ticks } => TxCommand::ChannelDelay {
                endpoint: bus_endpoint(*bus),
                ticks: *ticks,
            },
            Self::SetSetting(setting) => {
                let (id, data) = setting.wire();
                TxCommand::Setting { id, data }
            }
            Self::SaveSnapshot { slot, name } => TxCommand::SnapshotSave {
                slot: *slot,
                name: name.clone(),
            },
            Self::LoadSnapshot { slot } => TxCommand::SnapshotLoad { slot: *slot },
            Self::DeleteSnapshot { slot } => TxCommand::SnapshotDelete { slot: *slot },
            Self::RenameSnapshot { slot, name } => TxCommand::SnapshotRename {
                slot: *slot,
                name: name.clone(),
            },
            Self::SelectDeviceOutput { destination } => TxCommand::SelectOutput {
                endpoint: destination.endpoint(),
            },
            Self::RequestMeters { destination } => {
                let mut channel_codes = [0; 15];
                channel_codes[..7].copy_from_slice(&[0x40, 0x41, 0x42, 0x43, 0xc4, 0xc5, 0xc6]);
                // The APK derives this flag from output-channel metadata. Our
                // current MAIN/FX stereo model still needs hardware calibration.
                let stereo = matches!(
                    destination,
                    MixDestination::Main | MixDestination::Fx1 | MixDestination::Fx2
                );
                channel_codes[7] = destination.endpoint() | 0x40 | if stereo { 0x80 } else { 0 };
                TxCommand::MeterRequest(flow8_protocol::MeterRequest {
                    count: 8,
                    channel_codes,
                })
            }
            Self::RequestChannelLabels => TxCommand::GetChannelLabels,
            Self::RequestFullState => TxCommand::GetMixerState,
            Self::RequestSnapshotNames => TxCommand::GetSnapshotNames,
            Self::FactoryReset => TxCommand::FactoryReset,
        }
    }
}

const fn bus_endpoint(bus: MixBusId) -> u8 {
    match bus {
        MixBusId::Main => 15,
        MixBusId::Monitor1 => 10,
        MixBusId::Monitor2 => 11,
    }
}

const fn fx_endpoint(fx: FxId) -> u8 {
    match fx {
        FxId::Fx1 => 12,
        FxId::Fx2 => 13,
    }
}

const fn destination_for_bus(bus: MixBusId) -> MixDestination {
    match bus {
        MixBusId::Main => MixDestination::Main,
        MixBusId::Monitor1 => MixDestination::Monitor1,
        MixBusId::Monitor2 => MixDestination::Monitor2,
    }
}

const fn fx_index(fx: FxId) -> usize {
    match fx {
        FxId::Fx1 => 0,
        FxId::Fx2 => 1,
    }
}

const fn monitor_routing(value: u8) -> MonitorRoutingSource {
    match value {
        0 => MonitorRoutingSource::MonitorMix,
        1 => MonitorRoutingSource::Usb12,
        2 => MonitorRoutingSource::Usb34,
        other => MonitorRoutingSource::Unknown(other),
    }
}

const fn input_from_endpoint(endpoint: u8) -> Option<InputId> {
    match endpoint {
        0 => Some(InputId::Input1),
        1 => Some(InputId::Input2),
        2 => Some(InputId::Input3),
        3 => Some(InputId::Input4),
        4 => Some(InputId::Input56),
        5 => Some(InputId::Input78),
        6 => Some(InputId::UsbBluetooth),
        _ => None,
    }
}

const fn destination_from_endpoint(endpoint: u8) -> Option<MixDestination> {
    match endpoint {
        10 => Some(MixDestination::Monitor1),
        11 => Some(MixDestination::Monitor2),
        12 => Some(MixDestination::Fx1),
        13 => Some(MixDestination::Fx2),
        15 => Some(MixDestination::Main),
        _ => None,
    }
}

const fn fx_index_from_endpoint(endpoint: u8) -> Option<usize> {
    match endpoint {
        12 => Some(0),
        13 => Some(1),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoalesceKey {
    Gain(InputId),
    Route(InputId, MixDestination),
    Master(MixDestination),
    Pan(InputId),
    BusBalance(MixBusId),
    FxPan(FxId),
    HighPass(InputId),
    Peq(InputId, u8),
    Geq(MixBusId, u8),
    Compressor(InputId),
    Limiter(MixBusId),
    FxSetup(FxId),
    Tempo,
    Delay(MixBusId),
}

impl SemanticCommand {
    fn coalesce_key(&self) -> Option<CoalesceKey> {
        match self {
            Self::SetGain { input, .. } => Some(CoalesceKey::Gain(*input)),
            Self::SetRouteLevel {
                source,
                destination,
                ..
            } => Some(CoalesceKey::Route(*source, *destination)),
            Self::SetDestinationMaster { destination, .. } => {
                Some(CoalesceKey::Master(*destination))
            }
            Self::SetPan { input, .. } => Some(CoalesceKey::Pan(*input)),
            Self::SetBusBalance { bus, .. } => Some(CoalesceKey::BusBalance(*bus)),
            Self::SetFxPan { fx, .. } => Some(CoalesceKey::FxPan(*fx)),
            Self::SetHighPass { input, .. } => Some(CoalesceKey::HighPass(*input)),
            Self::SetPeqBand { input, band, .. } => Some(CoalesceKey::Peq(*input, *band)),
            Self::SetGeqBand { bus, band, .. } => Some(CoalesceKey::Geq(*bus, *band)),
            Self::SetCompressorAmount { input, .. } => Some(CoalesceKey::Compressor(*input)),
            Self::SetLimiter { bus, .. } => Some(CoalesceKey::Limiter(*bus)),
            Self::SetFxSetup { fx, .. } => Some(CoalesceKey::FxSetup(*fx)),
            Self::SetTempo { .. } => Some(CoalesceKey::Tempo),
            Self::SetDelay { bus, .. } => Some(CoalesceKey::Delay(*bus)),
            Self::SetMute { .. }
            | Self::SetSolo { .. }
            | Self::SetPhase { .. }
            | Self::SetPhantom { .. }
            | Self::SetLabel { .. }
            | Self::SetMuteTarget { .. }
            | Self::SetFxPreset { .. }
            | Self::SetSetting(_)
            | Self::SaveSnapshot { .. }
            | Self::LoadSnapshot { .. }
            | Self::DeleteSnapshot { .. }
            | Self::RenameSnapshot { .. }
            | Self::SelectDeviceOutput { .. }
            | Self::RequestMeters { .. }
            | Self::RequestChannelState { .. }
            | Self::RequestChannelLabels
            | Self::RequestFullState
            | Self::RequestSnapshotNames
            | Self::FactoryReset => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SemanticCommandQueue {
    commands: VecDeque<(u64, SemanticCommand)>,
}

const MAX_PENDING_COMMANDS: usize = 256;
const PENDING_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(15);
const FADER_CONFIRMATION_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FaderTarget {
    Route(InputId, MixDestination),
    Master(MixDestination),
}

#[derive(Debug)]
struct FaderEdit {
    command_id: u64,
    target: u8,
    // The production codec has 256 representable levels. This set is bounded
    // and contains only values whose writes have actually started.
    sent: [bool; 256],
    editing: bool,
    deadline: Instant,
}

fn fader_command(command: &SemanticCommand) -> Option<(FaderTarget, u8)> {
    let (target, normalized) = match *command {
        SemanticCommand::SetRouteLevel {
            source,
            destination,
            normalized,
        } => (FaderTarget::Route(source, destination), normalized),
        SemanticCommand::SetDestinationMaster {
            destination,
            normalized,
        } => (FaderTarget::Master(destination), normalized),
        _ => return None,
    };
    let raw = flow8_protocol::encode_fix8(
        flow8_protocol::Fix8Format::FaderDb,
        flow8_protocol::normalized_to_fader_db(normalized)?,
    )?;
    Some((target, raw))
}

trait PendingCell {
    fn track(&mut self, id: u64, now: Instant);
    fn reject(&mut self, id: Option<u64>, reason: &str);
    fn expire(&mut self, now: Instant) -> Option<u64>;
    fn deadline(&self) -> Option<Instant>;
}

impl<T> PendingCell for StateValue<T> {
    fn track(&mut self, id: u64, now: Instant) {
        self.track_pending(id, now);
    }
    fn reject(&mut self, id: Option<u64>, reason: &str) {
        self.reject_pending(id, reason);
    }
    fn expire(&mut self, now: Instant) -> Option<u64> {
        self.expire_pending(now, PENDING_CONFIRMATION_TIMEOUT)
    }
    fn deadline(&self) -> Option<Instant> {
        self.pending_deadline(PENDING_CONFIRMATION_TIMEOUT)
    }
}

fn normalize_command(mut command: SemanticCommand) -> Result<SemanticCommand, CoreError> {
    fn checked(value: f32, spec: flow8_model::ParameterSpec) -> Result<f32, CoreError> {
        if value.is_finite() && (spec.min..=spec.max).contains(&value) {
            Ok(value)
        } else {
            Err(CoreError::InvalidValue)
        }
    }
    fn quantized(value: f32, format: flow8_protocol::Fix8Format) -> Result<f32, CoreError> {
        let raw = flow8_protocol::encode_fix8(format, value).ok_or(CoreError::InvalidValue)?;
        Ok(flow8_protocol::decode_fix8(format, raw))
    }
    match &mut command {
        SemanticCommand::SetPan { value, .. }
        | SemanticCommand::SetBusBalance { value, .. }
        | SemanticCommand::SetFxPan { value, .. } => {
            checked(*value, specs::PAN)?;
        }
        SemanticCommand::SetHighPass { frequency_hz, .. } => {
            checked(*frequency_hz as f32, specs::HIGH_PASS)?;
        }
        SemanticCommand::SetLimiter { threshold_db, .. } => {
            *threshold_db = quantized(
                checked(*threshold_db, specs::LIMITER)?,
                flow8_protocol::Fix8Format::GainDb,
            )?;
        }
        SemanticCommand::SetCompressorAmount { amount, .. } => {
            checked(*amount, specs::COMPRESSOR_AMOUNT)?;
        }
        SemanticCommand::SetTempo { bpm } => {
            // Product UI guardrail, not a claim about the hardware's accepted
            // range. Keep programmatic dispatch consistent with the BPM control.
            checked(*bpm as f32, specs::TEMPO_DISPLAY)?;
        }
        SemanticCommand::SetPeqBand { q, gain_db, .. }
        | SemanticCommand::SetGeqBand { q, gain_db, .. } => {
            *q = quantized(
                checked(*q, specs::EQ_Q_WIRE)?,
                flow8_protocol::Fix8Format::Q,
            )?;
            *gain_db = quantized(
                checked(*gain_db, specs::EQ_GAIN)?,
                flow8_protocol::Fix8Format::EqGainDb,
            )?;
        }
        SemanticCommand::SaveSnapshot { name, .. }
        | SemanticCommand::RenameSnapshot { name, .. }
            if name.len() > 20 =>
        {
            return Err(CoreError::InvalidValue);
        }
        SemanticCommand::SetSetting(KnownSetting::MonitorRouting(
            MonitorRoutingSource::Unknown(_),
        )) => return Err(CoreError::InvalidValue),
        _ => {}
    }
    Ok(command)
}

fn validate_received_value(value: f32, spec: flow8_model::ParameterSpec) -> Result<(), CoreError> {
    if value.is_finite() && (spec.min..=spec.max).contains(&value) {
        Ok(())
    } else {
        Err(CoreError::InvalidCompositeState)
    }
}

fn validate_received_eq(q: f32, gain_db: f32) -> Result<(), CoreError> {
    validate_received_value(q, specs::EQ_Q_WIRE)?;
    validate_received_value(gain_db, specs::EQ_GAIN)
}

fn validate_received_level(db: f32) -> Result<(), CoreError> {
    use flow8_protocol::{Fix8Format, decode_fix8};
    if db.is_finite()
        && (decode_fix8(Fix8Format::FaderDb, 0)..=decode_fix8(Fix8Format::FaderDb, 255))
            .contains(&db)
    {
        Ok(())
    } else {
        Err(CoreError::InvalidCompositeState)
    }
}

impl SemanticCommandQueue {
    pub fn push(&mut self, command: SemanticCommand) -> Result<(), CoreError> {
        self.push_tracked(0, command)
    }

    fn can_accept(&self, command: &SemanticCommand) -> bool {
        self.commands.len() < MAX_PENDING_COMMANDS
            || command.coalesce_key().is_some_and(|key| {
                self.commands
                    .iter()
                    .rev()
                    .take_while(|(_, item)| item.coalesce_key().is_some())
                    .any(|(_, item)| item.coalesce_key() == Some(key))
            })
    }

    fn push_tracked(&mut self, command_id: u64, command: SemanticCommand) -> Result<(), CoreError> {
        if !self.can_accept(&command) {
            return Err(CoreError::QueueFull);
        }
        if let Some(key) = command.coalesce_key()
            && let Some(existing) = self
                .commands
                .iter_mut()
                .rev()
                // A discrete command observes the values before it. Never
                // move a newer continuous value across that boundary.
                .take_while(|(_, item)| item.coalesce_key().is_some())
                .find(|(_, item)| item.coalesce_key() == Some(key))
        {
            *existing = (command_id, command);
            return Ok(());
        }
        self.commands.push_back((command_id, command));
        Ok(())
    }
    pub fn pop(&mut self) -> Option<SemanticCommand> {
        self.pop_tracked().map(|(_, command)| command)
    }
    pub fn pop_tracked(&mut self) -> Option<(u64, SemanticCommand)> {
        self.commands.pop_front()
    }
    /// Core owns semantic-to-protocol conversion. Keep the queued intent until
    /// the transport accepts it, so backpressure does not drop or reorder it.
    pub fn front_protocol_tracked(&self) -> Option<(u64, TxCommand)> {
        self.commands
            .front()
            .map(|(id, command)| (*id, command.to_protocol()))
    }
    /// Restores an unsent command at the head after runtime backpressure.
    pub fn push_front(&mut self, command: SemanticCommand) -> Result<(), CoreError> {
        self.push_front_tracked(0, command)
    }
    pub fn push_front_tracked(
        &mut self,
        command_id: u64,
        command: SemanticCommand,
    ) -> Result<(), CoreError> {
        if self.commands.len() >= MAX_PENDING_COMMANDS {
            return Err(CoreError::QueueFull);
        }
        self.commands.push_front((command_id, command));
        Ok(())
    }
    pub fn clear(&mut self) {
        self.commands.clear();
    }
    pub fn len(&self) -> usize {
        self.commands.len()
    }
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

fn synthetic_meter() -> MeterState {
    MeterState {
        level_db: StateValue::confirmed(-60.0, EvidenceStatus::Synthetic),
        peak_db: StateValue::confirmed(-60.0, EvidenceStatus::Synthetic),
        gain_reduction_db: StateValue::confirmed(0.0, EvidenceStatus::Synthetic),
        clipping: StateValue::confirmed(false, EvidenceStatus::Synthetic),
    }
}

fn output_eq() -> EqState {
    const FREQUENCIES: [f32; 9] = [
        63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
    ];
    EqState {
        enabled: StateValue::confirmed(true, EvidenceStatus::Synthetic),
        bands: FREQUENCIES
            .into_iter()
            .map(|frequency| EqBandState {
                frequency_hz: StateValue::confirmed(frequency, EvidenceStatus::Synthetic),
                gain_db: StateValue::confirmed(0.0, EvidenceStatus::Synthetic),
                q: StateValue::confirmed(1.0, EvidenceStatus::Synthetic),
            })
            .collect(),
    }
}

fn bus(id: MixBusId, balance: bool) -> MixBusState {
    MixBusState {
        id,
        master_level: StateValue::confirmed(0.75, EvidenceStatus::Synthetic),
        muted: StateValue::confirmed(false, EvidenceStatus::Synthetic),
        soloed: StateValue::confirmed(false, EvidenceStatus::Synthetic),
        balance: balance.then(|| StateValue::confirmed(0.0, EvidenceStatus::Synthetic)),
        eq: output_eq(),
        limiter: LimiterState {
            threshold_db: StateValue::confirmed(-3.0, EvidenceStatus::Synthetic),
        },
        delay_ticks: StateValue::confirmed(0, EvidenceStatus::Synthetic),
        meter: synthetic_meter(),
    }
}

fn fx(id: FxId) -> FxState {
    FxState {
        id,
        preset: StateValue::confirmed(0, EvidenceStatus::Synthetic),
        parameters: std::array::from_fn(|_| StateValue::confirmed(0, EvidenceStatus::Synthetic)),
        master_level: StateValue::confirmed(0.75, EvidenceStatus::Synthetic),
        pan: StateValue::confirmed(0.0, EvidenceStatus::Synthetic),
        muted: StateValue::confirmed(false, EvidenceStatus::Synthetic),
        return_to_main: StateValue::confirmed(true, EvidenceStatus::Synthetic),
        return_to_mon1: StateValue::confirmed(false, EvidenceStatus::Synthetic),
        return_to_mon2: StateValue::confirmed(false, EvidenceStatus::Synthetic),
        meter: synthetic_meter(),
        aux_gains_db: std::array::from_fn(|_| {
            StateValue::confirmed(0.0, EvidenceStatus::Synthetic)
        }),
    }
}

#[derive(Debug, Clone)]
pub struct Flow8State {
    pub session: SessionState,
    pub channels: [InputChannelState; 7],
    pub buses: [MixBusState; 3],
    pub effects: [FxState; 2],
    pub routing: RoutingState,
    pub snapshots: SnapshotState,
    pub headphone_volume_db: StateValue<f32>,
    pub device_selected_output: StateValue<Option<MixDestination>>,
    pub selected_destination: MixDestination,
    pub selected_input: Option<InputId>,
    pub global_tempo_bpm: StateValue<f32>,
    pub mixer_flags_raw: StateValue<u16>,
}

impl Flow8State {
    pub fn synthetic() -> Self {
        let names = [
            "Input 1",
            "Input 2",
            "Input 3",
            "Input 4",
            "Input 5/6",
            "Input 7/8",
            "USB / Bluetooth",
        ];
        Self {
            session: SessionState::Ready,
            channels: std::array::from_fn(|index| {
                InputChannelState::synthetic(InputId::ALL[index], names[index])
            }),
            buses: [
                bus(MixBusId::Main, true),
                bus(MixBusId::Monitor1, false),
                bus(MixBusId::Monitor2, false),
            ],
            effects: [fx(FxId::Fx1), fx(FxId::Fx2)],
            routing: RoutingState {
                usb_audio: [
                    UsbAudioEndpointState {
                        id: UsbAudioEndpointId::Usb12,
                        active: StateValue::confirmed(true, EvidenceStatus::Synthetic),
                    },
                    UsbAudioEndpointState {
                        id: UsbAudioEndpointId::Usb34,
                        active: StateValue::confirmed(true, EvidenceStatus::Synthetic),
                    },
                ],
                headphones: HeadphoneRoutingState {
                    source: StateValue::confirmed(HeadphoneSource::Main, EvidenceStatus::Synthetic),
                    tap_point: StateValue::confirmed(
                        TapPoint::PostFader,
                        EvidenceStatus::Synthetic,
                    ),
                },
                monitor_link: MonitorLinkState {
                    stereo_linked: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    propagation_evidence: EvidenceStatus::Unknown,
                },
                settings: DeviceSettingsState {
                    bt_usb_switch: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    phones_only: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    footswitch_fx_mode: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    device_name: StateValue::confirmed("FLOW 8".into(), EvidenceStatus::Synthetic),
                    usb_streaming: StateValue::confirmed(true, EvidenceStatus::Synthetic),
                    monitor_routing: StateValue::confirmed(
                        MonitorRoutingSource::MonitorMix,
                        EvidenceStatus::Synthetic,
                    ),
                    input56_from_usb12: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    input78_from_usb34: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    main_minus_10_dbv: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    monitor_minus_10_dbv: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    snapshot_scope_bits: StateValue::confirmed(0x1f, EvidenceStatus::Synthetic),
                    monitor_post_fader: StateValue::confirmed(false, EvidenceStatus::Synthetic),
                    device_linked_selection: StateValue::confirmed(
                        false,
                        EvidenceStatus::Synthetic,
                    ),
                },
            },
            snapshots: SnapshotState {
                device_slots: (0..15)
                    .map(|slot| SnapshotSlotState {
                        slot,
                        name: StateValue::confirmed(
                            format!("Snapshot {:02}", slot + 1),
                            EvidenceStatus::Synthetic,
                        ),
                    })
                    .collect(),
                last_loaded: StateValue::confirmed(None, EvidenceStatus::Synthetic),
            },
            headphone_volume_db: StateValue::confirmed(-10.0, EvidenceStatus::Synthetic),
            device_selected_output: StateValue::confirmed(
                Some(MixDestination::Main),
                EvidenceStatus::Synthetic,
            ),
            selected_destination: MixDestination::Main,
            selected_input: Some(InputId::Input1),
            global_tempo_bpm: StateValue::confirmed(120.0, EvidenceStatus::Synthetic),
            mixer_flags_raw: StateValue::confirmed(0, EvidenceStatus::Synthetic),
        }
    }

    /// Preserve the product topology for disconnected UI navigation, but expose
    /// no synthetic device values as confirmed data in a production session.
    pub fn unconfirmed() -> Self {
        let mut state = Self::synthetic();
        state.session = SessionState::Disconnected;
        for channel in &mut state.channels {
            unknown(&mut channel.name);
            unknown(&mut channel.icon);
            unknown(&mut channel.visible);
            unknown(&mut channel.gain_db);
            unknown(&mut channel.pan);
            unknown(&mut channel.muted);
            unknown(&mut channel.soloed);
            unknown(&mut channel.phase_inverted);
            unknown(&mut channel.phantom_48v);
            unknown(&mut channel.left_connected);
            unknown(&mut channel.right_connected);
            unknown(&mut channel.high_pass_enabled);
            unknown(&mut channel.high_pass_hz);
            unknown(&mut channel.compressor.amount);
            unknown_eq(&mut channel.eq);
            for level in &mut channel.route_levels {
                unknown(level);
            }
            unknown_meter(&mut channel.meter);
        }
        for bus in &mut state.buses {
            unknown(&mut bus.master_level);
            unknown(&mut bus.muted);
            unknown(&mut bus.soloed);
            if let Some(balance) = &mut bus.balance {
                unknown(balance);
            }
            unknown_eq(&mut bus.eq);
            unknown(&mut bus.limiter.threshold_db);
            unknown(&mut bus.delay_ticks);
            unknown_meter(&mut bus.meter);
        }
        for effect in &mut state.effects {
            unknown(&mut effect.preset);
            for parameter in &mut effect.parameters {
                unknown(parameter);
            }
            unknown(&mut effect.master_level);
            unknown(&mut effect.pan);
            unknown(&mut effect.muted);
            unknown(&mut effect.return_to_main);
            unknown(&mut effect.return_to_mon1);
            unknown(&mut effect.return_to_mon2);
            unknown_meter(&mut effect.meter);
            for gain in &mut effect.aux_gains_db {
                unknown(gain);
            }
        }
        for endpoint in &mut state.routing.usb_audio {
            unknown(&mut endpoint.active);
        }
        unknown(&mut state.routing.headphones.source);
        unknown(&mut state.routing.headphones.tap_point);
        unknown(&mut state.routing.monitor_link.stereo_linked);
        let settings = &mut state.routing.settings;
        unknown(&mut settings.bt_usb_switch);
        unknown(&mut settings.phones_only);
        unknown(&mut settings.footswitch_fx_mode);
        unknown(&mut settings.device_name);
        unknown(&mut settings.usb_streaming);
        unknown(&mut settings.monitor_routing);
        unknown(&mut settings.input56_from_usb12);
        unknown(&mut settings.input78_from_usb34);
        unknown(&mut settings.main_minus_10_dbv);
        unknown(&mut settings.monitor_minus_10_dbv);
        unknown(&mut settings.snapshot_scope_bits);
        unknown(&mut settings.monitor_post_fader);
        unknown(&mut settings.device_linked_selection);
        for slot in &mut state.snapshots.device_slots {
            unknown(&mut slot.name);
        }
        unknown(&mut state.snapshots.last_loaded);
        unknown(&mut state.headphone_volume_db);
        unknown(&mut state.device_selected_output);
        unknown(&mut state.global_tempo_bpm);
        unknown(&mut state.mixer_flags_raw);
        state
    }

    pub fn bus_for_destination(&self, destination: MixDestination) -> Option<&MixBusState> {
        match destination {
            MixDestination::Main => self.buses.first(),
            MixDestination::Monitor1 => self.buses.get(1),
            MixDestination::Monitor2 => self.buses.get(2),
            _ => None,
        }
    }
    pub fn bus_for_destination_mut(
        &mut self,
        destination: MixDestination,
    ) -> Option<&mut MixBusState> {
        match destination {
            MixDestination::Main => self.buses.first_mut(),
            MixDestination::Monitor1 => self.buses.get_mut(1),
            MixDestination::Monitor2 => self.buses.get_mut(2),
            _ => None,
        }
    }
}

fn unknown<T>(value: &mut StateValue<T>) {
    *value = StateValue::unknown();
}

fn unknown_meter(meter: &mut MeterState) {
    unknown(&mut meter.level_db);
    unknown(&mut meter.peak_db);
    unknown(&mut meter.gain_reduction_db);
    unknown(&mut meter.clipping);
}

fn unknown_eq(eq: &mut EqState) {
    unknown(&mut eq.enabled);
    for band in &mut eq.bands {
        unknown(&mut band.frequency_hz);
        unknown(&mut band.gain_db);
        unknown(&mut band.q);
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("wait for a complete device state before sending commands")]
    NotReady,
    #[error("the command queue is full")]
    QueueFull,
    #[error("the selected input does not support this control")]
    UnsupportedCapability,
    #[error("the command value is invalid")]
    InvalidValue,
    #[error("the received compound state failed validation")]
    InvalidCompositeState,
}

#[derive(Debug)]
pub struct Flow8Store {
    pub state: Flow8State,
    pub queue: SemanticCommandQueue,
    meter_output_target: Option<MixDestination>,
    simulator_time: Duration,
    simulator_event_bucket: u64,
    simulator_mode: bool,
    next_command_id: u64,
    fader_edits: HashMap<FaderTarget, FaderEdit>,
    fader_commands: VecDeque<(u64, FaderTarget, u8)>,
    fader_pointer_down: bool,
}

impl Default for Flow8Store {
    fn default() -> Self {
        Self::disconnected()
    }
}

impl Flow8Store {
    pub fn simulator() -> Self {
        Self {
            state: Flow8State::synthetic(),
            queue: SemanticCommandQueue::default(),
            meter_output_target: None,
            simulator_time: Duration::ZERO,
            simulator_event_bucket: 0,
            simulator_mode: true,
            next_command_id: 1,
            fader_edits: HashMap::new(),
            fader_commands: VecDeque::new(),
            fader_pointer_down: false,
        }
    }

    pub fn disconnected() -> Self {
        let state = Flow8State::unconfirmed();
        Self {
            state,
            queue: SemanticCommandQueue::default(),
            meter_output_target: None,
            simulator_time: Duration::ZERO,
            simulator_event_bucket: 0,
            simulator_mode: false,
            next_command_id: 1,
            fader_edits: HashMap::new(),
            fader_commands: VecDeque::new(),
            fader_pointer_down: false,
        }
    }

    pub fn is_simulator(&self) -> bool {
        self.simulator_mode
    }

    /// The live session phase comes from the transport runtime, not RX command
    /// inference or optimistic GUI button presses. Offline simulation keeps
    /// its separate, explicitly synthetic lifecycle.
    pub fn apply_session_phase(&mut self, phase: SessionState) {
        if matches!(phase, SessionState::Disconnected | SessionState::Error) {
            self.clear_pending("Device session ended");
        }
        self.state.session = phase;
    }

    pub fn dispatch(&mut self, mut command: SemanticCommand) -> Result<(), CoreError> {
        let synthetic = self.simulator_mode;
        command = normalize_command(command)?;
        // Validate before creating optimistic state, including string lengths.
        flow8_protocol::encode_frames(
            &command.to_protocol(),
            flow8_protocol::MAX_RAW_PACKET_SIZE,
            0,
        )
        .map_err(|_| CoreError::InvalidValue)?;
        if !synthetic && self.state.session != SessionState::Ready {
            return Err(CoreError::NotReady);
        }
        if !self.queue.can_accept(&command) {
            return Err(CoreError::QueueFull);
        }
        match command.clone() {
            SemanticCommand::SetGain { input, db } => {
                if !input.has_analog_gain() {
                    return Err(CoreError::UnsupportedCapability);
                }
                if !db.is_finite()
                    || !(specs::INPUT_GAIN_WIRE.min..=specs::INPUT_GAIN_WIRE.max).contains(&db)
                {
                    return Err(CoreError::InvalidValue);
                }
                // Keep the pending value and queued command at the exact
                // half-decibel value represented by the production codec.
                let wire = flow8_protocol::encode_fix8(flow8_protocol::Fix8Format::GainDb, db)
                    .ok_or(CoreError::InvalidValue)?;
                let value = flow8_protocol::decode_fix8(flow8_protocol::Fix8Format::GainDb, wire);
                command = SemanticCommand::SetGain { input, db: value };
                self.state.channels[input.index()]
                    .gain_db
                    .set_pending(value);
                if synthetic {
                    self.state.channels[input.index()]
                        .gain_db
                        .observe(value, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetRouteLevel {
                source,
                destination,
                normalized,
            } => {
                if !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {
                    return Err(CoreError::InvalidValue);
                }
                let cell =
                    &mut self.state.channels[source.index()].route_levels[destination.index()];
                cell.set_pending(normalized);
                if synthetic {
                    cell.observe(normalized, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetDestinationMaster {
                destination,
                normalized,
            } => {
                if !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {
                    return Err(CoreError::InvalidValue);
                }
                if let Some(bus) = self.state.bus_for_destination_mut(destination) {
                    bus.master_level.set_pending(normalized);
                    if synthetic {
                        bus.master_level
                            .observe(normalized, EvidenceStatus::Synthetic);
                    }
                } else {
                    let fx = if destination == MixDestination::Fx1 {
                        &mut self.state.effects[0]
                    } else {
                        &mut self.state.effects[1]
                    };
                    fx.master_level.set_pending(normalized);
                    if synthetic {
                        fx.master_level
                            .observe(normalized, EvidenceStatus::Synthetic);
                    }
                }
            }
            SemanticCommand::SetPan { input, value } => {
                let raw = flow8_protocol::encode_fix8(flow8_protocol::Fix8Format::Pan, value)
                    .ok_or(CoreError::InvalidValue)?;
                let value = flow8_protocol::decode_fix8(flow8_protocol::Fix8Format::Pan, raw);
                self.state.channels[input.index()].pan.set_pending(value);
                if synthetic {
                    self.state.channels[input.index()]
                        .pan
                        .observe(value, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetBusBalance { bus, value } => {
                let raw = flow8_protocol::encode_fix8(flow8_protocol::Fix8Format::Pan, value)
                    .ok_or(CoreError::InvalidValue)?;
                let value = flow8_protocol::decode_fix8(flow8_protocol::Fix8Format::Pan, raw);
                let balance = self
                    .state
                    .bus_for_destination_mut(destination_for_bus(bus))
                    .and_then(|bus| bus.balance.as_mut())
                    .ok_or(CoreError::UnsupportedCapability)?;
                balance.set_pending(value);
                if synthetic {
                    balance.observe(value, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetFxPan { fx, value } => {
                let raw = flow8_protocol::encode_fix8(flow8_protocol::Fix8Format::Pan, value)
                    .ok_or(CoreError::InvalidValue)?;
                let value = flow8_protocol::decode_fix8(flow8_protocol::Fix8Format::Pan, raw);
                let pan = &mut self.state.effects[fx_index(fx)].pan;
                pan.set_pending(value);
                if synthetic {
                    pan.observe(value, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetMute { input, enabled } => {
                self.state.channels[input.index()]
                    .muted
                    .set_pending(enabled);
                if synthetic {
                    self.state.channels[input.index()]
                        .muted
                        .observe(enabled, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetSolo { input, enabled } => {
                self.state.channels[input.index()]
                    .soloed
                    .set_pending(enabled);
                if synthetic {
                    self.state.channels[input.index()]
                        .soloed
                        .observe(enabled, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetPhase { input, inverted } => {
                if !self.state.channels[input.index()].capabilities.phase {
                    return Err(CoreError::UnsupportedCapability);
                }
                self.state.channels[input.index()]
                    .phase_inverted
                    .set_pending(inverted);
                if synthetic {
                    self.state.channels[input.index()]
                        .phase_inverted
                        .observe(inverted, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetPhantom { input, enabled } => {
                if !input.has_phantom() {
                    return Err(CoreError::UnsupportedCapability);
                }
                self.state.channels[input.index()]
                    .phantom_48v
                    .set_pending(enabled);
                if synthetic {
                    self.state.channels[input.index()]
                        .phantom_48v
                        .observe(enabled, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetHighPass {
                input,
                enabled,
                frequency_hz,
            } => {
                if !self.state.channels[input.index()].capabilities.high_pass {
                    return Err(CoreError::UnsupportedCapability);
                }
                let frequency = frequency_hz as f32;
                let channel = &mut self.state.channels[input.index()];
                channel.high_pass_enabled.set_pending(enabled);
                channel.high_pass_hz.set_pending(frequency);
                if synthetic {
                    channel
                        .high_pass_enabled
                        .observe(enabled, EvidenceStatus::Synthetic);
                    channel
                        .high_pass_hz
                        .observe(frequency, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetPeqBand {
                input,
                band,
                frequency_hz,
                q,
                gain_db,
            } => {
                if !self.state.channels[input.index()].capabilities.peq {
                    return Err(CoreError::UnsupportedCapability);
                }
                if !q.is_finite() || !gain_db.is_finite() {
                    return Err(CoreError::InvalidValue);
                }
                let Some(target) = self.state.channels[input.index()]
                    .eq
                    .bands
                    .get_mut(band as usize)
                else {
                    return Err(CoreError::InvalidValue);
                };
                let frequency = specs::EQ_FREQUENCY_WIRE.clamp(frequency_hz as f32);
                let gain = gain_db;
                target.frequency_hz.set_pending(frequency);
                target.q.set_pending(q);
                target.gain_db.set_pending(gain);
                if synthetic {
                    target
                        .frequency_hz
                        .observe(frequency, EvidenceStatus::Synthetic);
                    target.q.observe(q, EvidenceStatus::Synthetic);
                    target.gain_db.observe(gain, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetGeqBand {
                bus,
                band,
                frequency_hz,
                q,
                gain_db,
            } => {
                if !q.is_finite() || !gain_db.is_finite() {
                    return Err(CoreError::InvalidValue);
                }
                let destination = destination_for_bus(bus);
                let Some(target) = self
                    .state
                    .bus_for_destination_mut(destination)
                    .and_then(|bus| bus.eq.bands.get_mut(band as usize))
                else {
                    return Err(CoreError::InvalidValue);
                };
                let frequency = specs::EQ_FREQUENCY_WIRE.clamp(frequency_hz as f32);
                let gain = gain_db;
                target.frequency_hz.set_pending(frequency);
                target.q.set_pending(q);
                target.gain_db.set_pending(gain);
                if synthetic {
                    target
                        .frequency_hz
                        .observe(frequency, EvidenceStatus::Synthetic);
                    target.q.observe(q, EvidenceStatus::Synthetic);
                    target.gain_db.observe(gain, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetCompressorAmount { input, amount } => {
                if !self.state.channels[input.index()].capabilities.compressor {
                    return Err(CoreError::UnsupportedCapability);
                }
                if !amount.is_finite() {
                    return Err(CoreError::InvalidValue);
                }
                let raw =
                    flow8_protocol::encode_fix8(flow8_protocol::Fix8Format::UnitInterval, amount)
                        .ok_or(CoreError::InvalidValue)?;
                let amount =
                    flow8_protocol::decode_fix8(flow8_protocol::Fix8Format::UnitInterval, raw);
                let target = &mut self.state.channels[input.index()].compressor.amount;
                target.set_pending(amount);
                if synthetic {
                    target.observe(amount, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetLimiter { bus, threshold_db } => {
                if !threshold_db.is_finite() {
                    return Err(CoreError::InvalidValue);
                }
                let threshold = threshold_db;
                let target = &mut self
                    .state
                    .bus_for_destination_mut(destination_for_bus(bus))
                    .ok_or(CoreError::InvalidValue)?
                    .limiter
                    .threshold_db;
                target.set_pending(threshold);
                if synthetic {
                    target.observe(threshold, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetLabel {
                input,
                icon,
                ref name,
            } => {
                if name.as_bytes().len() > 20 {
                    return Err(CoreError::InvalidValue);
                }
                let channel = &mut self.state.channels[input.index()];
                channel.name.set_pending(name.clone());
                channel.icon.set_pending(icon);
                if synthetic {
                    channel
                        .name
                        .observe(name.clone(), EvidenceStatus::Synthetic);
                    channel.icon.observe(icon, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetMuteTarget { target, enabled } => {
                self.set_mute_pending(target, enabled, synthetic)?;
            }
            SemanticCommand::SetFxSetup {
                fx,
                values,
                return_to_main,
                return_to_mon1,
                return_to_mon2,
            } => {
                let effect = &mut self.state.effects[fx_index(fx)];
                for (target, value) in effect.parameters.iter_mut().zip(values) {
                    target.set_pending(value);
                    if synthetic {
                        target.observe(value, EvidenceStatus::Synthetic);
                    }
                }
                for (target, value) in [
                    (&mut effect.return_to_main, return_to_main),
                    (&mut effect.return_to_mon1, return_to_mon1),
                    (&mut effect.return_to_mon2, return_to_mon2),
                ] {
                    target.set_pending(value);
                    if synthetic {
                        target.observe(value, EvidenceStatus::Synthetic);
                    }
                }
            }
            SemanticCommand::SetFxPreset { fx, preset } => {
                let target = &mut self.state.effects[fx_index(fx)].preset;
                target.set_pending(preset);
                if synthetic {
                    target.observe(preset, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetTempo { bpm } => {
                let target = &mut self.state.global_tempo_bpm;
                target.set_pending(bpm as f32);
                if synthetic {
                    target.observe(bpm as f32, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetDelay { bus, ticks } => {
                let target = &mut self
                    .state
                    .bus_for_destination_mut(destination_for_bus(bus))
                    .ok_or(CoreError::InvalidValue)?
                    .delay_ticks;
                target.set_pending(ticks);
                if synthetic {
                    target.observe(ticks, EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SetSetting(ref setting) => {
                if matches!(setting, KnownSetting::DeviceName(name) if name.len() > u8::MAX as usize)
                {
                    return Err(CoreError::InvalidValue);
                }
                self.set_setting_pending(setting, synthetic);
            }
            SemanticCommand::SaveSnapshot { slot, ref name }
            | SemanticCommand::RenameSnapshot { slot, ref name } => {
                let Some(snapshot) = self.state.snapshots.device_slots.get_mut(slot as usize)
                else {
                    return Err(CoreError::InvalidValue);
                };
                snapshot.name.set_pending(name.clone());
                if synthetic {
                    snapshot
                        .name
                        .observe(name.clone(), EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::LoadSnapshot { slot } => {
                if slot as usize >= self.state.snapshots.device_slots.len() {
                    return Err(CoreError::InvalidValue);
                }
                self.state.snapshots.last_loaded.set_pending(Some(slot));
                if synthetic {
                    self.state
                        .snapshots
                        .last_loaded
                        .observe(Some(slot), EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::DeleteSnapshot { slot } => {
                let Some(snapshot) = self.state.snapshots.device_slots.get_mut(slot as usize)
                else {
                    return Err(CoreError::InvalidValue);
                };
                snapshot.name.set_pending(String::new());
                if synthetic {
                    snapshot
                        .name
                        .observe(String::new(), EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::SelectDeviceOutput { destination } => {
                self.state
                    .device_selected_output
                    .set_pending(Some(destination));
                if synthetic {
                    self.state
                        .device_selected_output
                        .observe(Some(destination), EvidenceStatus::Synthetic);
                }
            }
            SemanticCommand::RequestMeters { destination } => {
                self.meter_output_target = Some(destination);
            }
            SemanticCommand::RequestChannelState { .. }
            | SemanticCommand::RequestChannelLabels
            | SemanticCommand::RequestFullState
            | SemanticCommand::RequestSnapshotNames => {}
            SemanticCommand::FactoryReset => {
                if synthetic {
                    let destination = self.state.selected_destination;
                    let input = self.state.selected_input;
                    self.state = Flow8State::synthetic();
                    self.state.selected_destination = destination;
                    self.state.selected_input = input;
                }
            }
        }
        let id = self.next_command_id;
        self.next_command_id = self
            .next_command_id
            .checked_add(1)
            .expect("FLOW local command IDs exhausted");
        if !synthetic {
            let now = Instant::now();
            self.visit_pending(&mut |value| value.track(id, now));
            if let Some((target, raw)) = fader_command(&command) {
                let editing = self.fader_pointer_down;
                let edit = self.fader_edits.entry(target).or_insert_with(|| FaderEdit {
                    command_id: id,
                    target: raw,
                    sent: [false; 256],
                    editing,
                    deadline: now + FADER_CONFIRMATION_GRACE,
                });
                if editing && !edit.editing {
                    edit.sent = [false; 256];
                }
                edit.command_id = id;
                edit.target = raw;
                edit.editing = editing;
                edit.deadline = now + FADER_CONFIRMATION_GRACE;
                self.fader_commands.push_back((id, target, raw));
                while self.fader_commands.len() > MAX_PENDING_COMMANDS * 2 + 1 {
                    self.fader_commands.pop_front();
                }
            }
        }
        self.queue.push_tracked(id, command)?;
        Ok(())
    }

    /// Local write bookkeeping; never creates protocol sequence numbers.
    pub fn command_started(&mut self, id: u64) {
        if let Some((_, target, raw)) = self.fader_commands.iter().find(|(key, _, _)| *key == id)
            && let Some(edit) = self.fader_edits.get_mut(target)
        {
            edit.sent[*raw as usize] = true;
        }
        self.fader_commands.retain(|(key, _, _)| *key != id);
    }

    pub fn set_fader_pointer_down(&mut self, down: bool, now: Instant) {
        if self.fader_pointer_down && !down {
            for edit in self.fader_edits.values_mut().filter(|edit| edit.editing) {
                edit.editing = false;
                edit.deadline = now + FADER_CONFIRMATION_GRACE;
            }
        }
        self.fader_pointer_down = down;
    }

    pub fn command_failed(&mut self, id: u64, reason: &str) {
        self.visit_pending(&mut |value| value.reject(Some(id), reason));
        self.fader_edits.retain(|_, edit| edit.command_id != id);
        self.fader_commands.retain(|(key, _, _)| *key != id);
    }

    pub fn clear_pending(&mut self, reason: &str) {
        self.visit_pending(&mut |value| value.reject(None, reason));
        self.queue.clear();
        self.fader_edits.clear();
        self.fader_commands.clear();
        self.fader_pointer_down = false;
    }

    pub fn expire_pending(&mut self, now: Instant) -> bool {
        // Composite channel/output observations also clear pending values.
        // Do not report a timeout for an edit they already reconciled.
        let targets = self.fader_edits.keys().copied().collect::<Vec<_>>();
        for target in targets {
            if self.fader_cell(target).pending.is_none() {
                self.fader_edits.remove(&target);
            }
        }
        self.fader_commands
            .retain(|(_, target, _)| self.fader_edits.contains_key(target));
        let mut expired = Vec::new();
        for edit in self
            .fader_edits
            .values()
            .filter(|edit| !edit.editing && now >= edit.deadline)
        {
            expired.push(edit.command_id);
        }
        for id in &expired {
            self.visit_pending(&mut |value| {
                value.reject(Some(*id), "Device confirmation timed out")
            });
        }
        self.visit_pending(&mut |value| {
            if let Some(id) = value.expire(now) {
                expired.push(id);
            }
        });
        self.queue.commands.retain(|(id, _)| !expired.contains(id));
        self.fader_edits
            .retain(|_, edit| !expired.contains(&edit.command_id));
        self.fader_commands
            .retain(|(id, _, _)| !expired.contains(id));
        !expired.is_empty()
    }

    pub fn next_pending_deadline(&mut self) -> Option<Instant> {
        let mut next = self
            .fader_edits
            .values()
            .filter(|edit| !edit.editing)
            .map(|edit| edit.deadline)
            .min();
        self.visit_pending(&mut |value| {
            if let Some(deadline) = value.deadline() {
                next = Some(next.map_or(deadline, |previous| previous.min(deadline)));
            }
        });
        next
    }

    fn visit_pending(&mut self, visit: &mut dyn FnMut(&mut dyn PendingCell)) {
        macro_rules! fields {
            ($($field:expr),+ $(,)?) => { $(visit(&mut $field);)+ };
        }
        for channel in &mut self.state.channels {
            fields!(
                channel.gain_db,
                channel.pan,
                channel.muted,
                channel.soloed,
                channel.phase_inverted,
                channel.phantom_48v,
                channel.high_pass_enabled,
                channel.high_pass_hz,
                channel.name,
                channel.icon,
                channel.compressor.amount
            );
            for level in &mut channel.route_levels {
                visit(level);
            }
            for band in &mut channel.eq.bands {
                fields!(band.frequency_hz, band.q, band.gain_db);
            }
        }
        for bus in &mut self.state.buses {
            fields!(
                bus.master_level,
                bus.muted,
                bus.limiter.threshold_db,
                bus.delay_ticks
            );
            if let Some(balance) = &mut bus.balance {
                visit(balance);
            }
            for band in &mut bus.eq.bands {
                fields!(band.frequency_hz, band.q, band.gain_db);
            }
        }
        for effect in &mut self.state.effects {
            fields!(
                effect.master_level,
                effect.pan,
                effect.muted,
                effect.preset,
                effect.return_to_main,
                effect.return_to_mon1,
                effect.return_to_mon2
            );
            for parameter in &mut effect.parameters {
                visit(parameter);
            }
        }
        fields!(
            self.state.global_tempo_bpm,
            self.state.device_selected_output,
            self.state.snapshots.last_loaded,
            self.state.routing.monitor_link.stereo_linked,
            self.state.routing.headphones.source,
            self.state.routing.headphones.tap_point
        );
        for slot in &mut self.state.snapshots.device_slots {
            visit(&mut slot.name);
        }
        let settings = &mut self.state.routing.settings;
        fields!(
            settings.bt_usb_switch,
            settings.phones_only,
            settings.footswitch_fx_mode,
            settings.device_name,
            settings.usb_streaming,
            settings.monitor_routing,
            settings.input56_from_usb12,
            settings.input78_from_usb34,
            settings.main_minus_10_dbv,
            settings.monitor_minus_10_dbv,
            settings.snapshot_scope_bits,
            settings.monitor_post_fader,
            settings.device_linked_selection
        );
    }

    fn set_mute_pending(
        &mut self,
        target: MuteTarget,
        enabled: bool,
        synthetic: bool,
    ) -> Result<(), CoreError> {
        let value = match target {
            MuteTarget::Input(input) => &mut self.state.channels[input.index()].muted,
            MuteTarget::Bus(bus) => {
                &mut self
                    .state
                    .bus_for_destination_mut(destination_for_bus(bus))
                    .ok_or(CoreError::InvalidValue)?
                    .muted
            }
            MuteTarget::Fx(fx) => &mut self.state.effects[fx_index(fx)].muted,
        };
        value.set_pending(enabled);
        if synthetic {
            value.observe(enabled, EvidenceStatus::Synthetic);
        }
        Ok(())
    }

    fn set_setting_pending(&mut self, setting: &KnownSetting, synthetic: bool) {
        macro_rules! update {
            ($target:expr, $value:expr) => {{
                let target = $target;
                let value = $value;
                target.set_pending(value.clone());
                if synthetic {
                    target.observe(value, EvidenceStatus::Synthetic);
                }
            }};
        }
        match setting {
            KnownSetting::BtUsbSwitch(value) => {
                update!(&mut self.state.routing.settings.bt_usb_switch, *value)
            }
            KnownSetting::PhonesOnly(value) => {
                update!(&mut self.state.routing.settings.phones_only, *value)
            }
            KnownSetting::FootswitchFxMode(value) => {
                update!(&mut self.state.routing.settings.footswitch_fx_mode, *value)
            }
            KnownSetting::DeviceName(value) => {
                update!(&mut self.state.routing.settings.device_name, value.clone())
            }
            KnownSetting::UsbStreaming(value) => {
                update!(&mut self.state.routing.settings.usb_streaming, *value)
            }
            KnownSetting::MonitorRouting(value) => {
                update!(&mut self.state.routing.settings.monitor_routing, *value)
            }
            KnownSetting::Input56FromUsb12(value) => {
                update!(&mut self.state.routing.settings.input56_from_usb12, *value)
            }
            KnownSetting::Input78FromUsb34(value) => {
                update!(&mut self.state.routing.settings.input78_from_usb34, *value)
            }
            KnownSetting::MonitorStereoLink(value) => {
                update!(&mut self.state.routing.monitor_link.stereo_linked, *value)
            }
            KnownSetting::HeadphonesUseMonitor(value) => update!(
                &mut self.state.routing.headphones.source,
                if *value {
                    HeadphoneSource::Monitor
                } else {
                    HeadphoneSource::Main
                }
            ),
            KnownSetting::HeadphonesPostFader(value) => update!(
                &mut self.state.routing.headphones.tap_point,
                if *value {
                    TapPoint::PostFader
                } else {
                    TapPoint::PreFader
                }
            ),
            KnownSetting::MainMinus10Dbv(value) => {
                update!(&mut self.state.routing.settings.main_minus_10_dbv, *value)
            }
            KnownSetting::MonitorMinus10Dbv(value) => update!(
                &mut self.state.routing.settings.monitor_minus_10_dbv,
                *value
            ),
            KnownSetting::SnapshotScope(value) => {
                update!(&mut self.state.routing.settings.snapshot_scope_bits, *value)
            }
            KnownSetting::MonitorPostFader(value) => {
                update!(&mut self.state.routing.settings.monitor_post_fader, *value)
            }
            KnownSetting::DeviceLinkedSelection(value) => update!(
                &mut self.state.routing.settings.device_linked_selection,
                *value
            ),
        }
    }

    pub fn tick_simulator(&mut self, dt_seconds: f32) {
        if !self.simulator_mode {
            return;
        }
        let Ok(elapsed) = Duration::try_from_secs_f64(f64::from(dt_seconds)) else {
            return;
        };
        self.simulator_time = self.simulator_time.saturating_add(elapsed);
        let time = self.simulator_time.as_secs_f64();
        for (index, channel) in self.state.channels.iter_mut().enumerate() {
            let route = *channel.route_levels[self.state.selected_destination.index()]
                .effective()
                .unwrap_or(&0.0);
            let motion = ((time * (1.2 + index as f64 * 0.09)).sin() as f32 * 0.5 + 0.5) * 14.0;
            let level = (-58.0 + route * 58.0 + motion).clamp(-60.0, 10.0);
            channel
                .meter
                .level_db
                .observe(level, EvidenceStatus::Synthetic);
            channel
                .meter
                .peak_db
                .observe((level + 3.0).min(10.0), EvidenceStatus::Synthetic);
            channel
                .meter
                .clipping
                .observe(level >= 9.5, EvidenceStatus::Synthetic);
        }
        let bucket = self.simulator_time.as_secs() / 8;
        if bucket > self.simulator_event_bucket {
            self.simulator_event_bucket = bucket;
            let parameter = (((bucket % 101) * 17) % 101) as u8;
            self.state.effects[0].parameters[0].observe(parameter, EvidenceStatus::Synthetic);
            self.state.snapshots.last_loaded.observe(
                Some((bucket % self.state.snapshots.device_slots.len() as u64) as u8),
                EvidenceStatus::Synthetic,
            );
        }
    }

    /// Atomically validates and applies a composite state. Invalid endpoint
    /// ordering cannot partially mutate the store.
    pub fn apply_rx(
        &mut self,
        command: RxCommand,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        match command {
            RxCommand::Pan { endpoint, value } => self.apply_pan(endpoint, value, evidence),
            RxCommand::Solo { endpoint, enabled } => {
                if let Some(input) = input_from_endpoint(endpoint) {
                    self.state.channels[input.index()]
                        .soloed
                        .observe(enabled, evidence);
                }
                // Output-solo atomic state is not proven by the APK evidence.
                Ok(())
            }
            RxCommand::Gain { endpoint, db } => {
                let input =
                    input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                let channel = &mut self.state.channels[input.index()];
                if channel.capabilities.gain {
                    validate_received_value(db, specs::INPUT_GAIN_WIRE)?;
                    channel.gain_db.observe(db, evidence);
                }
                Ok(())
            }
            RxCommand::GraphicEq {
                endpoint,
                band,
                frequency_hz,
                q,
                gain_db,
            } => self.apply_geq(endpoint, band, frequency_hz, q, gain_db, evidence),
            RxCommand::HighPassFilter {
                endpoint,
                enabled,
                frequency_hz,
            } => {
                let input =
                    input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                let channel = &mut self.state.channels[input.index()];
                if channel.capabilities.high_pass {
                    channel.high_pass_enabled.observe(enabled, evidence);
                    channel.high_pass_hz.observe(frequency_hz as f32, evidence);
                }
                Ok(())
            }
            RxCommand::Label(label) => self.apply_label(label, evidence),
            RxCommand::RouteState {
                endpoint_a,
                destination,
                level_db,
            } => self.apply_route(endpoint_a, destination, level_db, evidence),
            RxCommand::Mute { endpoint, enabled } => self.apply_mute(endpoint, enabled, evidence),
            RxCommand::ParametricEq {
                endpoint,
                band,
                frequency_hz,
                q,
                gain_db,
            } => self.apply_peq(endpoint, band, frequency_hz, q, gain_db, evidence),
            RxCommand::Phase { endpoint, inverted } => {
                let input =
                    input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                let channel = &mut self.state.channels[input.index()];
                if channel.capabilities.phase {
                    channel.phase_inverted.observe(inverted, evidence);
                }
                Ok(())
            }
            RxCommand::FxSetup {
                endpoint,
                values,
                route_flags,
            } => self.apply_fx_setup(endpoint, values, route_flags, evidence),
            RxCommand::SnapshotDelete { slot } => self.snapshot_name(slot, String::new(), evidence),
            RxCommand::Compressor { endpoint, amount } => {
                let input =
                    input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                let channel = &mut self.state.channels[input.index()];
                if channel.capabilities.compressor {
                    validate_received_value(amount, specs::COMPRESSOR_AMOUNT)?;
                    channel.compressor.amount.observe(amount, evidence);
                }
                Ok(())
            }
            RxCommand::Limiter {
                endpoint,
                threshold_db,
            } => {
                let destination = destination_from_endpoint(endpoint)
                    .filter(|destination| {
                        !matches!(destination, MixDestination::Fx1 | MixDestination::Fx2)
                    })
                    .ok_or(CoreError::InvalidCompositeState)?;
                validate_received_value(threshold_db, specs::LIMITER)?;
                self.state
                    .bus_for_destination_mut(destination)
                    .ok_or(CoreError::InvalidCompositeState)?
                    .limiter
                    .threshold_db
                    .observe(threshold_db, evidence);
                Ok(())
            }
            RxCommand::Phantom { endpoint, enabled } => {
                let input =
                    input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                if self.state.channels[input.index()].capabilities.phantom {
                    self.state.channels[input.index()]
                        .phantom_48v
                        .observe(enabled, evidence);
                }
                Ok(())
            }
            RxCommand::GetChannelState { .. }
            | RxCommand::MeterRequest(_)
            | RxCommand::GetSnapshotNames
            | RxCommand::GetChannelLabels
            | RxCommand::GetSetting { .. }
            | RxCommand::GetMixerState
            | RxCommand::HandshakeClient { .. } => Ok(()),
            RxCommand::InputState(input) => self.apply_input(input, evidence),
            RxCommand::OutputState(output) => self.apply_output(output, evidence),
            RxCommand::SnapshotSave { slot, name } | RxCommand::SnapshotRename { slot, name } => {
                self.snapshot_name(slot, name, evidence)
            }
            RxCommand::SnapshotLoad { slot } => {
                self.state
                    .snapshots
                    .last_loaded
                    .observe(Some(slot), evidence);
                if self.simulator_mode {
                    self.state.session = SessionState::StateSyncing;
                }
                Ok(())
            }
            RxCommand::MeterUpdate(update) => {
                self.apply_meter_update(update, evidence);
                Ok(())
            }
            RxCommand::Setting { id, data } => self.apply_setting(id, &data, evidence),
            RxCommand::FactoryReset => {
                if self.simulator_mode {
                    self.state.session = SessionState::StateSyncing;
                }
                Ok(())
            }
            RxCommand::FxState(effect) => self.apply_fx(effect, evidence),
            RxCommand::FxPreset { endpoint, preset } => {
                let index =
                    fx_index_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                self.state.effects[index].preset.observe(preset, evidence);
                Ok(())
            }
            RxCommand::ChannelConnection {
                endpoint,
                subchannel,
                connected,
            } => {
                let input =
                    input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
                let channel = &mut self.state.channels[input.index()];
                match subchannel {
                    0 => channel.left_connected.observe(connected, evidence),
                    _ => channel.right_connected.observe(connected, evidence),
                }
                Ok(())
            }
            RxCommand::MixerState(mixer) => {
                validate_received_level(mixer.headphone_volume_db)?;
                let mut output_ids = mixer
                    .outputs
                    .iter()
                    .map(|output| output.id)
                    .collect::<Vec<_>>();
                output_ids.sort_unstable();
                let mut effect_ids = mixer
                    .effects
                    .iter()
                    .map(|effect| effect.id)
                    .collect::<Vec<_>>();
                effect_ids.sort_unstable();
                if !mixer
                    .inputs
                    .iter()
                    .enumerate()
                    .all(|(index, input)| input.id as usize == index)
                    || output_ids != [10, 11, 15]
                    || effect_ids != [12, 13]
                {
                    return Err(CoreError::InvalidCompositeState);
                }
                // Construct the full replacement off to the side. No decoded field is
                // visible until every endpoint applies, so malformed composite state cannot
                // leave a partially replaced confirmed state behind.
                let mut candidate = Flow8Store {
                    state: self.state.clone(),
                    queue: SemanticCommandQueue::default(),
                    meter_output_target: self.meter_output_target,
                    simulator_time: self.simulator_time,
                    simulator_event_bucket: self.simulator_event_bucket,
                    simulator_mode: self.simulator_mode,
                    next_command_id: self.next_command_id,
                    fader_edits: HashMap::new(),
                    fader_commands: VecDeque::new(),
                    fader_pointer_down: false,
                };
                for input in mixer.inputs {
                    candidate.apply_input(input, evidence)?;
                }
                for output in mixer.outputs {
                    candidate.apply_output(output, evidence)?;
                }
                for effect in mixer.effects {
                    candidate.apply_fx(effect, evidence)?;
                }
                candidate
                    .state
                    .routing
                    .monitor_link
                    .stereo_linked
                    .observe(mixer.flags[7], evidence);
                candidate
                    .state
                    .global_tempo_bpm
                    .observe(mixer.tempo_bpm as f32, evidence);
                candidate
                    .state
                    .mixer_flags_raw
                    .observe(mixer.raw_flags, evidence);
                candidate
                    .state
                    .headphone_volume_db
                    .observe(mixer.headphone_volume_db, evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .bt_usb_switch
                    .observe(mixer.flags[0], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .phones_only
                    .observe(mixer.flags[1], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .footswitch_fx_mode
                    .observe(mixer.flags[2], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .usb_streaming
                    .observe(mixer.flags[4], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .input56_from_usb12
                    .observe(mixer.flags[5], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .input78_from_usb34
                    .observe(mixer.flags[6], evidence);
                candidate.state.routing.headphones.source.observe(
                    if mixer.flags[8] {
                        HeadphoneSource::Monitor
                    } else {
                        HeadphoneSource::Main
                    },
                    evidence,
                );
                candidate.state.routing.headphones.tap_point.observe(
                    if mixer.flags[9] {
                        TapPoint::PostFader
                    } else {
                        TapPoint::PreFader
                    },
                    evidence,
                );
                candidate
                    .state
                    .routing
                    .settings
                    .main_minus_10_dbv
                    .observe(mixer.flags[10], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .monitor_minus_10_dbv
                    .observe(mixer.flags[11], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .monitor_post_fader
                    .observe(mixer.flags[12], evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .monitor_routing
                    .observe(monitor_routing(mixer.monitor_routing), evidence);
                candidate
                    .state
                    .routing
                    .settings
                    .snapshot_scope_bits
                    .observe(mixer.snapshot_scope, evidence);
                candidate
                    .state
                    .device_selected_output
                    .observe(destination_from_endpoint(mixer.selected_output), evidence);
                candidate
                    .state
                    .snapshots
                    .last_loaded
                    .observe(Some(mixer.last_snapshot), evidence);
                self.state = candidate.state;
                self.fader_edits.clear();
                self.fader_commands.clear();
                Ok(())
            }
            RxCommand::FxTempo { bpm } => {
                self.state.global_tempo_bpm.observe(bpm as f32, evidence);
                Ok(())
            }
            RxCommand::SelectOutput { endpoint } => {
                self.state
                    .device_selected_output
                    .observe(destination_from_endpoint(endpoint), evidence);
                Ok(())
            }
            RxCommand::ChannelDelay { endpoint, ticks } => {
                let destination = destination_from_endpoint(endpoint)
                    .filter(|destination| {
                        !matches!(destination, MixDestination::Fx1 | MixDestination::Fx2)
                    })
                    .ok_or(CoreError::InvalidCompositeState)?;
                self.state
                    .bus_for_destination_mut(destination)
                    .ok_or(CoreError::InvalidCompositeState)?
                    .delay_ticks
                    .observe(ticks, evidence);
                Ok(())
            }
            RxCommand::ChannelLabels(labels) => {
                for label in labels {
                    if input_from_endpoint(label.endpoint).is_some() {
                        self.apply_label(label, evidence)?;
                    }
                }
                Ok(())
            }
            RxCommand::SnapshotNames(names) => {
                for (index, name) in names.into_iter().enumerate() {
                    if let Some(slot) = self.state.snapshots.device_slots.get_mut(index) {
                        slot.name.observe(name, evidence);
                    }
                }
                Ok(())
            }
            RxCommand::HandshakeHost { .. } => {
                if self.simulator_mode {
                    self.state.session = SessionState::Handshaking;
                }
                Ok(())
            }
            RxCommand::HandshakeReply => {
                if self.simulator_mode {
                    self.state.session = SessionState::StateSyncing;
                }
                Ok(())
            }
        }
    }

    fn apply_pan(
        &mut self,
        endpoint: u8,
        value: f32,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        if let Some(input) = input_from_endpoint(endpoint) {
            validate_received_value(value, specs::PAN)?;
            self.state.channels[input.index()]
                .pan
                .observe(value, evidence);
            return Ok(());
        }
        if let Some(index) = fx_index_from_endpoint(endpoint) {
            validate_received_value(value, specs::PAN)?;
            self.state.effects[index].pan.observe(value, evidence);
            return Ok(());
        }
        if let Some(destination) = destination_from_endpoint(endpoint) {
            let bus = self
                .state
                .bus_for_destination_mut(destination)
                .ok_or(CoreError::InvalidCompositeState)?;
            if let Some(balance) = &mut bus.balance {
                validate_received_value(value, specs::PAN)?;
                balance.observe(value, evidence);
            }
            // MON buses intentionally have no exposed balance. A notification
            // for that endpoint is not evidence of a new editable parameter.
            return Ok(());
        }
        Err(CoreError::InvalidCompositeState)
    }

    fn apply_geq(
        &mut self,
        endpoint: u8,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        validate_received_eq(q, gain_db)?;
        let destination = destination_from_endpoint(endpoint)
            .filter(|value| !matches!(value, MixDestination::Fx1 | MixDestination::Fx2))
            .ok_or(CoreError::InvalidCompositeState)?;
        let target = self
            .state
            .bus_for_destination_mut(destination)
            .and_then(|bus| bus.eq.bands.get_mut(band as usize))
            .ok_or(CoreError::InvalidCompositeState)?;
        target.frequency_hz.observe(frequency_hz as f32, evidence);
        target.q.observe(q, evidence);
        target.gain_db.observe(gain_db, evidence);
        Ok(())
    }

    fn apply_peq(
        &mut self,
        endpoint: u8,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let input = input_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
        if !self.state.channels[input.index()].capabilities.peq {
            return Ok(());
        }
        validate_received_eq(q, gain_db)?;
        let target = self.state.channels[input.index()]
            .eq
            .bands
            .get_mut(band as usize)
            .ok_or(CoreError::InvalidCompositeState)?;
        target.frequency_hz.observe(frequency_hz as f32, evidence);
        target.q.observe(q, evidence);
        target.gain_db.observe(gain_db, evidence);
        Ok(())
    }

    fn apply_label(
        &mut self,
        label: flow8_protocol::ChannelLabel,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let input = input_from_endpoint(label.endpoint).ok_or(CoreError::InvalidCompositeState)?;
        let channel = &mut self.state.channels[input.index()];
        channel.name.observe(label.text, evidence);
        channel.icon.observe(label.icon, evidence);
        Ok(())
    }

    fn apply_route(
        &mut self,
        endpoint_a: u8,
        endpoint_b: u8,
        level_db: f32,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        validate_received_level(level_db)?;
        let destination =
            destination_from_endpoint(endpoint_b).ok_or(CoreError::InvalidCompositeState)?;
        let target = if endpoint_a == endpoint_b {
            FaderTarget::Master(destination)
        } else {
            FaderTarget::Route(
                input_from_endpoint(endpoint_a).ok_or(CoreError::InvalidCompositeState)?,
                destination,
            )
        };
        let normalized = db_to_normalized(level_db);
        let raw = flow8_protocol::encode_fix8(flow8_protocol::Fix8Format::FaderDb, level_db)
            .ok_or(CoreError::InvalidCompositeState)?;
        let known_old_echo = self.fader_edits.get(&target).is_some_and(|edit| {
            (edit.editing || Instant::now() < edit.deadline)
                && raw != edit.target
                && edit.sent[raw as usize]
        });
        let cell = self.fader_cell(target);
        if known_old_echo && cell.pending.is_some() {
            // Confirmed state always updates. Only a known intermediate write
            // echo retains the latest edit; an unexpected device value wins.
            cell.observe_preserving_pending(normalized, evidence);
        } else {
            cell.observe(normalized, evidence);
            self.fader_edits.remove(&target);
            self.fader_commands
                .retain(|(_, queued_target, _)| *queued_target != target);
        }
        Ok(())
    }

    fn fader_cell(&mut self, target: FaderTarget) -> &mut StateValue<f32> {
        match target {
            FaderTarget::Route(input, destination) => {
                &mut self.state.channels[input.index()].route_levels[destination.index()]
            }
            FaderTarget::Master(destination) => match destination {
                MixDestination::Main => &mut self.state.buses[0].master_level,
                MixDestination::Monitor1 => &mut self.state.buses[1].master_level,
                MixDestination::Monitor2 => &mut self.state.buses[2].master_level,
                MixDestination::Fx1 => &mut self.state.effects[0].master_level,
                MixDestination::Fx2 => &mut self.state.effects[1].master_level,
            },
        }
    }

    fn apply_mute(
        &mut self,
        endpoint: u8,
        enabled: bool,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        if let Some(input) = input_from_endpoint(endpoint) {
            self.state.channels[input.index()]
                .muted
                .observe(enabled, evidence);
            return Ok(());
        }
        if let Some(index) = fx_index_from_endpoint(endpoint) {
            self.state.effects[index].muted.observe(enabled, evidence);
            return Ok(());
        }
        let destination =
            destination_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
        self.state
            .bus_for_destination_mut(destination)
            .ok_or(CoreError::InvalidCompositeState)?
            .muted
            .observe(enabled, evidence);
        Ok(())
    }

    fn apply_fx_setup(
        &mut self,
        endpoint: u8,
        values: [u8; 3],
        route_flags: u8,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let index = fx_index_from_endpoint(endpoint).ok_or(CoreError::InvalidCompositeState)?;
        let effect = &mut self.state.effects[index];
        for (target, value) in effect.parameters.iter_mut().zip(values) {
            target.observe(value, evidence);
        }
        effect
            .return_to_main
            .observe(route_flags & 0x01 != 0, evidence);
        effect
            .return_to_mon1
            .observe(route_flags & 0x02 != 0, evidence);
        effect
            .return_to_mon2
            .observe(route_flags & 0x04 != 0, evidence);
        Ok(())
    }

    fn snapshot_name(
        &mut self,
        slot: u8,
        name: String,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let target = self
            .state
            .snapshots
            .device_slots
            .get_mut(slot as usize)
            .ok_or(CoreError::InvalidCompositeState)?;
        target.name.observe(name, evidence);
        Ok(())
    }

    fn apply_meter_update(
        &mut self,
        update: flow8_protocol::MeterUpdate,
        evidence: EvidenceStatus,
    ) {
        let stereo_level = |left: f32, right: f32| left.max(right);
        let input_levels = [
            update.meters_db[0],
            update.meters_db[1],
            update.meters_db[2],
            update.meters_db[3],
            stereo_level(update.meters_db[4], update.meters_db[5]),
            stereo_level(update.meters_db[6], update.meters_db[7]),
            stereo_level(update.meters_db[8], update.meters_db[9]),
        ];
        for (index, level) in input_levels.into_iter().enumerate() {
            let meter = &mut self.state.channels[index].meter;
            meter.level_db.observe(level, evidence);
            meter.peak_db.observe(level, evidence);
            meter.clipping.observe(level >= 0.0, evidence);
            let raw_reduction = ((update.gain_reduction_bits >> (index * 2)) & 0x03) as f32;
            meter
                .gain_reduction_db
                .observe(-raw_reduction, EvidenceStatus::Unknown);
        }
        let output_level = stereo_level(update.meters_db[10], update.meters_db[11]);
        // 0x21 selects the meter output independently of the physical output.
        let selected =
            self.meter_output_target
                .or(self.state.device_selected_output.confirmed.flatten());
        if let Some(destination) = selected
            && let Some(bus) = self.state.bus_for_destination_mut(destination)
        {
            bus.meter.level_db.observe(output_level, evidence);
            bus.meter.peak_db.observe(output_level, evidence);
            bus.meter.clipping.observe(output_level >= 0.0, evidence);
            let raw_reduction = ((update.gain_reduction_bits >> 14) & 0x03) as f32;
            bus.meter
                .gain_reduction_db
                .observe(-raw_reduction, EvidenceStatus::Unknown);
        } else if let Some(destination) = selected
            && let Some(index) = match destination {
                MixDestination::Fx1 => Some(0),
                MixDestination::Fx2 => Some(1),
                _ => None,
            }
        {
            self.state.effects[index]
                .meter
                .level_db
                .observe(output_level, evidence);
        }
    }

    fn apply_setting(
        &mut self,
        id: u8,
        data: &[u8],
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let boolean = || match data {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(CoreError::InvalidCompositeState),
        };
        match id {
            0x01 => self
                .state
                .routing
                .settings
                .bt_usb_switch
                .observe(boolean()?, evidence),
            0x02 => self
                .state
                .routing
                .settings
                .phones_only
                .observe(boolean()?, evidence),
            0x03 => self
                .state
                .routing
                .settings
                .footswitch_fx_mode
                .observe(boolean()?, evidence),
            0x05 => self
                .state
                .routing
                .settings
                .device_name
                .observe(String::from_utf8_lossy(data).into_owned(), evidence),
            0x07 => self
                .state
                .routing
                .settings
                .usb_streaming
                .observe(boolean()?, evidence),
            0x08 => {
                let [value] = data else {
                    return Err(CoreError::InvalidCompositeState);
                };
                self.state
                    .routing
                    .settings
                    .monitor_routing
                    .observe(monitor_routing(*value), evidence);
            }
            0x09 => self
                .state
                .routing
                .settings
                .input56_from_usb12
                .observe(boolean()?, evidence),
            0x0a => self
                .state
                .routing
                .settings
                .input78_from_usb34
                .observe(boolean()?, evidence),
            0x0b => self
                .state
                .routing
                .monitor_link
                .stereo_linked
                .observe(boolean()?, evidence),
            0x0c => self.state.routing.headphones.source.observe(
                if boolean()? {
                    HeadphoneSource::Monitor
                } else {
                    HeadphoneSource::Main
                },
                evidence,
            ),
            0x0d => self.state.routing.headphones.tap_point.observe(
                if boolean()? {
                    TapPoint::PostFader
                } else {
                    TapPoint::PreFader
                },
                evidence,
            ),
            0x0e => self
                .state
                .routing
                .settings
                .main_minus_10_dbv
                .observe(boolean()?, evidence),
            0x0f => self
                .state
                .routing
                .settings
                .monitor_minus_10_dbv
                .observe(boolean()?, evidence),
            0x10 => {
                let [value] = data else {
                    return Err(CoreError::InvalidCompositeState);
                };
                self.state
                    .routing
                    .settings
                    .snapshot_scope_bits
                    .observe(*value, evidence);
            }
            0x11 => self
                .state
                .routing
                .settings
                .monitor_post_fader
                .observe(boolean()?, evidence),
            0xb0 => self
                .state
                .routing
                .settings
                .device_linked_selection
                .observe(boolean()?, evidence),
            // 0x04 and 0x06 deliberately remain unnamed/UNKNOWN.
            _ => return Ok(()),
        }
        Ok(())
    }

    fn apply_input(
        &mut self,
        input: flow8_protocol::InputState,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let channel = self
            .state
            .channels
            .get_mut(input.id as usize)
            .ok_or(CoreError::InvalidCompositeState)?;
        // Validate every applicable field before touching confirmed state.
        // Unsupported USB/BT fields may be padding; never validate or apply
        // them as if they were real input controls.
        validate_received_value(input.balance, specs::PAN)?;
        if channel.capabilities.gain {
            validate_received_value(input.gain_db, specs::INPUT_GAIN_WIRE)?;
        }
        if channel.capabilities.compressor {
            validate_received_value(input.compressor_amount, specs::COMPRESSOR_AMOUNT)?;
        }
        if channel.capabilities.peq {
            for (q, gain) in input.eq_q.iter().zip(&input.eq_gain_db) {
                validate_received_eq(*q, *gain)?;
            }
        }
        if channel.capabilities.gain {
            channel.gain_db.observe(input.gain_db, evidence);
        }
        channel.pan.observe(input.balance, evidence);
        channel.muted.observe(input.flags & 0x01 != 0, evidence);
        channel.soloed.observe(input.flags & 0x40 != 0, evidence);
        if channel.capabilities.high_pass {
            channel
                .high_pass_enabled
                .observe(input.flags & 0x02 != 0, evidence);
            channel
                .high_pass_hz
                .observe(input.high_pass_hz as f32, evidence);
        }
        if channel.capabilities.phase {
            channel
                .phase_inverted
                .observe(input.flags & 0x04 != 0, evidence);
        }
        if channel.capabilities.phantom {
            channel
                .phantom_48v
                .observe(input.flags & 0x08 != 0, evidence);
        }
        if channel.capabilities.compressor {
            channel
                .compressor
                .amount
                .observe(input.compressor_amount, evidence);
        }
        channel.name.observe(input.label.text, evidence);
        channel.icon.observe(input.label.icon, evidence);
        channel
            .left_connected
            .observe(input.flags & 0x10 != 0, evidence);
        channel
            .right_connected
            .observe(input.flags & 0x20 != 0, evidence);
        if channel.capabilities.peq {
            for (index, band) in channel.eq.bands.iter_mut().enumerate() {
                band.gain_db.observe(input.eq_gain_db[index], evidence);
                band.frequency_hz
                    .observe(input.eq_frequency_hz[index] as f32, evidence);
                band.q.observe(input.eq_q[index], evidence);
            }
        }
        Ok(())
    }

    fn apply_output(
        &mut self,
        output: flow8_protocol::OutputState,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let destination = match output.id {
            15 => MixDestination::Main,
            10 => MixDestination::Monitor1,
            11 => MixDestination::Monitor2,
            _ => return Err(CoreError::InvalidCompositeState),
        };
        validate_received_level(output.volume_db)?;
        validate_received_value(output.limiter_db, specs::LIMITER)?;
        if destination == MixDestination::Main {
            validate_received_value(output.pan, specs::PAN)?;
        }
        for (q, gain) in output.eq_q.iter().zip(&output.eq_gain_db) {
            validate_received_eq(*q, *gain)?;
        }
        for gain in output.input_gains_db {
            validate_received_level(gain)?;
        }
        let bus = self
            .state
            .bus_for_destination_mut(destination)
            .ok_or(CoreError::InvalidCompositeState)?;
        bus.master_level
            .observe(db_to_normalized(output.volume_db), evidence);
        bus.muted.observe(output.flags & 1 != 0, evidence);
        bus.soloed.observe(output.flags & 0x02 != 0, evidence);
        if let Some(balance) = &mut bus.balance {
            balance.observe(output.pan, evidence);
        }
        bus.limiter
            .threshold_db
            .observe(output.limiter_db, evidence);
        bus.delay_ticks.observe(output.delay_ticks, evidence);
        for (index, band) in bus.eq.bands.iter_mut().enumerate() {
            band.gain_db.observe(output.eq_gain_db[index], evidence);
            band.frequency_hz
                .observe(output.eq_frequency_hz[index] as f32, evidence);
            band.q.observe(output.eq_q[index], evidence);
        }
        for (index, gain) in output.input_gains_db.into_iter().enumerate() {
            self.state.channels[index].route_levels[destination.index()]
                .observe(db_to_normalized(gain), evidence);
        }
        Ok(())
    }

    fn apply_fx(
        &mut self,
        effect: flow8_protocol::FxState,
        evidence: EvidenceStatus,
    ) -> Result<(), CoreError> {
        let (index, destination) = match effect.id {
            12 => (0, MixDestination::Fx1),
            13 => (1, MixDestination::Fx2),
            _ => return Err(CoreError::InvalidCompositeState),
        };
        validate_received_level(effect.volume_db)?;
        validate_received_value(effect.pan, specs::PAN)?;
        for gain in effect.input_gains_db.iter().chain(&effect.aux_gains_db) {
            validate_received_level(*gain)?;
        }
        let fx = &mut self.state.effects[index];
        fx.master_level
            .observe(db_to_normalized(effect.volume_db), evidence);
        fx.pan.observe(effect.pan, evidence);
        fx.muted.observe(effect.flags & 1 != 0, evidence);
        fx.return_to_main.observe(effect.flags & 2 != 0, evidence);
        fx.return_to_mon1.observe(effect.flags & 4 != 0, evidence);
        fx.return_to_mon2.observe(effect.flags & 8 != 0, evidence);
        fx.preset.observe(effect.preset, evidence);
        for (target, value) in fx.parameters.iter_mut().zip(effect.values) {
            target.observe(value, evidence);
        }
        for (target, value) in fx.aux_gains_db.iter_mut().zip(effect.aux_gains_db) {
            target.observe(value, evidence);
        }
        for (input, gain) in effect.input_gains_db.into_iter().enumerate() {
            self.state.channels[input].route_levels[destination.index()]
                .observe(db_to_normalized(gain), evidence);
        }
        Ok(())
    }
}

pub fn db_to_normalized(db: f32) -> f32 {
    if db >= 10.0 {
        1.0
    } else if db >= -10.0 {
        (db + 30.0) / 40.0
    } else if db >= -30.0 {
        (db + 50.0) / 80.0
    } else if db >= -60.0 {
        (db + 70.0) / 160.0
    } else if db > -144.0 {
        (db + 90.0) / 480.0
    } else {
        0.0
    }
    .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_store_has_topology_but_no_synthetic_confirmed_state() {
        let store = Flow8Store::default();
        assert!(!store.is_simulator());
        assert_eq!(store.state.session, SessionState::Disconnected);
        assert_eq!(store.state.channels.len(), 7);
        assert_eq!(store.state.buses.len(), 3);
        assert_eq!(store.state.effects.len(), 2);
        assert!(store.state.channels.iter().all(|channel| {
            channel.name.confirmed.is_none()
                && channel.gain_db.confirmed.is_none()
                && channel
                    .route_levels
                    .iter()
                    .all(|route| route.confirmed.is_none())
                && channel.meter.level_db.confirmed.is_none()
        }));
        assert!(
            store
                .state
                .buses
                .iter()
                .all(|bus| bus.master_level.confirmed.is_none())
        );
        assert!(
            store
                .state
                .effects
                .iter()
                .all(|effect| effect.preset.confirmed.is_none())
        );
        assert!(store.state.routing.settings.device_name.confirmed.is_none());
        assert!(store.state.global_tempo_bpm.confirmed.is_none());
    }

    #[test]
    fn continuous_commands_coalesce_by_full_semantic_key() {
        let mut queue = SemanticCommandQueue::default();
        queue
            .push(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: 1.0,
            })
            .unwrap();
        queue
            .push(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: 2.0,
            })
            .unwrap();
        queue
            .push(SemanticCommand::SetRouteLevel {
                source: InputId::Input1,
                destination: MixDestination::Monitor1,
                normalized: 0.5,
            })
            .unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue.pop(),
            Some(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: 2.0
            })
        );
    }

    #[test]
    fn discrete_commands_are_never_coalesced() {
        let mut queue = SemanticCommandQueue::default();
        queue
            .push(SemanticCommand::SetMute {
                input: InputId::Input1,
                enabled: true,
            })
            .unwrap();
        queue
            .push(SemanticCommand::SetMute {
                input: InputId::Input1,
                enabled: false,
            })
            .unwrap();
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn snapshot_save_is_a_barrier_for_gain_coalescing() {
        let mut queue = SemanticCommandQueue::default();
        let gain = |db| SemanticCommand::SetGain {
            input: InputId::Input1,
            db,
        };
        let save = SemanticCommand::SaveSnapshot {
            slot: 0,
            name: "Before change".into(),
        };
        queue.push(gain(10.0)).unwrap();
        queue.push(save.clone()).unwrap();
        queue.push(gain(20.0)).unwrap();
        assert_eq!(queue.pop(), Some(gain(10.0)));
        assert_eq!(queue.pop(), Some(save));
        assert_eq!(queue.pop(), Some(gain(20.0)));
    }

    #[test]
    fn gain_pending_matches_the_queued_wire_value() {
        let mut store = Flow8Store::disconnected();
        store.state.session = SessionState::Ready;
        store
            .dispatch(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: -30.0,
            })
            .unwrap();
        assert_eq!(store.state.channels[0].gain_db.pending, Some(-30.0));
        assert_eq!(
            store.queue.pop(),
            Some(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: -30.0,
            })
        );
        assert_eq!(
            store.dispatch(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: -61.0,
            }),
            Err(CoreError::InvalidValue)
        );
        assert!(store.queue.is_empty());
    }

    #[test]
    fn oversized_device_name_is_rejected_before_pending_or_enqueue() {
        let mut store = Flow8Store::disconnected();
        assert_eq!(
            store.dispatch(SemanticCommand::SetSetting(KnownSetting::DeviceName(
                "界".repeat(86)
            ),)),
            Err(CoreError::InvalidValue)
        );
        assert!(store.queue.is_empty());
        assert!(store.state.routing.settings.device_name.pending.is_none());
    }

    #[test]
    fn nested_continuous_commands_coalesce_only_matching_parameter_identity() {
        let mut queue = SemanticCommandQueue::default();
        for gain_db in [1.0, 2.0] {
            queue
                .push(SemanticCommand::SetPeqBand {
                    input: InputId::Input1,
                    band: 0,
                    frequency_hz: 100,
                    q: 1.0,
                    gain_db,
                })
                .unwrap();
        }
        queue
            .push(SemanticCommand::SetPeqBand {
                input: InputId::Input1,
                band: 1,
                frequency_hz: 500,
                q: 1.0,
                gain_db: 3.0,
            })
            .unwrap();
        queue
            .push(SemanticCommand::SetGeqBand {
                bus: MixBusId::Main,
                band: 0,
                frequency_hz: 63,
                q: 1.0,
                gain_db: 4.0,
            })
            .unwrap();
        assert_eq!(queue.len(), 3);
        assert!(matches!(
            queue.pop(),
            Some(SemanticCommand::SetPeqBand {
                band: 0,
                gain_db: 2.0,
                ..
            })
        ));
    }

    #[test]
    fn simulator_keeps_bus_routes_independent() {
        let mut store = Flow8Store::simulator();
        store
            .dispatch(SemanticCommand::SetRouteLevel {
                source: InputId::Input1,
                destination: MixDestination::Monitor1,
                normalized: 0.2,
            })
            .unwrap();
        assert_eq!(
            store.state.channels[0].route_levels[MixDestination::Monitor1.index()].confirmed,
            Some(0.2)
        );
        assert_eq!(
            store.state.channels[0].route_levels[MixDestination::Monitor2.index()].confirmed,
            Some(0.72)
        );
    }

    #[test]
    fn device_observation_wins_over_pending() {
        let mut value = StateValue::confirmed(0.1_f32, EvidenceStatus::Synthetic);
        value.set_pending(0.9);
        value.observe(0.3, EvidenceStatus::VerifiedFromDevice);
        assert_eq!(value.effective(), Some(&0.3));
        assert!(value.pending.is_none());
    }

    #[test]
    fn unsupported_bt_usb_gain_is_rejected_by_semantic_layer() {
        let mut store = Flow8Store::simulator();
        assert_eq!(
            store.dispatch(SemanticCommand::SetGain {
                input: InputId::UsbBluetooth,
                db: 0.0
            }),
            Err(CoreError::UnsupportedCapability)
        );
    }

    fn input_state(id: u8, gain_db: f32) -> flow8_protocol::InputState {
        flow8_protocol::InputState {
            id,
            flags: 0,
            gain_db,
            compressor_amount: 0.0,
            high_pass_hz: 80,
            balance: 0.0,
            eq_gain_db: [0.0; 4],
            eq_frequency_hz: [80, 250, 2_000, 8_000],
            eq_q: [1.0; 4],
            label: flow8_protocol::ChannelLabel {
                endpoint: id,
                icon: 0,
                text: format!("Input {}", id + 1),
            },
        }
    }

    fn output_state(id: u8) -> flow8_protocol::OutputState {
        flow8_protocol::OutputState {
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

    fn fx_state(id: u8) -> flow8_protocol::FxState {
        flow8_protocol::FxState {
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

    fn mixer_state() -> flow8_protocol::MixerState {
        let mut inputs: [flow8_protocol::InputState; 7] =
            std::array::from_fn(|index| input_state(index as u8, index as f32));
        inputs[0].gain_db = 12.5;
        let mut flags = [false; 13];
        flags[7] = true;
        flow8_protocol::MixerState {
            inputs,
            outputs: [output_state(10), output_state(11), output_state(15)],
            effects: [fx_state(12), fx_state(13)],
            headphone_volume_db: 0.0,
            flags,
            raw_flags: 1 << 7,
            tempo_bpm: 135,
            selected_output: 15,
            last_snapshot: 0,
            monitor_routing: 0,
            snapshot_scope: 0,
        }
    }

    #[test]
    fn semantic_queue_item_encodes_only_after_dequeue() {
        let mut queue = SemanticCommandQueue::default();
        queue
            .push(SemanticCommand::SetGain {
                input: InputId::Input1,
                db: 0.0,
            })
            .unwrap();
        let command = queue.pop().expect("queued command");
        assert_eq!(
            flow8_protocol::encode(&command.to_protocol()).unwrap(),
            vec![0x02, 0x01, 0x00, 0x78, 0x7b]
        );
    }

    #[test]
    fn fx_preset_switch_uses_existing_codec_and_device_wins_on_readback() {
        let mut store = Flow8Store::disconnected();
        store.state.session = SessionState::Ready;
        store
            .dispatch(SemanticCommand::SetFxPreset {
                fx: FxId::Fx2,
                preset: 4,
            })
            .unwrap();
        assert_eq!(store.state.effects[1].preset.pending, Some(4));
        let command = store.queue.pop().expect("FX preset command");
        assert_eq!(
            flow8_protocol::encode(&command.to_protocol()).unwrap(),
            vec![0x31, 0x01, 0x0d, 0x04, 0x43]
        );
        store
            .apply_rx(
                RxCommand::FxPreset {
                    endpoint: 0x0d,
                    preset: 3,
                },
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
        assert_eq!(store.state.effects[1].preset.confirmed, Some(3));
        assert_eq!(store.state.effects[1].preset.pending, None);
    }

    #[test]
    fn device_rx_clears_conflicting_pending_value() {
        let mut store = Flow8Store::simulator();
        store.state.channels[0].gain_db.set_pending(10.0);
        store
            .apply_rx(
                RxCommand::InputState(input_state(0, 5.0)),
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
        assert_eq!(store.state.channels[0].gain_db.confirmed, Some(5.0));
        assert_eq!(store.state.channels[0].gain_db.pending, None);
        assert_eq!(
            store.state.channels[0].gain_db.evidence,
            EvidenceStatus::VerifiedFromDevice
        );
    }

    #[test]
    fn initial_device_snapshot_populates_unconfirmed_store_without_synthetic_fallback() {
        let mut store = Flow8Store::disconnected();
        assert_eq!(store.state.channels[0].gain_db.confirmed, None);
        store
            .apply_rx(
                RxCommand::MixerState(Box::new(mixer_state())),
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
        assert_eq!(store.state.channels[0].gain_db.confirmed, Some(12.5));
        assert_eq!(
            store.state.channels[0].gain_db.evidence,
            EvidenceStatus::VerifiedFromDevice
        );
        assert_eq!(store.state.effects[1].preset.confirmed, Some(0));
        assert_eq!(store.state.routing.settings.device_name.confirmed, None);
        assert_eq!(
            store.state.routing.settings.device_name.evidence,
            EvidenceStatus::Unknown
        );
    }

    #[test]
    fn composite_state_is_atomic_and_rejects_duplicate_endpoints() {
        let mut store = Flow8Store::simulator();
        store
            .apply_rx(
                RxCommand::MixerState(Box::new(mixer_state())),
                EvidenceStatus::VerifiedFromApk,
            )
            .unwrap();
        assert_eq!(store.state.channels[0].gain_db.confirmed, Some(12.5));
        assert_eq!(store.state.global_tempo_bpm.confirmed, Some(135.0));
        assert_eq!(
            store.state.routing.monitor_link.stereo_linked.confirmed,
            Some(true)
        );

        let before_gain = store.state.channels[0].gain_db.clone();
        let before_tempo = store.state.global_tempo_bpm.clone();
        let mut invalid = mixer_state();
        invalid.outputs[1].id = 10;
        assert_eq!(
            store.apply_rx(
                RxCommand::MixerState(Box::new(invalid)),
                EvidenceStatus::VerifiedFromApk,
            ),
            Err(CoreError::InvalidCompositeState)
        );
        assert_eq!(store.state.channels[0].gain_db, before_gain);
        assert_eq!(store.state.global_tempo_bpm, before_tempo);
    }

    #[test]
    fn composite_apply_does_not_bypass_the_runtime_ready_gate() {
        let mut store = Flow8Store::disconnected();
        store.state.session = SessionState::StateSyncing;
        store
            .apply_rx(
                RxCommand::MixerState(Box::new(mixer_state())),
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();

        assert_eq!(store.state.channels[0].gain_db.confirmed, Some(12.5));
        assert_eq!(store.state.session, SessionState::StateSyncing);
    }

    #[test]
    fn atomic_rx_reconciles_pending_and_updates_the_shared_state_tree() {
        let mut store = Flow8Store::disconnected();
        store.state.channels[0].gain_db.set_pending(12.0);
        store
            .apply_rx(
                RxCommand::Gain {
                    endpoint: 0,
                    db: 5.0,
                },
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
        store
            .apply_rx(
                RxCommand::RouteState {
                    endpoint_a: 0,
                    destination: 10,
                    level_db: -30.0,
                },
                EvidenceStatus::VerifiedFromApk,
            )
            .unwrap();
        store
            .apply_rx(
                RxCommand::FxSetup {
                    endpoint: 12,
                    values: [20, 1, 9],
                    route_flags: 0b101,
                },
                EvidenceStatus::VerifiedFromApk,
            )
            .unwrap();
        store
            .apply_rx(
                RxCommand::Setting {
                    id: 0x0b,
                    data: vec![1],
                },
                EvidenceStatus::VerifiedFromApk,
            )
            .unwrap();
        assert_eq!(store.state.channels[0].gain_db.confirmed, Some(5.0));
        assert!(store.state.channels[0].gain_db.pending.is_none());
        assert_eq!(
            store.state.channels[0].route_levels[MixDestination::Monitor1.index()].confirmed,
            Some(db_to_normalized(-30.0))
        );
        assert_eq!(store.state.effects[0].parameters[2].confirmed, Some(9));
        assert_eq!(store.state.effects[0].return_to_mon1.confirmed, Some(false));
        assert_eq!(
            store.state.routing.monitor_link.stereo_linked.confirmed,
            Some(true)
        );
    }

    #[test]
    fn meter_rx_is_isolated_from_control_state() {
        let mut store = Flow8Store::disconnected();
        let gain_before = store.state.channels[0].gain_db.clone();
        let mut meters = [-60.0; 15];
        meters[0] = -12.0;
        meters[10] = -3.0;
        store
            .apply_rx(
                RxCommand::MeterUpdate(flow8_protocol::MeterUpdate {
                    meters_db: meters,
                    gain_reduction_bits: 0b10,
                }),
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
        assert_eq!(
            store.state.channels[0].meter.level_db.confirmed,
            Some(-12.0)
        );
        assert_eq!(store.state.channels[0].gain_db, gain_before);
    }

    #[test]
    fn snapshot_and_reset_events_force_state_resynchronization() {
        let mut store = Flow8Store::disconnected();
        store.state.session = SessionState::Ready;
        store
            .apply_rx(
                RxCommand::SnapshotLoad { slot: 4 },
                EvidenceStatus::VerifiedFromApk,
            )
            .unwrap();
        assert_eq!(store.state.session, SessionState::StateSyncing);
        assert_eq!(store.state.snapshots.last_loaded.confirmed, Some(Some(4)));
        store.state.session = SessionState::Ready;
        store
            .apply_rx(RxCommand::FactoryReset, EvidenceStatus::VerifiedFromApk)
            .unwrap();
        assert_eq!(store.state.session, SessionState::StateSyncing);
    }

    #[test]
    fn deterministic_simulator_activity_remains_synthetic() {
        let mut store = Flow8Store::simulator();
        store.tick_simulator(8.1);
        assert_eq!(store.state.effects[0].parameters[0].confirmed, Some(17));
        assert_eq!(
            store.state.effects[0].parameters[0].evidence,
            EvidenceStatus::Synthetic
        );
        assert_eq!(store.state.snapshots.last_loaded.confirmed, Some(Some(1)));
    }
}
