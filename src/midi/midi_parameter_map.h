#pragma once

#include "model/bus.h"
#include "model/channel.h"

#include <QString>

#include <optional>

namespace flow8::midi {

enum class MessageType {
    ControlChange,
    ProgramChange,
    NoteOn,
};

enum class Parameter {
    Level,
    Balance,
    Mute,
    Solo,
    EqLow,
    EqLowMid,
    EqHighMid,
    EqHigh,
    Gain,
    LowCut,
    Compressor,
    Phantom48V,
    SendMonitor1,
    SendMonitor2,
    SendFx1,
    SendFx2,
    Limiter,
    BusEq62Hz,
    BusEq125Hz,
    BusEq250Hz,
    BusEq500Hz,
    BusEq1kHz,
    BusEq2kHz,
    BusEq4kHz,
    BusEq8kHz,
    BusEq16kHz,
    FxPreset,
    FxParameter1,
    FxParameter2,
    SnapshotRecall,
    MixerReset,
    FxMute,
    TapTempo,
};

struct Mapping {
    int midiChannel {}; // zero-based C++ representation
    MessageType messageType {MessageType::ControlChange};
    int number {};      // CC, program, or note number as applicable
    int minimum {};
    int maximum {127};
    Parameter parameter {Parameter::Level};
    QString source;
};

class MidiParameterMap final {
public:
    [[nodiscard]] static int channelForInput(model::InputId input) noexcept;
    [[nodiscard]] static int channelForBus(model::BusId bus) noexcept;
    [[nodiscard]] static int channelForFxEngine(int engineIndex) noexcept;
    [[nodiscard]] static int globalChannel() noexcept;

    [[nodiscard]] static std::optional<Mapping> input(model::InputId input,
                                                       Parameter parameter);
    [[nodiscard]] static std::optional<Mapping> bus(model::BusId bus,
                                                     Parameter parameter);
    [[nodiscard]] static std::optional<Mapping> fx(int engineIndex,
                                                    Parameter parameter);
    [[nodiscard]] static std::optional<Mapping> global(Parameter parameter);
};

} // namespace flow8::midi
