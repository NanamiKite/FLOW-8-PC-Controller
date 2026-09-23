//! FLOW Mix APK-evidenced protocol primitives and first-stage command set.
//! No item in this crate claims acceptance by physical FLOW 8 hardware.

use std::collections::HashMap;

use flow8_model::{InputId, MixDestination};
use thiserror::Error;

pub const MAX_RAW_PACKET_SIZE: usize = 251;
pub const MAX_REASSEMBLED_PAYLOAD: usize = 499;
pub const TARGET_COMMAND_COUNT: usize = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Fix8Format {
    Pan = 1,
    UnitInterval = 2,
    FaderDb = 3,
    EqGainDb = 4,
    GainDb = 5,
    Q = 6,
}

fn round_ties_even(value: f32) -> i32 {
    let floor = value.floor();
    let fraction = value - floor;
    if fraction < 0.5 {
        floor as i32
    } else if fraction > 0.5 {
        floor as i32 + 1
    } else {
        let base = floor as i32;
        if base % 2 == 0 { base } else { base + 1 }
    }
}

pub fn normalized_to_fader_db(normalized: f32) -> Option<f32> {
    if !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {
        return None;
    }
    Some(if normalized >= 1.0 {
        10.0
    } else if normalized >= 0.5 {
        40.0 * normalized - 30.0
    } else if normalized >= 0.25 {
        80.0 * normalized - 50.0
    } else if normalized >= 0.0625 {
        160.0 * normalized - 70.0
    } else if normalized >= 1.0 / 1024.0 {
        480.0 * normalized - 90.0
    } else {
        -144.0
    })
}

pub fn route_level_table() -> [f32; 256] {
    let mut table = [0.0; 256];
    for (index, value) in table.iter_mut().enumerate() {
        *value = normalized_to_fader_db(index as f32 / 255.0).expect("table input is bounded");
    }
    table[191] = 0.0;
    table
}

pub fn encode_fix8(format: Fix8Format, value: f32) -> Option<u8> {
    if !value.is_finite() {
        return None;
    }
    match format {
        Fix8Format::Pan => Some((value.clamp(-1.0, 1.0) * 127.0 + 127.0).trunc() as u8),
        Fix8Format::UnitInterval => Some((value.clamp(0.0, 1.0) * 255.0).trunc() as u8),
        Fix8Format::FaderDb => {
            let table = route_level_table();
            for index in 1..table.len() {
                if table[index] > value {
                    return Some((index - 1) as u8);
                }
            }
            Some(255)
        }
        Fix8Format::EqGainDb => {
            let scaled = value.clamp(-15.0, 15.0) / 15.0 * 127.0 + 127.0;
            Some(round_ties_even(scaled) as u8)
        }
        Fix8Format::GainDb => {
            let scaled = (value.clamp(-60.0, 60.0) + 60.0) * 2.0;
            Some(round_ties_even(scaled) as u8)
        }
        Fix8Format::Q => {
            if value <= 0.0 {
                Some(0)
            } else if value <= 10.7 {
                Some((round_ties_even((value - 0.3) * 10.0) + 1).clamp(1, 101) as u8)
            } else {
                Some((round_ties_even(value - 11.0) + 102).clamp(102, 255) as u8)
            }
        }
    }
}

pub fn decode_fix8(format: Fix8Format, value: u8) -> f32 {
    match format {
        Fix8Format::Pan => (value as f32 - 127.0) / 127.0,
        Fix8Format::UnitInterval => value as f32 / 255.0,
        Fix8Format::FaderDb => route_level_table()[value as usize],
        Fix8Format::EqGainDb => (value as f32 - 127.0) / 127.0 * 15.0,
        Fix8Format::GainDb => value as f32 / 2.0 - 60.0,
        Fix8Format::Q if value == 0 => 0.0,
        Fix8Format::Q if value <= 101 => 0.3 + (value - 1) as f32 / 10.0,
        Fix8Format::Q => value as f32 - 91.0,
    }
}

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte))
}

pub fn frame_single(command: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.extend([command, 1]);
    frame.extend(payload);
    frame.push(checksum(&frame));
    frame
}

