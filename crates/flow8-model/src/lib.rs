//! Transport- and GUI-independent FLOW 8 product model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceStatus {
    VerifiedFromDevice,
    VerifiedFromApk,
    Inferred,
    Unknown,
    Blocked,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Normalized,
    Decibels,
    Hertz,
    Percent,
    Milliseconds,
    Bpm,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub min: f32,
    pub max: f32,
    pub step: Option<f32>,
    pub default: f32,
    pub unit: Unit,
    pub evidence: EvidenceStatus,
    pub display_decimals: u8,
}

impl ParameterSpec {
    pub const fn new(
        min: f32,
        max: f32,
        step: Option<f32>,
        default: f32,
        unit: Unit,
        evidence: EvidenceStatus,
    ) -> Self {
        Self {
            min,
            max,
            step,
            default,
            unit,
            evidence,
            display_decimals: match unit {
                Unit::Decibels => 1,
                Unit::Normalized => 2,
                Unit::Milliseconds => 2,
                Unit::Hertz | Unit::Percent | Unit::Bpm | Unit::Raw => 0,
            },
        }
    }

    pub fn clamp(self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }
}

pub mod specs {
    use super::{EvidenceStatus::*, ParameterSpec, Unit};

    pub const ROUTE_LEVEL: ParameterSpec =
        ParameterSpec::new(0.0, 1.0, None, 0.75, Unit::Normalized, VerifiedFromApk);
    // FLOW Mix display range. The verified native 0x02 wire clamp is kept
    // separately so UI assumptions never redefine the codec.
    pub const INPUT_GAIN_DISPLAY: ParameterSpec =
        ParameterSpec::new(-20.0, 60.0, Some(0.5), 0.0, Unit::Decibels, VerifiedFromApk);
    pub const INPUT_GAIN_WIRE: ParameterSpec =
        ParameterSpec::new(-60.0, 60.0, Some(0.5), 0.0, Unit::Decibels, VerifiedFromApk);
    pub const PAN: ParameterSpec =
        ParameterSpec::new(-1.0, 1.0, None, 0.0, Unit::Normalized, VerifiedFromApk);
    pub const HIGH_PASS: ParameterSpec =
        ParameterSpec::new(20.0, 600.0, Some(1.0), 80.0, Unit::Hertz, VerifiedFromApk);
    pub const COMPRESSOR_AMOUNT: ParameterSpec =
        ParameterSpec::new(0.0, 1.0, None, 0.0, Unit::Normalized, VerifiedFromApk);
    pub const EQ_GAIN: ParameterSpec =
        ParameterSpec::new(-15.0, 15.0, Some(0.1), 0.0, Unit::Decibels, VerifiedFromApk);
    pub const EQ_FREQUENCY_WIRE: ParameterSpec =
        ParameterSpec::new(0.0, 65_535.0, Some(1.0), 0.0, Unit::Hertz, VerifiedFromApk);
    pub const EQ_Q_WIRE: ParameterSpec =
        ParameterSpec::new(0.0, 164.0, None, 1.0, Unit::Raw, VerifiedFromApk);
    pub const LIMITER: ParameterSpec =
        ParameterSpec::new(-30.0, 0.0, Some(0.5), -3.0, Unit::Decibels, VerifiedFromApk);
    pub const FX_RAW: ParameterSpec =
        ParameterSpec::new(0.0, 255.0, Some(1.0), 0.0, Unit::Raw, Unknown);
    pub const TEMPO_WIRE: ParameterSpec =
        ParameterSpec::new(0.0, 65_535.0, Some(1.0), 120.0, Unit::Bpm, VerifiedFromApk);
    // UI guardrail inherited from the APK-facing product behavior. The wire
    // serializer remains full u16; accepted hardware limits need calibration.
    pub const TEMPO_DISPLAY: ParameterSpec =
        ParameterSpec::new(30.0, 300.0, Some(1.0), 120.0, Unit::Bpm, Inferred);
    pub const DELAY_TICKS: ParameterSpec = ParameterSpec::new(
        0.0,
        u32::MAX as f32,
        Some(1.0),
        0.0,
        Unit::Raw,
        VerifiedFromApk,
    );
    pub const METER_DISPLAY: ParameterSpec =
        ParameterSpec::new(-60.0, 10.0, None, -60.0, Unit::Decibels, Unknown);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateError {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateValue<T> {
    pub confirmed: Option<T>,
    pub pending: Option<T>,
    pub error: Option<StateError>,
    pub evidence: EvidenceStatus,
}

impl<T> StateValue<T> {
    pub fn unknown() -> Self {
        Self {
            confirmed: None,
            pending: None,
            error: None,
            evidence: EvidenceStatus::Unknown,
        }
    }

    pub fn confirmed(value: T, evidence: EvidenceStatus) -> Self {
        Self {
            confirmed: Some(value),
            pending: None,
            error: None,
            evidence,
        }
    }

    pub fn set_pending(&mut self, value: T) {
        self.pending = Some(value);
        self.error = None;
    }

    /// Device observations are authoritative, including when they disagree
    /// with an optimistic local pending value.
    pub fn observe(&mut self, value: T, evidence: EvidenceStatus) {
        self.confirmed = Some(value);
        self.pending = None;
        self.error = None;
        self.evidence = evidence;
    }

    pub fn effective(&self) -> Option<&T> {
        self.pending.as_ref().or(self.confirmed.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum InputId {
    Input1 = 0,
    Input2 = 1,
    Input3 = 2,
    Input4 = 3,
    Input56 = 4,
    Input78 = 5,
    UsbBluetooth = 6,
}

impl InputId {
    pub const ALL: [Self; 7] = [
        Self::Input1,
        Self::Input2,
        Self::Input3,
        Self::Input4,
        Self::Input56,
        Self::Input78,
        Self::UsbBluetooth,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn endpoint(self) -> u8 {
        self as u8
    }
    pub const fn is_stereo(self) -> bool {
        matches!(self, Self::Input56 | Self::Input78 | Self::UsbBluetooth)
    }
    pub const fn has_analog_gain(self) -> bool {
        !matches!(self, Self::UsbBluetooth)
    }
    pub const fn has_phantom(self) -> bool {
        matches!(self, Self::Input1 | Self::Input2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MixDestination {
    Main,
    Monitor1,
    Monitor2,
    Fx1,
    Fx2,
}

impl MixDestination {
    pub const ALL: [Self; 5] = [
        Self::Main,
        Self::Monitor1,
        Self::Monitor2,
        Self::Fx1,
        Self::Fx2,
    ];
    pub const fn index(self) -> usize {
        match self {
            Self::Main => 0,
            Self::Monitor1 => 1,
            Self::Monitor2 => 2,
            Self::Fx1 => 3,
            Self::Fx2 => 4,
        }
    }
    pub const fn endpoint(self) -> u8 {
        match self {
            Self::Monitor1 => 10,
            Self::Monitor2 => 11,
            Self::Fx1 => 12,
            Self::Fx2 => 13,
            Self::Main => 15,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixBusId {
    Main,
    Monitor1,
    Monitor2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxId {
    Fx1,
    Fx2,
}

/// The hardware/official documentation confirms 16 slots per engine. The
/// individual names and zero-based name-to-BLE-ID association below come from
/// an independent FLOW 8 controller, not a captured FLOW 8 preset switch.
/// Keep that distinction until verified on the physical device.
pub const FX_PRESET_COUNT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxPresetInfo {
    pub id: u8,
    pub name: &'static str,
    pub evidence: EvidenceStatus,
}

// Source for the names/order: abelroes/flow-8-midi,
// src/model/channels.rs (FX1_PRESETS / FX2_PRESETS). Official FLOW 8 block
// diagram independently confirms slots 0-11 are reverb or delay/echo,
// slot 12 is flanger, and slots 13-15 are chorus variants.
pub const FX1_PRESET_NAMES: [&str; FX_PRESET_COUNT] = [
    "Ambience",
    "Perc-Rev1",
    "Perc-Rev2",
    "Guit-Rev1",
    "Guit-Rev2",
    "Chamber",
    "Room",
    "Concert",
    "Church",
    "Cathedral",
    "Temple",
    "Stadium",
    "Flanger",
    "Soft Chor",
    "Warm Chor",
    "Deep Chor",
];

pub const FX2_PRESET_NAMES: [&str; FX_PRESET_COUNT] = [
    "Delay 1/1",
    "Delay 1/2",
    "Delay 1/3",
    "Delay 2/1",
    "Echo 1/1",
    "Echo 1/2",
    "Echo 1/3",
    "Echo 2/1",
    "Wide Echo",
    "Ping Pong",
    "Ping P 1/3",
    "Ping P R>L",
    "Flanger",
    "Soft Chor",
    "Warm Chor",
    "Deep Chor",
];

pub fn fx_preset_info(fx: FxId, id: u8) -> Option<FxPresetInfo> {
    let names = match fx {
        FxId::Fx1 => &FX1_PRESET_NAMES,
        FxId::Fx2 => &FX2_PRESET_NAMES,
    };
    names.get(id as usize).map(|name| FxPresetInfo {
        id,
        name,
        evidence: EvidenceStatus::Inferred,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbAudioEndpointId {
    Usb12,
    Usb34,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalOutputId {
    MainOut,
    MonitorOut1,
    MonitorOut2,
    Headphones,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadphoneSource {
    Main,
    Monitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapPoint {
    PreFader,
    PostFader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCapabilities {
    pub gain: bool,
    pub phase: bool,
    pub phantom: bool,
    pub high_pass: bool,
    pub compressor: bool,
    pub peq: bool,
}

impl InputCapabilities {
    pub const fn for_input(id: InputId) -> Self {
        let analog = !matches!(id, InputId::UsbBluetooth);
        Self {
            gain: analog,
            phase: analog,
            phantom: matches!(id, InputId::Input1 | InputId::Input2),
            high_pass: analog,
            compressor: analog,
            peq: analog,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeterState {
    pub level_db: StateValue<f32>,
    pub peak_db: StateValue<f32>,
    pub gain_reduction_db: StateValue<f32>,
    pub clipping: StateValue<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqBandState {
    pub frequency_hz: StateValue<f32>,
    pub gain_db: StateValue<f32>,
    pub q: StateValue<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqState {
    pub enabled: StateValue<bool>,
    pub bands: Vec<EqBandState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompressorState {
    pub amount: StateValue<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimiterState {
    pub threshold_db: StateValue<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputChannelState {
    pub id: InputId,
    pub name: StateValue<String>,
    pub icon: StateValue<u16>,
    pub visible: StateValue<bool>,
    pub capabilities: InputCapabilities,
    pub gain_db: StateValue<f32>,
    pub pan: StateValue<f32>,
    pub muted: StateValue<bool>,
    pub soloed: StateValue<bool>,
    pub phase_inverted: StateValue<bool>,
    pub phantom_48v: StateValue<bool>,
    pub left_connected: StateValue<bool>,
    pub right_connected: StateValue<bool>,
    pub high_pass_enabled: StateValue<bool>,
    pub high_pass_hz: StateValue<f32>,
    pub compressor: CompressorState,
    pub eq: EqState,
    pub route_levels: [StateValue<f32>; 5],
    pub meter: MeterState,
}

impl InputChannelState {
    pub fn synthetic(id: InputId, name: impl Into<String>) -> Self {
        let synthetic = EvidenceStatus::Synthetic;
        let band = |frequency| EqBandState {
            frequency_hz: StateValue::confirmed(frequency, synthetic),
            gain_db: StateValue::confirmed(0.0, synthetic),
            q: StateValue::confirmed(1.0, synthetic),
        };
        Self {
            id,
            name: StateValue::confirmed(name.into(), synthetic),
            icon: StateValue::confirmed(0, synthetic),
            visible: StateValue::confirmed(true, synthetic),
            capabilities: InputCapabilities::for_input(id),
            gain_db: StateValue::confirmed(0.0, synthetic),
            pan: StateValue::confirmed(0.0, synthetic),
            muted: StateValue::confirmed(false, synthetic),
            soloed: StateValue::confirmed(false, synthetic),
            phase_inverted: StateValue::confirmed(false, synthetic),
            phantom_48v: StateValue::confirmed(false, synthetic),
            left_connected: StateValue::confirmed(true, synthetic),
            right_connected: StateValue::confirmed(id.is_stereo(), synthetic),
            high_pass_enabled: StateValue::confirmed(false, synthetic),
            high_pass_hz: StateValue::confirmed(80.0, synthetic),
            compressor: CompressorState {
                amount: StateValue::confirmed(0.0, synthetic),
            },
            eq: EqState {
                enabled: StateValue::confirmed(true, synthetic),
                bands: vec![band(80.0), band(400.0), band(2_500.0), band(10_000.0)],
            },
            route_levels: std::array::from_fn(|_| StateValue::confirmed(0.72, synthetic)),
            meter: MeterState {
                level_db: StateValue::confirmed(-60.0, synthetic),
                peak_db: StateValue::confirmed(-60.0, synthetic),
                gain_reduction_db: StateValue::confirmed(0.0, synthetic),
                clipping: StateValue::confirmed(false, synthetic),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixBusState {
    pub id: MixBusId,
    pub master_level: StateValue<f32>,
    pub muted: StateValue<bool>,
    pub soloed: StateValue<bool>,
    pub balance: Option<StateValue<f32>>,
    pub eq: EqState,
    pub limiter: LimiterState,
    pub delay_ticks: StateValue<u32>,
    pub meter: MeterState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxState {
    pub id: FxId,
    pub preset: StateValue<u8>,
    pub parameters: [StateValue<u8>; 3],
    pub master_level: StateValue<f32>,
    pub pan: StateValue<f32>,
    pub muted: StateValue<bool>,
    pub return_to_main: StateValue<bool>,
    pub return_to_mon1: StateValue<bool>,
    pub return_to_mon2: StateValue<bool>,
    pub meter: MeterState,
    pub aux_gains_db: [StateValue<f32>; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsbAudioEndpointState {
    pub id: UsbAudioEndpointId,
    pub active: StateValue<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadphoneRoutingState {
    pub source: StateValue<HeadphoneSource>,
    pub tap_point: StateValue<TapPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorLinkState {
    pub stereo_linked: StateValue<bool>,
    pub propagation_evidence: EvidenceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorRoutingSource {
    MonitorMix,
    Usb12,
    Usb34,
    Unknown(u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSettingsState {
    pub bt_usb_switch: StateValue<bool>,
    pub phones_only: StateValue<bool>,
    pub footswitch_fx_mode: StateValue<bool>,
    pub device_name: StateValue<String>,
    pub usb_streaming: StateValue<bool>,
    pub monitor_routing: StateValue<MonitorRoutingSource>,
    pub input56_from_usb12: StateValue<bool>,
    pub input78_from_usb34: StateValue<bool>,
    pub main_minus_10_dbv: StateValue<bool>,
    pub monitor_minus_10_dbv: StateValue<bool>,
    pub snapshot_scope_bits: StateValue<u8>,
    pub monitor_post_fader: StateValue<bool>,
    pub device_linked_selection: StateValue<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSlotState {
    pub slot: u8,
    pub name: StateValue<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotState {
    pub device_slots: Vec<SnapshotSlotState>,
    pub last_loaded: StateValue<Option<u8>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingState {
    pub usb_audio: [UsbAudioEndpointState; 2],
    pub headphones: HeadphoneRoutingState,
    pub monitor_link: MonitorLinkState,
    pub settings: DeviceSettingsState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_preset_catalog_has_sixteen_distinct_slots_per_engine() {
        for fx in [FxId::Fx1, FxId::Fx2] {
            for id in 0..FX_PRESET_COUNT as u8 {
                let info = fx_preset_info(fx, id).expect("all 16 slots have a name");
                assert_eq!(info.id, id);
                assert!(!info.name.is_empty());
                assert_eq!(info.evidence, EvidenceStatus::Inferred);
            }
            assert!(fx_preset_info(fx, FX_PRESET_COUNT as u8).is_none());
        }
    }

    #[test]
    fn device_observation_clears_pending_even_when_value_differs() {
        let mut value = StateValue::confirmed(1.0_f32, EvidenceStatus::Synthetic);
        value.set_pending(2.0);
        value.observe(1.5, EvidenceStatus::VerifiedFromDevice);
        assert_eq!(value.confirmed, Some(1.5));
        assert_eq!(value.pending, None);
        assert_eq!(value.evidence, EvidenceStatus::VerifiedFromDevice);
    }

    #[test]
    fn capability_profile_keeps_phantom_on_inputs_one_and_two_only() {
        assert!(InputCapabilities::for_input(InputId::Input1).phantom);
        assert!(InputCapabilities::for_input(InputId::Input2).phantom);
        assert!(!InputCapabilities::for_input(InputId::Input3).phantom);
        assert!(!InputCapabilities::for_input(InputId::UsbBluetooth).gain);
    }
}
