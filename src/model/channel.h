#pragma once

#include "model/compressor.h"
#include "model/capability.h"
#include "model/eq.h"
#include "model/state_value.h"

#include <QString>

#include <array>
#include <optional>

namespace flow8::model {

enum class InputId {
    Input1,
    Input2,
    Input3,
    Input4,
    Input56,
    Input78,
    UsbBluetooth,
};

enum class InputType {
    Microphone,
    MicrophoneLine,
    StereoLinePair,
    UsbBluetooth,
};

enum class SpatialControl {
    Pan,
    Balance,
};

// This is an extensible desktop UI identifier, not a claim about the FLOW 8
// icon wire format or the complete icon catalogue used by FLOW Mix.
enum class ChannelIcon {
    None,
    Microphone,
    Instrument,
    GuitarBass,
    Playback,
};

enum class MonitorSendMode {
    PreFader,
    PostFader,
};

struct LowCutState {
    StateValue<bool> enabled;
    StateValue<double> frequencyHz;
};

struct MonitorSendState {
    StateValue<double> levelDb;
    StateValue<MonitorSendMode> mode;
};

struct InputCapabilities {
    bool gain {};
    bool phase {};
    bool lowCut {};
    bool phantom48V {};
    bool equalizer {true};
    bool compressor {};
    bool monitorSends {true};
    bool fxSends {true};
    CapabilityEvidence evidence;
};

struct ChannelState {
    int index {};
    InputId inputId {InputId::Input1};
    InputType inputType {InputType::Microphone};
    SpatialControl spatialControl {SpatialControl::Pan};
    bool stereoPair {};
    InputCapabilities capabilities;
    QString defaultLabel;
    StateValue<QString> name;
    StateValue<ChannelIcon> icon;
    StateValue<bool> visible;
    StateValue<double> gain;
    std::optional<StateValue<bool>> phaseInverted;
    StateValue<double> fader;
    StateValue<bool> muted;
    StateValue<bool> soloed;
    StateValue<double> pan;
    // Explicit engineering-unit observations from state dumps. These do not
    // overwrite the normalized live-control values above.
    StateValue<double> levelDb;
    StateValue<double> gainDb;
    std::optional<LowCutState> lowCut;
    // Retained as a conservative wire-decoder observation while reference
    // SysEx offsets are still INFERRED. Flow8State keeps it synchronized with
    // lowCut->frequencyHz when safe to do so.
    std::optional<StateValue<quint16>> lowCutHz;
    std::optional<StateValue<bool>> phantom48V;
    std::array<MonitorSendState, 2> monitorSends;
    std::array<StateValue<double>, 2> fxSendLevelDb;
    // Compatibility view used by the existing reference SysEx parser:
    // MON1, MON2, FX1, FX2.
    std::array<StateValue<double>, 4> sendLevelDb;
    StateValue<double> meterLevel;
    StateValue<double> meterPeak;
    StateValue<bool> clipping;
    EqState eq;
    CompressorState compressor;
};

} // namespace flow8::model