pub fn frame_command(
    command: u8,
    payload: &[u8],
    max_size: usize,
    sequence: u8,
) -> Result<Vec<Vec<u8>>, ProtocolError> {
    if max_size < 3 || payload.len() >= 500 {
        return Err(ProtocolError::PayloadTooLarge);
    }
    if payload.len() <= max_size - 3 {
        return Ok(vec![frame_single(command, payload)]);
    }
    if max_size < 6 {
        return Err(ProtocolError::PayloadTooLarge);
    }
    let chunk = max_size - 5;
    let count = payload.len().div_ceil(chunk);
    if !(2..34).contains(&count) {
        return Err(ProtocolError::TooManyFragments);
    }
    Ok(payload
        .chunks(chunk)
        .enumerate()
        .map(|(index, bytes)| {
            let mut frame = Vec::with_capacity(bytes.len() + 5);
            frame.extend([command, count as u8, sequence, index as u8]);
            frame.extend(bytes);
            frame.push(checksum(&frame));
            frame
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub command: u8,
    pub fragment_count: u8,
    pub sequence: Option<u8>,
    pub fragment_index: Option<u8>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("packet is too short")]
    TooShort,
    #[error("checksum mismatch")]
    ChecksumMismatch,
    #[error("invalid fragment metadata")]
    InvalidFragment,
    #[error("too many fragments")]
    TooManyFragments,
    #[error("payload is too large")]
    PayloadTooLarge,
    #[error("fragment sequence conflicts with buffered data")]
    ConflictingFragment,
    #[error("all four reassembly slots are occupied")]
    NoReassemblySlot,
    #[error("invalid endpoint relationship")]
    InvalidEndpoint,
    #[error("invalid or non-finite value")]
    InvalidValue,
    #[error("payload does not match command 0x{0:02x}")]
    InvalidPayload(u8),
    #[error("unsupported command 0x{0:02x}")]
    UnsupportedCommand(u8),
}

pub fn parse_packet(raw: &[u8]) -> Result<Packet, ProtocolError> {
    if raw.len() < 3 {
        return Err(ProtocolError::TooShort);
    }
    if checksum(&raw[..raw.len() - 1]) != raw[raw.len() - 1] {
        return Err(ProtocolError::ChecksumMismatch);
    }
    let count = raw[1];
    if count == 0 {
        return Err(ProtocolError::InvalidFragment);
    }
    if count >= 34 {
        return Err(ProtocolError::TooManyFragments);
    }
    if count == 1 {
        Ok(Packet {
            command: raw[0],
            fragment_count: 1,
            sequence: None,
            fragment_index: None,
            payload: raw[2..raw.len() - 1].to_vec(),
        })
    } else {
        if raw.len() < 5 || raw[3] >= count {
            return Err(ProtocolError::InvalidFragment);
        }
        Ok(Packet {
            command: raw[0],
            fragment_count: count,
            sequence: Some(raw[2]),
            fragment_index: Some(raw[3]),
            payload: raw[4..raw.len() - 1].to_vec(),
        })
    }
}

#[derive(Debug, Clone)]
struct ReassemblySlot {
    fragment_count: u8,
    fragments: Vec<Option<Vec<u8>>>,
}

#[derive(Debug, Default)]
pub struct FragmentReassembler {
    slots: HashMap<(u8, u8), ReassemblySlot>,
}

impl FragmentReassembler {
    pub fn accept(&mut self, raw: &[u8]) -> Result<Option<(u8, Vec<u8>)>, ProtocolError> {
        let packet = parse_packet(raw)?;
        if packet.fragment_count == 1 {
            return Ok(Some((packet.command, packet.payload)));
        }
        let sequence = packet.sequence.ok_or(ProtocolError::InvalidFragment)?;
        let index = packet
            .fragment_index
            .ok_or(ProtocolError::InvalidFragment)? as usize;
        let key = (packet.command, sequence);
        if !self.slots.contains_key(&key) && self.slots.len() >= 4 {
            return Err(ProtocolError::NoReassemblySlot);
        }
        let slot = self.slots.entry(key).or_insert_with(|| ReassemblySlot {
            fragment_count: packet.fragment_count,
            fragments: vec![None; packet.fragment_count as usize],
        });
        if slot.fragment_count != packet.fragment_count {
            return Err(ProtocolError::ConflictingFragment);
        }
        if let Some(previous) = &slot.fragments[index] {
            if previous != &packet.payload {
                return Err(ProtocolError::ConflictingFragment);
            }
        } else {
            slot.fragments[index] = Some(packet.payload);
        }
        let size: usize = slot
            .fragments
            .iter()
            .filter_map(Option::as_ref)
            .map(Vec::len)
            .sum();
        if size > MAX_REASSEMBLED_PAYLOAD {
            self.slots.remove(&key);
            return Err(ProtocolError::PayloadTooLarge);
        }
        if slot.fragments.iter().any(Option::is_none) {
            return Ok(None);
        }
        let mut payload = Vec::with_capacity(size);
        for fragment in &slot.fragments {
            payload.extend(fragment.as_ref().expect("complete slot"));
        }
        self.slots.remove(&key);
        Ok(Some((packet.command, payload)))
    }

    pub fn pending_count(&self) -> usize {
        self.slots.len()
    }
    pub fn clear(&mut self) {
        self.slots.clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TxCommand {
    Pan {
        input: InputId,
        value: f32,
    },
    EndpointPan {
        endpoint: u8,
        value: f32,
    },
    Solo {
        input: InputId,
        enabled: bool,
    },
    Gain {
        input: InputId,
        db: f32,
    },
    RouteLevel {
        source: InputId,
        destination: MixDestination,
        normalized: f32,
    },
    DestinationMaster {
        destination: MixDestination,
        normalized: f32,
    },
    Mute {
        endpoint: u8,
        enabled: bool,
    },
    Phase {
        input: InputId,
        inverted: bool,
    },
    Phantom {
        input: InputId,
        enabled: bool,
    },
    GraphicEq {
        endpoint: u8,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
    },
    HighPassFilter {
        input: InputId,
        enabled: bool,
        frequency_hz: u16,
    },
    Label(ChannelLabel),
    ParametricEq {
        input: InputId,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
    },
    FxSetup {
        endpoint: u8,
        values: [u8; 3],
        route_flags: u8,
    },
    SnapshotDelete {
        slot: u8,
    },
    Compressor {
        input: InputId,
        amount: f32,
    },
    Limiter {
        endpoint: u8,
        threshold_db: f32,
    },
    GetChannelState {
        endpoint: u8,
    },
    InputState(InputState),
    OutputState(OutputState),
    SnapshotSave {
        slot: u8,
        name: String,
    },
    SnapshotLoad {
        slot: u8,
    },
    MeterRequest(MeterRequest),
    MeterUpdate(MeterUpdate),
    Setting {
        id: u8,
        data: Vec<u8>,
    },
    FactoryReset,
    FxState(FxState),
    FxPreset {
        endpoint: u8,
        preset: u8,
    },
    ChannelConnection {
        endpoint: u8,
        subchannel: u8,
        connected: bool,
    },
    MixerState(Box<MixerState>),
    FxTempo {
        bpm: u16,
    },
    SelectOutput {
        endpoint: u8,
    },
    ChannelDelay {
        endpoint: u8,
        ticks: u32,
    },
    GetSnapshotNames,
    GetChannelLabels,
    GetSetting {
        id: u8,
    },
    SnapshotRename {
        slot: u8,
        name: String,
    },
    /// Diagnostic-only emission of the APK-confirmed device-to-client
    /// HandshakeHost schema. Normal session code never sends this command.
    HandshakeHostProbe {
        device_id: [u8; 16],
        pairing_any: bool,
        protocol_version: u16,
        firmware_build: u16,
    },
    /// Diagnostic-only emission of the empty APK-confirmed HandshakeReply
    /// schema. Normal session code only receives this command.
    HandshakeReplyProbe,
    GetMixerState,
    HandshakeClient {
        client_id: [u8; 16],
    },
}

pub fn encode(command: &TxCommand) -> Result<Vec<u8>, ProtocolError> {
    let frames = encode_frames(command, MAX_RAW_PACKET_SIZE, 0)?;
    if frames.len() == 1 {
        Ok(frames.into_iter().next().expect("one encoded frame"))
    } else {
        Err(ProtocolError::PayloadTooLarge)
    }
}

pub fn encode_frames(
    command: &TxCommand,
    max_raw_packet_size: usize,
    sequence: u8,
) -> Result<Vec<Vec<u8>>, ProtocolError> {
    let (id, payload) = serialize_payload(command)?;
    frame_command(id, &payload, max_raw_packet_size, sequence)
}

fn serialize_payload(command: &TxCommand) -> Result<(u8, Vec<u8>), ProtocolError> {
    let (id, payload) = match command {
        TxCommand::Pan { input, value } => (
            0x00,
            vec![
                input.endpoint(),
                encode_fix8(Fix8Format::Pan, *value).ok_or(ProtocolError::InvalidValue)?,
            ],
        ),
        TxCommand::EndpointPan { endpoint, value } => (
            0x00,
            vec![
                *endpoint,
                encode_fix8(Fix8Format::Pan, *value).ok_or(ProtocolError::InvalidValue)?,
            ],
        ),
        TxCommand::Solo { input, enabled } => (0x01, vec![input.endpoint(), *enabled as u8]),
        TxCommand::Gain { input, db } => (
            0x02,
            vec![
                input.endpoint(),
                encode_fix8(Fix8Format::GainDb, *db).ok_or(ProtocolError::InvalidValue)?,
            ],
        ),
        TxCommand::RouteLevel {
            source,
            destination,
            normalized,
        } => {
            let db = normalized_to_fader_db(*normalized).ok_or(ProtocolError::InvalidValue)?;
            (
                0x06,
                vec![
                    source.endpoint(),
                    destination.endpoint(),
                    encode_fix8(Fix8Format::FaderDb, db).ok_or(ProtocolError::InvalidValue)?,
                ],
            )
        }
        TxCommand::DestinationMaster {
            destination,
            normalized,
        } => {
            let db = normalized_to_fader_db(*normalized).ok_or(ProtocolError::InvalidValue)?;
            let endpoint = destination.endpoint();
            (
                0x06,
                vec![
                    endpoint,
                    endpoint,
                    encode_fix8(Fix8Format::FaderDb, db).ok_or(ProtocolError::InvalidValue)?,
                ],
            )
        }
        TxCommand::Mute { endpoint, enabled } => (0x08, vec![*endpoint, *enabled as u8]),
        TxCommand::Phase { input, inverted } => (0x10, vec![input.endpoint(), *inverted as u8]),
        TxCommand::Phantom { input, enabled } => {
            if !input.has_phantom() {
                return Err(ProtocolError::InvalidEndpoint);
            }
            (0x15, vec![input.endpoint(), *enabled as u8])
        }
        TxCommand::GraphicEq {
            endpoint,
            band,
            frequency_hz,
            q,
            gain_db,
        } => (
            0x03,
            eq_payload(*endpoint, *band, *frequency_hz, *q, *gain_db)?,
        ),
        TxCommand::HighPassFilter {
            input,
            enabled,
            frequency_hz,
        } => {
            if !input.has_analog_gain() {
                return Err(ProtocolError::InvalidEndpoint);
            }
            let mut payload = vec![input.endpoint(), *enabled as u8];
            payload.extend(frequency_hz.to_be_bytes());
            (0x04, payload)
        }
        TxCommand::Label(label) => (0x05, encode_label(label)?),
        TxCommand::ParametricEq {
            input,
            band,
            frequency_hz,
            q,
            gain_db,
        } => (
            0x09,
            eq_payload(input.endpoint(), *band, *frequency_hz, *q, *gain_db)?,
        ),
        TxCommand::FxSetup {
            endpoint,
            values,
            route_flags,
        } => (
            0x11,
            vec![*endpoint, values[0], values[1], values[2], *route_flags],
        ),
        TxCommand::SnapshotDelete { slot } => (0x12, vec![*slot]),
        TxCommand::Compressor { input, amount } => (
            0x13,
            vec![
                input.endpoint(),
                encode_fix8(Fix8Format::UnitInterval, *amount)
                    .ok_or(ProtocolError::InvalidValue)?,
            ],
        ),
        TxCommand::Limiter {
            endpoint,
            threshold_db,
        } => (
            0x14,
            vec![
                *endpoint,
                encode_fix8(Fix8Format::GainDb, *threshold_db)
                    .ok_or(ProtocolError::InvalidValue)?,
            ],
        ),
        TxCommand::GetChannelState { endpoint } => (0x16, vec![*endpoint]),
        TxCommand::InputState(state) => (0x17, encode_input_state(state)?),
        TxCommand::OutputState(state) => (0x18, encode_output_state(state)?),
        TxCommand::SnapshotSave { slot, name } => {
            let mut payload = vec![*slot];
            write_string(&mut payload, name.as_bytes(), 20)?;
            (0x19, payload)
        }
        TxCommand::SnapshotLoad { slot } => (0x20, vec![*slot]),
        TxCommand::MeterRequest(request) => {
            if request.count as usize > request.channel_codes.len() {
                return Err(ProtocolError::InvalidValue);
            }
            let mut payload = Vec::with_capacity(16);
            payload.push(request.count);
            payload.extend(request.channel_codes);
            (0x21, payload)
        }
        TxCommand::MeterUpdate(update) => (0x22, encode_meter_update(update)?),
        TxCommand::Setting { id, data } => {
            let mut payload = vec![*id];
            write_bytes(&mut payload, data, u8::MAX as usize)?;
            (0x25, payload)
        }
        TxCommand::FactoryReset => (0x29, Vec::new()),
        TxCommand::FxState(state) => (0x30, encode_fx_state(state)?),
        TxCommand::FxPreset { endpoint, preset } => (0x31, vec![*endpoint, *preset]),
        TxCommand::ChannelConnection {
            endpoint,
            subchannel,
            connected,
        } => (0x33, vec![*endpoint, *subchannel, *connected as u8]),
        TxCommand::MixerState(state) => (0x38, encode_mixer_state(state)?),
        TxCommand::FxTempo { bpm } => (0x40, bpm.to_be_bytes().to_vec()),
        TxCommand::SelectOutput { endpoint } => (0x41, vec![*endpoint]),
        TxCommand::ChannelDelay { endpoint, ticks } => {
            let mut payload = vec![*endpoint];
            payload.extend(ticks.to_be_bytes());
            (0x4a, payload)
        }
        TxCommand::GetSnapshotNames => (0x07, Vec::new()),
        TxCommand::GetChannelLabels => (0x23, Vec::new()),
        TxCommand::GetSetting { id } => (0x26, vec![*id]),
        TxCommand::SnapshotRename { slot, name } => {
            let mut payload = vec![*slot];
            write_string(&mut payload, name.as_bytes(), 20)?;
            (0x32, payload)
        }
        TxCommand::HandshakeHostProbe {
            device_id,
            pairing_any,
            protocol_version,
            firmware_build,
        } => {
            let mut payload = device_id.to_vec();
            payload.push(*pairing_any as u8);
            payload.extend(protocol_version.to_be_bytes());
            payload.extend(firmware_build.to_be_bytes());
            (0x35, payload)
        }
        TxCommand::HandshakeReplyProbe => (0x36, Vec::new()),
        TxCommand::GetMixerState => (0x37, Vec::new()),
        TxCommand::HandshakeClient { client_id } => (0x39, client_id.to_vec()),
    };
    Ok((id, payload))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelLabel {
    pub endpoint: u8,
    pub icon: u16,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputState {
    pub id: u8,
    pub flags: u8,
    pub gain_db: f32,
    pub compressor_amount: f32,
    pub high_pass_hz: u16,
    pub balance: f32,
    pub eq_gain_db: [f32; 4],
    pub eq_frequency_hz: [u16; 4],
    pub eq_q: [f32; 4],
    pub label: ChannelLabel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputState {
    pub id: u8,
    pub flags: u8,
    pub volume_db: f32,
    pub pan: f32,
    pub limiter_db: f32,
    pub input_gains_db: [f32; 7],
    pub eq_gain_db: [f32; 9],
    pub eq_frequency_hz: [u16; 9],
    pub eq_q: [f32; 9],
    pub delay_ticks: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FxState {
    pub id: u8,
    pub flags: u8,
    pub volume_db: f32,
    pub pan: f32,
    pub values: [u8; 3],
    pub preset: u8,
    pub input_gains_db: [f32; 7],
    pub aux_gains_db: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct MixerState {
    pub inputs: [InputState; 7],
    pub outputs: [OutputState; 3],
    pub effects: [FxState; 2],
    pub headphone_volume_db: f32,
    pub flags: [bool; 13],
    pub tempo_bpm: u16,
    pub selected_output: u8,
    pub last_snapshot: u8,
    pub monitor_routing: u8,
    pub snapshot_scope: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeterRequest {
    pub count: u8,
    pub channel_codes: [u8; 15],
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeterUpdate {
    pub meters_db: [f32; 15],
    pub gain_reduction_bits: u16,
}

fn write_bytes(target: &mut Vec<u8>, bytes: &[u8], maximum: usize) -> Result<(), ProtocolError> {
    if bytes.len() > maximum || bytes.len() > u8::MAX as usize {
        return Err(ProtocolError::InvalidValue);
    }
    target.push(bytes.len() as u8);
    target.extend(bytes);
    Ok(())
}

fn write_string(target: &mut Vec<u8>, text: &[u8], maximum: usize) -> Result<(), ProtocolError> {
    write_bytes(target, text, maximum)
}

fn encode_label(label: &ChannelLabel) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = vec![label.endpoint];
    payload.extend(label.icon.to_be_bytes());
    write_string(&mut payload, label.text.as_bytes(), 20)?;
    Ok(payload)
}

fn eq_payload(
    endpoint: u8,
    band: u8,
    frequency_hz: u16,
    q: f32,
    gain_db: f32,
) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = vec![endpoint, band];
    payload.extend(frequency_hz.to_be_bytes());
    payload.push(encode_fix8(Fix8Format::Q, q).ok_or(ProtocolError::InvalidValue)?);
    payload.push(encode_fix8(Fix8Format::EqGainDb, gain_db).ok_or(ProtocolError::InvalidValue)?);
    Ok(payload)
}

fn encode_input_state(state: &InputState) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = vec![
        state.id,
        state.flags,
        encode_fix8(Fix8Format::GainDb, state.gain_db).ok_or(ProtocolError::InvalidValue)?,
        encode_fix8(Fix8Format::UnitInterval, state.compressor_amount)
            .ok_or(ProtocolError::InvalidValue)?,
    ];
    payload.extend(state.high_pass_hz.to_be_bytes());
    payload.push(encode_fix8(Fix8Format::Pan, state.balance).ok_or(ProtocolError::InvalidValue)?);
    for value in state.eq_gain_db {
        payload.push(encode_fix8(Fix8Format::EqGainDb, value).ok_or(ProtocolError::InvalidValue)?);
    }
    for value in state.eq_frequency_hz {
        payload.extend(value.to_be_bytes());
    }
    for value in state.eq_q {
        payload.push(encode_fix8(Fix8Format::Q, value).ok_or(ProtocolError::InvalidValue)?);
    }
    payload.extend(encode_label(&state.label)?);
    Ok(payload)
}

fn encode_output_state(state: &OutputState) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = vec![
        state.id,
        state.flags,
        encode_fix8(Fix8Format::FaderDb, state.volume_db).ok_or(ProtocolError::InvalidValue)?,
        encode_fix8(Fix8Format::Pan, state.pan).ok_or(ProtocolError::InvalidValue)?,
        encode_fix8(Fix8Format::GainDb, state.limiter_db).ok_or(ProtocolError::InvalidValue)?,
    ];
    for value in state.input_gains_db {
        payload.push(encode_fix8(Fix8Format::FaderDb, value).ok_or(ProtocolError::InvalidValue)?);
    }
    for value in state.eq_gain_db {
        payload.push(encode_fix8(Fix8Format::EqGainDb, value).ok_or(ProtocolError::InvalidValue)?);
    }
    for value in state.eq_frequency_hz {
        payload.extend(value.to_be_bytes());
    }
    for value in state.eq_q {
        payload.push(encode_fix8(Fix8Format::Q, value).ok_or(ProtocolError::InvalidValue)?);
    }
    payload.extend(state.delay_ticks.to_be_bytes());
    Ok(payload)
}

fn encode_fx_state(state: &FxState) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = vec![
        state.id,
        state.flags,
        encode_fix8(Fix8Format::FaderDb, state.volume_db).ok_or(ProtocolError::InvalidValue)?,
        encode_fix8(Fix8Format::Pan, state.pan).ok_or(ProtocolError::InvalidValue)?,
        state.values[0],
        state.values[1],
        state.values[2],
        state.preset,
    ];
    for value in state.input_gains_db {
        payload.push(encode_fix8(Fix8Format::FaderDb, value).ok_or(ProtocolError::InvalidValue)?);
    }
    for value in state.aux_gains_db {
        payload.push(encode_fix8(Fix8Format::FaderDb, value).ok_or(ProtocolError::InvalidValue)?);
    }
    Ok(payload)
}

fn encode_meter_update(update: &MeterUpdate) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = Vec::with_capacity(17);
    for value in update.meters_db {
        payload.push(encode_fix8(Fix8Format::FaderDb, value).ok_or(ProtocolError::InvalidValue)?);
    }
    payload.extend(update.gain_reduction_bits.to_be_bytes());
    Ok(payload)
}

fn encode_mixer_state(state: &MixerState) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = Vec::new();
    for input in &state.inputs {
        payload.extend(encode_input_state(input)?);
    }
    for output in &state.outputs {
        payload.extend(encode_output_state(output)?);
    }
    for effect in &state.effects {
        payload.extend(encode_fx_state(effect)?);
    }
    payload.push(
        encode_fix8(Fix8Format::FaderDb, state.headphone_volume_db)
            .ok_or(ProtocolError::InvalidValue)?,
    );
    let flags = state
        .flags
        .iter()
        .enumerate()
        .fold(0_u16, |bits, (index, enabled)| {
            bits | ((*enabled as u16) << index)
        });
    payload.extend(flags.to_le_bytes());
    payload.extend(state.tempo_bpm.to_be_bytes());
    payload.extend([
        state.selected_output,
        state.last_snapshot,
        state.monitor_routing,
        state.snapshot_scope,
    ]);
    Ok(payload)
}

#[derive(Debug, Clone, PartialEq)]
pub enum RxCommand {
    Pan {
        endpoint: u8,
        value: f32,
    },
    Solo {
        endpoint: u8,
        enabled: bool,
    },
    Gain {
        endpoint: u8,
        db: f32,
    },
    GraphicEq {
        endpoint: u8,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
    },
    HighPassFilter {
        endpoint: u8,
        enabled: bool,
        frequency_hz: u16,
    },
    Label(ChannelLabel),
    RouteState {
        endpoint_a: u8,
        destination: u8,
        level_db: f32,
    },
    Mute {
        endpoint: u8,
        enabled: bool,
    },
    ParametricEq {
        endpoint: u8,
        band: u8,
        frequency_hz: u16,
        q: f32,
        gain_db: f32,
    },
    Phase {
        endpoint: u8,
        inverted: bool,
    },
    FxSetup {
        endpoint: u8,
        values: [u8; 3],
        route_flags: u8,
    },
    SnapshotDelete {
        slot: u8,
    },
    Compressor {
        endpoint: u8,
        amount: f32,
    },
    Limiter {
        endpoint: u8,
        threshold_db: f32,
    },
    Phantom {
        endpoint: u8,
        enabled: bool,
    },
    GetChannelState {
        endpoint: u8,
    },
    InputState(InputState),
    OutputState(OutputState),
    SnapshotSave {
        slot: u8,
        name: String,
    },
    SnapshotLoad {
        slot: u8,
    },
    MeterRequest(MeterRequest),
    MeterUpdate(MeterUpdate),
    Setting {
        id: u8,
        data: Vec<u8>,
    },
    FactoryReset,
    FxState(FxState),
    FxPreset {
        endpoint: u8,
        preset: u8,
    },
    ChannelConnection {
        endpoint: u8,
        subchannel: u8,
        connected: bool,
    },
    MixerState(Box<MixerState>),
    FxTempo {
        bpm: u16,
    },
    SelectOutput {
        endpoint: u8,
    },
    ChannelDelay {
        endpoint: u8,
        ticks: u32,
    },
    GetSnapshotNames,
    GetChannelLabels,
    ChannelLabels(Vec<ChannelLabel>),
    GetSetting {
        id: u8,
    },
    SnapshotNames(Vec<String>),
    SnapshotRename {
        slot: u8,
        name: String,
    },
    HandshakeHost {
        device_id: [u8; 16],
        pairing_any: bool,
        protocol_version: u16,
        firmware_build: u16,
    },
    HandshakeReply,
    GetMixerState,
    HandshakeClient {
        client_id: [u8; 16],
    },
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn u8(&mut self) -> Option<u8> {
        let value = *self.bytes.get(self.offset)?;
        self.offset += 1;
        Some(value)
    }
    fn u16be(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes([self.u8()?, self.u8()?]))
    }
    fn u32be(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.u8()?,
            self.u8()?,
            self.u8()?,
            self.u8()?,
        ]))
    }
    fn fix8(&mut self, format: Fix8Format) -> Option<f32> {
        Some(decode_fix8(format, self.u8()?))
    }
    fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(length)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(value)
    }
    fn length_prefixed(&mut self, maximum: usize) -> Option<Vec<u8>> {
        let length = self.u8()? as usize;
        if length > maximum {
            return None;
        }
        Some(self.bytes(length)?.to_vec())
    }
    fn label(&mut self) -> Option<ChannelLabel> {
        let endpoint = self.u8()?;
        let icon = self.u16be()?;
        let text = String::from_utf8_lossy(&self.length_prefixed(20)?).into_owned();
        Some(ChannelLabel {
            endpoint,
            icon,
            text,
        })
    }
    fn done(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn parse_input(reader: &mut Reader<'_>) -> Option<InputState> {
    let id = reader.u8()?;
    let flags = reader.u8()?;
    let gain_db = reader.fix8(Fix8Format::GainDb)?;
    let compressor_amount = reader.fix8(Fix8Format::UnitInterval)?;
    let high_pass_hz = reader.u16be()?;
    let balance = reader.fix8(Fix8Format::Pan)?;
    let mut eq_gain_db = [0.0; 4];
    for value in &mut eq_gain_db {
        *value = reader.fix8(Fix8Format::EqGainDb)?;
    }
    let mut eq_frequency_hz = [0; 4];
    for value in &mut eq_frequency_hz {
        *value = reader.u16be()?;
    }
    let mut eq_q = [0.0; 4];
    for value in &mut eq_q {
        *value = reader.fix8(Fix8Format::Q)?;
    }
    let label = reader.label()?;
    Some(InputState {
        id,
        flags,
        gain_db,
        compressor_amount,
        high_pass_hz,
        balance,
        eq_gain_db,
        eq_frequency_hz,
        eq_q,
        label,
    })
}

fn parse_output(reader: &mut Reader<'_>) -> Option<OutputState> {
    let id = reader.u8()?;
    let flags = reader.u8()?;
    let volume_db = reader.fix8(Fix8Format::FaderDb)?;
    let pan = reader.fix8(Fix8Format::Pan)?;
    let limiter_db = reader.fix8(Fix8Format::GainDb)?;
    let mut input_gains_db = [0.0; 7];
    for value in &mut input_gains_db {
        *value = reader.fix8(Fix8Format::FaderDb)?;
    }
    let mut eq_gain_db = [0.0; 9];
    for value in &mut eq_gain_db {
        *value = reader.fix8(Fix8Format::EqGainDb)?;
    }
    let mut eq_frequency_hz = [0; 9];
    for value in &mut eq_frequency_hz {
        *value = reader.u16be()?;
    }
    let mut eq_q = [0.0; 9];
    for value in &mut eq_q {
        *value = reader.fix8(Fix8Format::Q)?;
    }
    let delay_ticks = reader.u32be()?;
    Some(OutputState {
        id,
        flags,
        volume_db,
        pan,
        limiter_db,
        input_gains_db,
        eq_gain_db,
        eq_frequency_hz,
        eq_q,
        delay_ticks,
    })
}

fn parse_fx(reader: &mut Reader<'_>) -> Option<FxState> {
    let id = reader.u8()?;
    let flags = reader.u8()?;
    let volume_db = reader.fix8(Fix8Format::FaderDb)?;
    let pan = reader.fix8(Fix8Format::Pan)?;
    let values = [reader.u8()?, reader.u8()?, reader.u8()?];
    let preset = reader.u8()?;
    let mut input_gains_db = [0.0; 7];
    for value in &mut input_gains_db {
        *value = reader.fix8(Fix8Format::FaderDb)?;
    }
    let mut aux_gains_db = [0.0; 2];
    for value in &mut aux_gains_db {
        *value = reader.fix8(Fix8Format::FaderDb)?;
    }
    Some(FxState {
        id,
        flags,
        volume_db,
        pan,
        values,
        preset,
        input_gains_db,
        aux_gains_db,
    })
}

pub fn decode_payload(command: u8, payload: &[u8]) -> Result<RxCommand, ProtocolError> {
    let mut reader = Reader::new(payload);
    let decoded = match command {
        0x00 => RxCommand::Pan {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            value: reader
                .fix8(Fix8Format::Pan)
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x01 => RxCommand::Solo {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            enabled: read_bool(&mut reader, command)?,
        },
        0x02 => RxCommand::Gain {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            db: reader
                .fix8(Fix8Format::GainDb)
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x03 | 0x09 => {
            let endpoint = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            let band = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            let frequency_hz = reader
                .u16be()
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let q = reader
                .fix8(Fix8Format::Q)
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let gain_db = reader
                .fix8(Fix8Format::EqGainDb)
                .ok_or(ProtocolError::InvalidPayload(command))?;
            if command == 0x03 {
                RxCommand::GraphicEq {
                    endpoint,
                    band,
                    frequency_hz,
                    q,
                    gain_db,
                }
            } else {
                RxCommand::ParametricEq {
                    endpoint,
                    band,
                    frequency_hz,
                    q,
                    gain_db,
                }
            }
        }
        0x04 => RxCommand::HighPassFilter {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            enabled: read_bool(&mut reader, command)?,
            frequency_hz: reader
                .u16be()
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x05 => RxCommand::Label(
            reader
                .label()
                .ok_or(ProtocolError::InvalidPayload(command))?,
        ),
        0x06 => RxCommand::RouteState {
            endpoint_a: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            destination: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            level_db: reader
                .fix8(Fix8Format::FaderDb)
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x07 => RxCommand::GetSnapshotNames,
        0x08 => RxCommand::Mute {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            enabled: read_bool(&mut reader, command)?,
        },
        0x10 => RxCommand::Phase {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            inverted: read_bool(&mut reader, command)?,
        },
        0x11 => RxCommand::FxSetup {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            values: [
                reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
                reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
                reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            ],
            route_flags: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x12 => RxCommand::SnapshotDelete {
            slot: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x13 => RxCommand::Compressor {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            amount: reader
                .fix8(Fix8Format::UnitInterval)
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x14 => RxCommand::Limiter {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            threshold_db: reader
                .fix8(Fix8Format::GainDb)
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x15 => RxCommand::Phantom {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            enabled: read_bool(&mut reader, command)?,
        },
        0x16 => RxCommand::GetChannelState {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x17 => RxCommand::InputState(
            parse_input(&mut reader).ok_or(ProtocolError::InvalidPayload(command))?,
        ),
        0x18 => RxCommand::OutputState(
            parse_output(&mut reader).ok_or(ProtocolError::InvalidPayload(command))?,
        ),
        0x19 => RxCommand::SnapshotSave {
            slot: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            name: String::from_utf8_lossy(
                &reader
                    .length_prefixed(20)
                    .ok_or(ProtocolError::InvalidPayload(command))?,
            )
            .into_owned(),
        },
        0x20 => RxCommand::SnapshotLoad {
            slot: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x21 => {
            let count = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            let channel_codes: [u8; 15] = reader
                .bytes(15)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(ProtocolError::InvalidPayload(command))?;
            if count as usize > channel_codes.len() {
                return Err(ProtocolError::InvalidPayload(command));
            }
            RxCommand::MeterRequest(MeterRequest {
                count,
                channel_codes,
            })
        }
        0x22 => {
            let mut meters_db = [0.0; 15];
            for meter in &mut meters_db {
                *meter = reader
                    .fix8(Fix8Format::FaderDb)
                    .ok_or(ProtocolError::InvalidPayload(command))?;
            }
            RxCommand::MeterUpdate(MeterUpdate {
                meters_db,
                gain_reduction_bits: reader
                    .u16be()
                    .ok_or(ProtocolError::InvalidPayload(command))?,
            })
        }
        0x23 => RxCommand::GetChannelLabels,
        0x24 => {
            let count = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            if count > 10 {
                return Err(ProtocolError::InvalidPayload(command));
            }
            let mut labels = Vec::with_capacity(10);
            for _ in 0..10 {
                labels.push(
                    reader
                        .label()
                        .ok_or(ProtocolError::InvalidPayload(command))?,
                );
            }
            RxCommand::ChannelLabels(labels)
        }
        0x25 => RxCommand::Setting {
            id: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            data: reader
                .length_prefixed(u8::MAX as usize)
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x26 => RxCommand::GetSetting {
            id: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x27 => {
            let mut names = Vec::with_capacity(15);
            for _ in 0..15 {
                names.push(
                    String::from_utf8_lossy(
                        &reader
                            .length_prefixed(20)
                            .ok_or(ProtocolError::InvalidPayload(command))?,
                    )
                    .into_owned(),
                );
            }
            RxCommand::SnapshotNames(names)
        }
        0x29 => RxCommand::FactoryReset,
        0x30 => {
            RxCommand::FxState(parse_fx(&mut reader).ok_or(ProtocolError::InvalidPayload(command))?)
        }
        0x31 => RxCommand::FxPreset {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            preset: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x32 => RxCommand::SnapshotRename {
            slot: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            name: String::from_utf8_lossy(
                &reader
                    .length_prefixed(20)
                    .ok_or(ProtocolError::InvalidPayload(command))?,
            )
            .into_owned(),
        },
        0x33 => RxCommand::ChannelConnection {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            subchannel: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            connected: read_bool(&mut reader, command)?,
        },
        0x35 => {
            let device_id: [u8; 16] = reader
                .bytes(16)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(ProtocolError::InvalidPayload(command))?;
            RxCommand::HandshakeHost {
                device_id,
                pairing_any: read_bool(&mut reader, command)?,
                protocol_version: reader
                    .u16be()
                    .ok_or(ProtocolError::InvalidPayload(command))?,
                firmware_build: reader
                    .u16be()
                    .ok_or(ProtocolError::InvalidPayload(command))?,
            }
        }
        0x36 => RxCommand::HandshakeReply,
        0x37 => RxCommand::GetMixerState,
        0x38 => {
            let inputs: [InputState; 7] = (0..7)
                .map(|_| parse_input(&mut reader))
                .collect::<Option<Vec<_>>>()
                .and_then(|values| values.try_into().ok())
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let outputs: [OutputState; 3] = (0..3)
                .map(|_| parse_output(&mut reader))
                .collect::<Option<Vec<_>>>()
                .and_then(|values| values.try_into().ok())
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let effects: [FxState; 2] = (0..2)
                .map(|_| parse_fx(&mut reader))
                .collect::<Option<Vec<_>>>()
                .and_then(|values| values.try_into().ok())
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let headphone_volume_db = reader
                .fix8(Fix8Format::FaderDb)
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let flags_low = reader.u8().ok_or(ProtocolError::InvalidPayload(command))? as u16;
            let flags_high = reader.u8().ok_or(ProtocolError::InvalidPayload(command))? as u16;
            let bits = flags_low | flags_high << 8;
            let flags = std::array::from_fn(|index| bits & (1 << index) != 0);
            let tempo_bpm = reader
                .u16be()
                .ok_or(ProtocolError::InvalidPayload(command))?;
            let selected_output = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            let last_snapshot = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            let monitor_routing = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            let snapshot_scope = reader.u8().ok_or(ProtocolError::InvalidPayload(command))?;
            RxCommand::MixerState(Box::new(MixerState {
                inputs,
                outputs,
                effects,
                headphone_volume_db,
                flags,
                tempo_bpm,
                selected_output,
                last_snapshot,
                monitor_routing,
                snapshot_scope,
            }))
        }
        0x39 => {
            let client_id: [u8; 16] = reader
                .bytes(16)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(ProtocolError::InvalidPayload(command))?;
            RxCommand::HandshakeClient { client_id }
        }
        0x40 => RxCommand::FxTempo {
            bpm: reader
                .u16be()
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x41 => RxCommand::SelectOutput {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
        },
        0x4a => RxCommand::ChannelDelay {
            endpoint: reader.u8().ok_or(ProtocolError::InvalidPayload(command))?,
            ticks: reader
                .u32be()
                .ok_or(ProtocolError::InvalidPayload(command))?,
        },
        _ => return Err(ProtocolError::UnsupportedCommand(command)),
    };
    if !reader.done() {
        return Err(ProtocolError::InvalidPayload(command));
    }
    Ok(decoded)
}

fn read_bool(reader: &mut Reader<'_>, command: u8) -> Result<bool, ProtocolError> {
    match reader.u8() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(ProtocolError::InvalidPayload(command)),
    }
}

#[derive(Debug, Default)]
pub struct CommandStreamDecoder {
    reassembler: FragmentReassembler,
}

impl CommandStreamDecoder {
    pub fn accept(&mut self, raw: &[u8]) -> Result<Option<RxCommand>, ProtocolError> {
        let Some((command, payload)) = self.reassembler.accept(raw)? else {
            return Ok(None);
        };
        decode_payload(command, &payload).map(Some)
    }

    pub fn pending_count(&self) -> usize {
        self.reassembler.pending_count()
    }

    pub fn clear(&mut self) {
        self.reassembler.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn zero_input_state() -> InputState {
        InputState {
            id: 0,
            flags: 0,
            gain_db: 0.0,
            compressor_amount: 0.0,
            high_pass_hz: 0,
            balance: 0.0,
            eq_gain_db: [0.0; 4],
            eq_frequency_hz: [0; 4],
            eq_q: [0.0; 4],
            label: ChannelLabel {
                endpoint: 0,
                icon: 0,
                text: String::new(),
            },
        }
    }

    fn zero_output_state() -> OutputState {
        OutputState {
            id: 0,
            flags: 0,
            volume_db: 0.0,
            pan: 0.0,
            limiter_db: 0.0,
            input_gains_db: [0.0; 7],
            eq_gain_db: [0.0; 9],
            eq_frequency_hz: [0; 9],
            eq_q: [0.0; 9],
            delay_ticks: 0,
        }
    }

    fn zero_fx_state() -> FxState {
        FxState {
            id: 0,
            flags: 0,
            volume_db: 0.0,
            pan: 0.0,
            values: [0; 3],
            preset: 0,
            input_gains_db: [0.0; 7],
            aux_gains_db: [0.0; 2],
        }
    }

    fn zero_mixer_state() -> MixerState {
        MixerState {
            inputs: std::array::from_fn(|_| zero_input_state()),
            outputs: std::array::from_fn(|_| zero_output_state()),
            effects: std::array::from_fn(|_| zero_fx_state()),
            headphone_volume_db: 0.0,
            flags: [false; 13],
            tempo_bpm: 0,
            selected_output: 0,
            last_snapshot: 0,
            monitor_routing: 0,
            snapshot_scope: 0,
        }
    }

    #[test]
    fn all_31_target_commands_match_retained_exact_apk_vectors() {
        let mut covered = BTreeSet::new();
        let mut check = |command: TxCommand, expected: &str| {
            let bytes = encode(&command).unwrap();
            let expected = hex(expected);
            assert_eq!(bytes, expected, "vector for {command:?}");
            let packet = parse_packet(&expected).unwrap();
            decode_payload(packet.command, &packet.payload)
                .unwrap_or_else(|error| panic!("RX parser for {command:?}: {error}"));
            covered.insert(bytes[0]);
        };

        check(
            TxCommand::Pan {
                input: InputId::Input56,
                value: 0.0,
            },
            "0001047f84",
        );
        check(
            TxCommand::Solo {
                input: InputId::Input1,
                enabled: true,
            },
            "0101000103",
        );
        check(
            TxCommand::Gain {
                input: InputId::Input1,
                db: 0.0,
            },
            "020100787b",
        );
        check(
            TxCommand::GraphicEq {
                endpoint: 0x0f,
                band: 8,
                frequency_hz: 16_000,
                q: 1.0,
                gain_db: 0.0,
            },
            "03010f083e80087f60",
        );
        check(
            TxCommand::HighPassFilter {
                input: InputId::Input1,
                enabled: true,
                frequency_hz: 600,
            },
            "04010001025860",
        );
        check(
            TxCommand::Label(ChannelLabel {
                endpoint: 0,
                icon: 0x1234,
                text: "CH1".into(),
            }),
            "0501001234034348310b",
        );
        check(
            TxCommand::RouteLevel {
                source: InputId::Input1,
                destination: MixDestination::Main,
                normalized: 0.5,
            },
            "0601000f7f95",
        );
        check(
            TxCommand::Mute {
                endpoint: 0x0c,
                enabled: true,
            },
            "08010c0116",
        );
        check(
            TxCommand::ParametricEq {
                input: InputId::Input1,
                band: 2,
                frequency_hz: 1_000,
                q: 1.0,
                gain_db: 0.0,
            },
            "0901000203e8087f7e",
        );
        check(
            TxCommand::Phase {
                input: InputId::Input1,
                inverted: true,
            },
            "1001000112",
        );
        check(
            TxCommand::FxSetup {
                endpoint: 0x0c,
                values: [50, 1, 7],
                route_flags: 0x05,
            },
            "11010c320107055d",
        );
        check(TxCommand::SnapshotDelete { slot: 3 }, "12010316");
        check(
            TxCommand::Compressor {
                input: InputId::Input1,
                amount: 0.5,
            },
            "1301007f93",
        );
        check(
            TxCommand::Limiter {
                endpoint: 0x0f,
                threshold_db: -30.0,
            },
            "14010f3c60",
        );
        check(
            TxCommand::Phantom {
                input: InputId::Input1,
                enabled: true,
            },
            "1501000117",
        );
        check(TxCommand::GetChannelState { endpoint: 3 }, "1601031a");
        check(
            TxCommand::InputState(zero_input_state()),
            "17010000780000007f7f7f7f7f000000000000000000000000000000000b",
        );
        check(
            TxCommand::OutputState(zero_output_state()),
            concat!(
                "18010000bf7f78",
                "bfbfbfbfbfbfbf",
                "7f7f7f7f7f7f7f7f7f",
                "000000000000000000000000000000000000",
                "000000000000000000",
                "00000000",
                "7f"
            ),
        );
        check(
            TxCommand::SnapshotSave {
                slot: 3,
                name: "Band".into(),
            },
            "1901030442616e6496",
        );
        check(TxCommand::SnapshotLoad { slot: 0x7f }, "20017fa0");
        check(
            TxCommand::MeterRequest(MeterRequest {
                count: 8,
                channel_codes: [
                    0x40, 0x41, 0x42, 0x43, 0xc4, 0xc5, 0xc6, 0x8f, 0, 0, 0, 0, 0, 0, 0,
                ],
            }),
            "21010840414243c4c5c68f000000000000000e",
        );
        check(
            TxCommand::MeterUpdate(MeterUpdate {
                meters_db: [
                    -144.0,
                    route_level_table()[0x85],
                    route_level_table()[0x8c],
                    route_level_table()[0x92],
                    route_level_table()[0x99],
                    route_level_table()[0x9f],
                    route_level_table()[0xa5],
                    route_level_table()[0xac],
                    route_level_table()[0xb2],
                    route_level_table()[0xb8],
                    route_level_table()[0xbf],
                    route_level_table()[0xc5],
                    route_level_table()[0xcc],
                    route_level_table()[0xd2],
                    route_level_table()[0xd8],
                ],
                gain_reduction_bits: 0x1234,
            }),
            "220100858c92999fa5acb2b8bfc5ccd2d81234f9",
        );
        check(
            TxCommand::Setting {
                id: 7,
                data: vec![1],
            },
            "25010701012f",
        );
        check(TxCommand::FactoryReset, "29012a");
        check(
            TxCommand::FxState(zero_fx_state()),
            "30010000bf7f00000000bfbfbfbfbfbfbfbfbf26",
        );
        check(
            TxCommand::FxPreset {
                endpoint: 0x0d,
                preset: 4,
            },
            "31010d0443",
        );
        check(
            TxCommand::ChannelConnection {
                endpoint: 4,
                subchannel: 1,
                connected: true,
            },
            "33010401013a",
        );
        check(TxCommand::FxTempo { bpm: 120 }, "40010078b9");
        check(TxCommand::SelectOutput { endpoint: 0x0f }, "41010f51");
        check(
            TxCommand::ChannelDelay {
                endpoint: 0x0f,
                ticks: 0x0102_0304,
            },
            "4a010f0102030464",
        );
        drop(check);
        covered.insert(0x38);

        let frames = encode_frames(
            &TxCommand::MixerState(Box::new(zero_mixer_state())),
            MAX_RAW_PACKET_SIZE,
            0,
        )
        .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!((frames[0].len(), frames[1].len()), (251, 147));
        assert_eq!(frames[0][..4], [0x38, 0x02, 0x00, 0x00]);
        assert_eq!(frames[1][..4], [0x38, 0x02, 0x00, 0x01]);
        assert_eq!(frames[0].last(), Some(&0xfb));
        assert_eq!(frames[1].last(), Some(&0xfa));
        let mut decoder = CommandStreamDecoder::default();
        assert!(decoder.accept(&frames[0]).unwrap().is_none());
        assert!(matches!(
            decoder.accept(&frames[1]).unwrap(),
            Some(RxCommand::MixerState(_))
        ));

        assert_eq!(
            covered,
            BTreeSet::from([
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x08, 0x09, 0x10, 0x11, 0x12, 0x13, 0x14,
                0x15, 0x16, 0x17, 0x18, 0x19, 0x20, 0x21, 0x22, 0x25, 0x29, 0x30, 0x31, 0x33, 0x38,
                0x40, 0x41, 0x4a,
            ])
        );
    }

    #[test]
    fn first_stage_tx_commands_match_exact_apk_vectors() {
        assert_eq!(
            encode(&TxCommand::Pan {
                input: InputId::Input56,
                value: 0.0
            })
            .unwrap(),
            hex("0001047f84")
        );
        assert_eq!(
            encode(&TxCommand::Solo {
                input: InputId::Input1,
                enabled: true
            })
            .unwrap(),
            hex("0101000103")
        );
        assert_eq!(
            encode(&TxCommand::Mute {
                endpoint: 0x0c,
                enabled: true
            })
            .unwrap(),
            hex("08010c0116")
        );
        assert_eq!(
            encode(&TxCommand::Phase {
                input: InputId::Input1,
                inverted: true
            })
            .unwrap(),
            hex("1001000112")
        );
        assert_eq!(
            encode(&TxCommand::Phantom {
                input: InputId::Input1,
                enabled: true
            })
            .unwrap(),
            hex("1501000117")
        );
    }

    #[test]
    fn documented_boundary_vectors_match_apk_native_serializer() {
        let mut cases = 0_usize;
        let mut check = |command: TxCommand, expected: &str| {
            cases += 1;
            assert_eq!(encode(&command).unwrap(), hex(expected), "case {cases}");
        };
        check(
            TxCommand::Pan {
                input: InputId::Input56,
                value: -1.0,
            },
            "0001040005",
        );
        check(
            TxCommand::Pan {
                input: InputId::Input56,
                value: 1.0,
            },
            "000104fe03",
        );
        check(
            TxCommand::Solo {
                input: InputId::Input1,
                enabled: false,
            },
            "0101000002",
        );
        for (input, db, expected) in [
            (InputId::Input56, -20.0, "0201045057"),
            (InputId::Input56, 0.0, "020104787f"),
            (InputId::Input56, 20.0, "020104a0a7"),
            (InputId::Input56, 40.0, "020104c8cf"),
            (InputId::Input56, 60.0, "020104f0f7"),
            (InputId::Input1, -80.0, "0201000003"),
            (InputId::Input1, 80.0, "020100f0f3"),
        ] {
            check(TxCommand::Gain { input, db }, expected);
        }
        check(
            TxCommand::GraphicEq {
                endpoint: 0,
                band: 0,
                frequency_hz: 0,
                q: 0.0,
                gain_db: 0.0,
            },
            "030100000000007f83",
        );
        check(
            TxCommand::GraphicEq {
                endpoint: 15,
                band: 8,
                frequency_hz: 16_000,
                q: 20.0,
                gain_db: 15.0,
            },
            "03010f083e806ffe46",
        );
        check(
            TxCommand::HighPassFilter {
                input: InputId::Input1,
                enabled: false,
                frequency_hz: 0,
            },
            "04010000000005",
        );
        check(
            TxCommand::Label(ChannelLabel {
                endpoint: 0,
                icon: 0,
                text: String::new(),
            }),
            "05010000000006",
        );
        check(
            TxCommand::RouteLevel {
                source: InputId::Input1,
                destination: MixDestination::Monitor1,
                normalized: 0.5,
            },
            "0601000a7f90",
        );
        check(
            TxCommand::DestinationMaster {
                destination: MixDestination::Main,
                normalized: 0.5,
            },
            "06010f0f7fa4",
        );
        check(
            TxCommand::Mute {
                endpoint: 0,
                enabled: false,
            },
            "0801000009",
        );
        check(
            TxCommand::ParametricEq {
                input: InputId::Input1,
                band: 0,
                frequency_hz: 0,
                q: 0.0,
                gain_db: 0.0,
            },
            "090100000000007f89",
        );
        check(
            TxCommand::ParametricEq {
                input: InputId::Input1,
                band: 2,
                frequency_hz: 1_000,
                q: 20.0,
                gain_db: 15.0,
            },
            "0901000203e86ffe64",
        );
        check(
            TxCommand::Phase {
                input: InputId::Input1,
                inverted: false,
            },
            "1001000011",
        );
        check(
            TxCommand::FxSetup {
                endpoint: 0,
                values: [0; 3],
                route_flags: 0,
            },
            "1101000000000012",
        );
        check(TxCommand::SnapshotDelete { slot: 0 }, "12010013");
        check(
            TxCommand::Compressor {
                input: InputId::Input1,
                amount: 0.0,
            },
            "1301000014",
        );
        check(
            TxCommand::Compressor {
                input: InputId::Input1,
                amount: 1.0,
            },
            "130100ff13",
        );
        check(
            TxCommand::Limiter {
                endpoint: 0,
                threshold_db: 0.0,
            },
            "140100788d",
        );
        check(
            TxCommand::Limiter {
                endpoint: 15,
                threshold_db: 0.0,
            },
            "14010f789c",
        );
        check(
            TxCommand::Phantom {
                input: InputId::Input1,
                enabled: false,
            },
            "1501000016",
        );
        check(TxCommand::GetChannelState { endpoint: 0 }, "16010017");
        check(
            TxCommand::SnapshotSave {
                slot: 0,
                name: String::new(),
            },
            "190100001a",
        );
        check(TxCommand::SnapshotLoad { slot: 0 }, "20010021");
        check(TxCommand::SnapshotLoad { slot: 3 }, "20010324");
        check(
            TxCommand::MeterRequest(MeterRequest {
                count: 0,
                channel_codes: [0; 15],
            }),
            "21010000000000000000000000000000000022",
        );
        check(
            TxCommand::MeterUpdate(MeterUpdate {
                meters_db: [0.0; 15],
                gain_reduction_bits: 0,
            }),
            "2201bfbfbfbfbfbfbfbfbfbfbfbfbfbfbf000054",
        );
        check(
            TxCommand::Setting {
                id: 0,
                data: Vec::new(),
            },
            "2501000026",
        );
        check(
            TxCommand::Setting {
                id: 5,
                data: b"FLOW".to_vec(),
            },
            "25010504464c4f5767",
        );
        check(
            TxCommand::FxPreset {
                endpoint: 0,
                preset: 0,
            },
            "3101000032",
        );
        check(
            TxCommand::ChannelConnection {
                endpoint: 0,
                subchannel: 0,
                connected: false,
            },
            "330100000034",
        );
        check(TxCommand::FxTempo { bpm: 0 }, "4001000041");
        check(TxCommand::FxTempo { bpm: 300 }, "4001012c6e");
        check(TxCommand::SelectOutput { endpoint: 0 }, "41010042");
        check(
            TxCommand::ChannelDelay {
                endpoint: 0,
                ticks: 0,
            },
            "4a0100000000004b",
        );
        assert_eq!(cases, 41);
    }

    #[test]
    fn companion_session_vectors_match_apk_descriptors() {
        let cases = [
            (TxCommand::GetSnapshotNames, "070108"),
            (TxCommand::GetChannelLabels, "230124"),
            (TxCommand::GetSetting { id: 3 }, "2601032a"),
            (
                TxCommand::SnapshotRename {
                    slot: 3,
                    name: "Band".into(),
                },
                "3201030442616e64af",
            ),
            (
                TxCommand::HandshakeHostProbe {
                    device_id: [0; 16],
                    pairing_any: false,
                    protocol_version: 0,
                    firmware_build: 0,
                },
                "350100000000000000000000000000000000000000000036",
            ),
            (TxCommand::HandshakeReplyProbe, "360137"),
            (TxCommand::GetMixerState, "370138"),
            (
                TxCommand::HandshakeClient { client_id: [0; 16] },
                "3901000000000000000000000000000000003a",
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(encode(&command).unwrap(), hex(expected));
        }

        let handshake_host = hex("350100000000000000000000000000000000000000000036");
        let packet = parse_packet(&handshake_host).unwrap();
        assert!(matches!(
            decode_payload(packet.command, &packet.payload).unwrap(),
            RxCommand::HandshakeHost { .. }
        ));
        let packet = parse_packet(&hex("360137")).unwrap();
        assert_eq!(
            decode_payload(packet.command, &packet.payload).unwrap(),
            RxCommand::HandshakeReply
        );

        let mut snapshot_names = vec![0x27, 0x01];
        snapshot_names.extend([0; 15]);
        snapshot_names.push(0x28);
        let packet = parse_packet(&snapshot_names).unwrap();
        assert!(matches!(
            decode_payload(packet.command, &packet.payload).unwrap(),
            RxCommand::SnapshotNames(names) if names.len() == 15
        ));

        let mut channel_labels = vec![0x24, 0x01, 0x00];
        channel_labels.extend([0; 40]);
        channel_labels.push(0x25);
        let packet = parse_packet(&channel_labels).unwrap();
        assert!(matches!(
            decode_payload(packet.command, &packet.payload).unwrap(),
            RxCommand::ChannelLabels(labels) if labels.len() == 10
        ));
    }

    #[test]
    fn gain_exact_vectors_match_apk_native_serializer() {
        for (db, expected) in [
            (-20.0, "0201005053"),
            (0.0, "020100787b"),
            (20.0, "020100a0a3"),
            (40.0, "020100c8cb"),
            (60.0, "020100f0f3"),
        ] {
            assert_eq!(
                encode(&TxCommand::Gain {
                    input: InputId::Input1,
                    db
                })
                .unwrap(),
                hex(expected)
            );
        }
        assert_eq!(encode_fix8(Fix8Format::GainDb, -19.75), Some(0x50));
        assert_eq!(encode_fix8(Fix8Format::GainDb, -19.25), Some(0x52));
    }

    #[test]
    fn route_level_exact_vectors_match_apk_native_serializer() {
        for (normalized, expected) in [
            (0.0, "0601000f0016"),
            (0.25, "0601000f3f55"),
            (0.5, "0601000f7f95"),
            (0.75, "0601000fbfd5"),
            (1.0, "0601000fff15"),
        ] {
            assert_eq!(
                encode(&TxCommand::RouteLevel {
                    source: InputId::Input1,
                    destination: MixDestination::Main,
                    normalized
                })
                .unwrap(),
                hex(expected)
            );
        }
        assert_eq!(
            encode(&TxCommand::DestinationMaster {
                destination: MixDestination::Monitor1,
                normalized: 0.5
            })
            .unwrap(),
            hex("06010a0a7f9a")
        );
    }

    fn empty_mixer_payload() -> Vec<u8> {
        let input = hex("0000780000007f7f7f7f7f00000000000000000000000000000000");
        let mut output = vec![0x00, 0x00, 0xbf, 0x7f, 0x78];
        output.extend([0xbf; 7]);
        output.extend([0x7f; 9]);
        output.extend([0x00; 18]);
        output.extend([0x00; 9]);
        output.extend([0x00; 4]);
        let fx = hex("0000bf7f00000000bfbfbfbfbfbfbfbfbf");
        let mut payload = Vec::new();
        for _ in 0..7 {
            payload.extend(&input);
        }
        for _ in 0..3 {
            payload.extend(&output);
        }
        for _ in 0..2 {
            payload.extend(&fx);
        }
        payload.extend(hex("bf0000000000000000"));
        payload
    }

    #[test]
    fn mixer_state_fragments_reassemble_and_parse_atomically() {
        let payload = empty_mixer_payload();
        assert_eq!(payload.len(), 388);
        let frames = frame_command(0x38, &payload, 251, 0).unwrap();
        assert_eq!((frames[0].len(), frames[1].len()), (251, 147));
        assert_eq!(
            frames[0],
            hex(concat!(
                "380200000000780000007f7f7f7f7f000000000000000000000000000000000000780000007f7f7f7f7f000000000000000000000000000000000000780000007f7f7f7f7f000000000000000000000000000000000000780000007f7f7f7f7f000000000000000000000000000000000000780000007f7f7f7f7f000000000000000000000000000000000000780000007f7f7f7f7f000000000000000000000000000000000000780000007f7f7f7f7f000000000000000000000000000000000000bf7f78",
                "bfbfbfbfbfbfbf7f7f7f7f7f7f7f7f7f000000000000000000000000000000000000000000000000000000000000000000bf7f78fb"
            ))
        );
        assert_eq!(
            frames[1],
            hex(concat!(
                "38020001bfbfbfbfbfbfbf7f7f7f7f7f7f7f7f7f000000000000000000000000000000000000000000000000000000000000000000bf7f78",
                "bfbfbfbfbfbfbf7f7f7f7f7f7f7f7f7f000000000000000000000000000000000000000000000000000000000000000000bf7f00000000",
                "bfbfbfbfbfbfbfbfbf0000bf7f00000000bfbfbfbfbfbfbfbfbfbf0000000000000000fa"
            ))
        );
        let mut reassembler = FragmentReassembler::default();
        assert!(reassembler.accept(&frames[1]).unwrap().is_none());
        let (command, complete) = reassembler.accept(&frames[0]).unwrap().unwrap();
        assert_eq!(command, 0x38);
        match decode_payload(command, &complete).unwrap() {
            RxCommand::MixerState(state) => {
                assert_eq!(state.inputs.len(), 7);
                assert_eq!(state.outputs.len(), 3);
                assert_eq!(state.effects.len(), 2);
            }
            _ => panic!("expected mixer state"),
        }
    }

    #[test]
    fn mixer_state_reassembly_is_mtu_independent_and_accepts_four_fragments() {
        let payload = empty_mixer_payload();
        let frames = frame_command(0x38, &payload, 128, 3).unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].len(), 128);
        assert_eq!(frames[1].len(), 128);
        assert_eq!(frames[2].len(), 128);
        assert!(frames[3].len() < 128);

        for (index, frame) in frames.iter().enumerate() {
            let packet = parse_packet(frame).unwrap();
            assert_eq!(packet.command, 0x38);
            assert_eq!(packet.fragment_count, 4);
            assert_eq!(packet.sequence, Some(3));
            assert_eq!(packet.fragment_index, Some(index as u8));
        }

        let mut decoder = CommandStreamDecoder::default();
        for frame in &frames[..3] {
            assert!(decoder.accept(frame).unwrap().is_none());
        }
        match decoder.accept(&frames[3]).unwrap() {
            Some(RxCommand::MixerState(state)) => {
                assert_eq!(state.inputs.len(), 7);
                assert_eq!(state.outputs.len(), 3);
                assert_eq!(state.effects.len(), 2);
            }
            other => panic!("expected complete four-fragment MixerState, got {other:?}"),
        }
    }

    #[test]
    fn broken_fragments_never_produce_state() {
        let frames = frame_command(0x38, &empty_mixer_payload(), 251, 1).unwrap();

        // Missing and identical duplicate fragments stay incomplete. A fragment
        // carrying another sequence number must occupy a separate assembly.
        let mut incomplete = FragmentReassembler::default();
        assert!(incomplete.accept(&frames[0]).unwrap().is_none());
        assert!(incomplete.accept(&frames[0]).unwrap().is_none());
        assert_eq!(incomplete.pending_count(), 1);
        let mut wrong_sequence = frames[1].clone();
        wrong_sequence[2] = 2;
        let last = wrong_sequence.len() - 1;
        wrong_sequence[last] = checksum(&wrong_sequence[..last]);
        assert!(incomplete.accept(&wrong_sequence).unwrap().is_none());
        assert_eq!(incomplete.pending_count(), 2);

        let mut duplicate = frames[0].clone();
        duplicate[4] ^= 1;
        let last = duplicate.len() - 1;
        duplicate[last] = checksum(&duplicate[..last]);
        let mut reassembler = FragmentReassembler::default();
        assert!(reassembler.accept(&frames[0]).unwrap().is_none());
        assert_eq!(
            reassembler.accept(&duplicate),
            Err(ProtocolError::ConflictingFragment)
        );
        let mut bad = frames[1].clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert_eq!(
            FragmentReassembler::default().accept(&bad),
            Err(ProtocolError::ChecksumMismatch)
        );
    }

    #[test]
    fn compound_rx_exact_vectors_match_retained_apk_fixtures() {
        let input = hex("17010000780000007f7f7f7f7f000000000000000000000000000000000b");
        let packet = parse_packet(&input).unwrap();
        match decode_payload(packet.command, &packet.payload).unwrap() {
            RxCommand::InputState(state) => {
                assert_eq!(state.id, 0);
                assert_eq!(state.gain_db, 0.0);
                assert_eq!(state.high_pass_hz, 0);
                assert_eq!(state.label.endpoint, 0);
            }
            _ => panic!("expected input state"),
        }

        let output = hex(concat!(
            "18010000bf7f78",
            "bfbfbfbfbfbfbf",
            "7f7f7f7f7f7f7f7f7f",
            "000000000000000000000000000000000000",
            "000000000000000000",
            "00000000",
            "7f"
        ));
        let packet = parse_packet(&output).unwrap();
        match decode_payload(packet.command, &packet.payload).unwrap() {
            RxCommand::OutputState(state) => {
                assert_eq!(state.id, 0);
                assert_eq!(state.volume_db, 0.0);
                assert_eq!(state.delay_ticks, 0);
            }
            _ => panic!("expected output state"),
        }

        let fx = hex("30010000bf7f00000000bfbfbfbfbfbfbfbfbf26");
        let packet = parse_packet(&fx).unwrap();
        match decode_payload(packet.command, &packet.payload).unwrap() {
            RxCommand::FxState(state) => {
                assert_eq!(state.id, 0);
                assert_eq!(state.volume_db, 0.0);
                assert_eq!(state.values, [0; 3]);
            }
            _ => panic!("expected FX state"),
        }
    }
}
