#include "midi/midi_parameter_map.h"

#include <array>

namespace flow8::midi {
namespace {

constexpr auto officialSource =
    "OfficialManual: Behringer FLOW 8 Quick Start Guide, MIDI Implementation, pp. 44-45";

std::optional<int> inputControlNumber(const Parameter parameter)
{
    switch (parameter) {
    case Parameter::EqLow: return 1;
    case Parameter::EqLowMid: return 2;
    case Parameter::EqHighMid: return 3;
    case Parameter::EqHigh: return 4;
    case Parameter::Mute: return 5;
    case Parameter::Solo: return 6;
    case Parameter::Level: return 7;
    case Parameter::Gain: return 8;
    case Parameter::LowCut: return 9;
    case Parameter::Balance: return 10;
    case Parameter::Compressor: return 11;
    case Parameter::Phantom48V: return 12;
    case Parameter::SendMonitor1: return 14;
    case Parameter::SendMonitor2: return 15;
    case Parameter::SendFx1: return 16;
    case Parameter::SendFx2: return 17;
    default: return std::nullopt;
    }
}

std::optional<int> busControlNumber(const Parameter parameter)
{
    switch (parameter) {
    case Parameter::Level: return 7;
    case Parameter::Limiter: return 8;
    case Parameter::Balance: return 10;
    case Parameter::BusEq62Hz: return 11;
    case Parameter::BusEq125Hz: return 12;
    case Parameter::BusEq250Hz: return 13;
    case Parameter::BusEq500Hz: return 14;
    case Parameter::BusEq1kHz: return 15;
    case Parameter::BusEq2kHz: return 16;
    case Parameter::BusEq4kHz: return 17;
    case Parameter::BusEq8kHz: return 18;
    case Parameter::BusEq16kHz: return 19;
    default: return std::nullopt;
    }
}

Mapping controlChange(const int channel, const int control, const Parameter parameter,
                      const int maximum = 127)
{
    return {
        .midiChannel = channel,
        .messageType = MessageType::ControlChange,
        .number = control,
        .minimum = 0,
        .maximum = maximum,
        .parameter = parameter,
        .source = QString::fromLatin1(officialSource),
    };
}

bool isBusEq(const Parameter parameter) noexcept
{
    return parameter >= Parameter::BusEq62Hz && parameter <= Parameter::BusEq16kHz;
}

} // namespace

int MidiParameterMap::channelForInput(const model::InputId input) noexcept
{
    return static_cast<int>(input);
}

int MidiParameterMap::channelForBus(const model::BusId bus) noexcept
{
    return 7 + static_cast<int>(bus);
}

int MidiParameterMap::channelForFxEngine(const int engineIndex) noexcept
{
    return engineIndex >= 0 && engineIndex < 2 ? 13 + engineIndex : -1;
}

int MidiParameterMap::globalChannel() noexcept
{
    return 15;
}

std::optional<Mapping> MidiParameterMap::input(const model::InputId input,
                                                const Parameter parameter)
{
    const auto control = inputControlNumber(parameter);
    if (!control.has_value()) {
        return std::nullopt;
    }
    if (input == model::InputId::UsbBluetooth
        && (parameter == Parameter::Gain || parameter == Parameter::LowCut
            || parameter == Parameter::Compressor || parameter == Parameter::Phantom48V)) {
        return std::nullopt;
    }
    if (parameter == Parameter::Phantom48V
        && input != model::InputId::Input1 && input != model::InputId::Input2) {
        return std::nullopt;
    }
    const int maximum = parameter == Parameter::Compressor ? 100 : 127;
    return controlChange(channelForInput(input), *control, parameter, maximum);
}

std::optional<Mapping> MidiParameterMap::bus(const model::BusId bus,
                                              const Parameter parameter)
{
    const auto control = busControlNumber(parameter);
    if (!control.has_value()) {
        return std::nullopt;
    }
    const bool fxBus = bus == model::BusId::Fx1 || bus == model::BusId::Fx2;
    if (fxBus && (parameter == Parameter::Limiter || parameter == Parameter::Balance
                  || isBusEq(parameter))) {
        return std::nullopt;
    }
    if (parameter == Parameter::Balance && bus != model::BusId::Main) {
        return std::nullopt;
    }
    return controlChange(channelForBus(bus), *control, parameter);
}

std::optional<Mapping> MidiParameterMap::fx(const int engineIndex,
                                             const Parameter parameter)
{
    const int channel = channelForFxEngine(engineIndex);
    if (channel < 0) {
        return std::nullopt;
    }
    if (parameter == Parameter::FxPreset) {
        return Mapping {
            .midiChannel = channel,
            .messageType = MessageType::ProgramChange,
            .number = 0,
            .minimum = 1,
            .maximum = 16,
            .parameter = parameter,
            .source = QString::fromLatin1(officialSource),
        };
    }
    if (parameter == Parameter::FxParameter1) {
        return controlChange(channel, 1, parameter, 100);
    }
    if (parameter == Parameter::FxParameter2) {
        return controlChange(channel, 2, parameter);
    }
    return std::nullopt;
}

std::optional<Mapping> MidiParameterMap::global(const Parameter parameter)
{
    if (parameter == Parameter::SnapshotRecall) {
        return Mapping {
            .midiChannel = globalChannel(),
            .messageType = MessageType::ProgramChange,
            .number = 0,
            .minimum = 1,
            .maximum = 15,
            .parameter = parameter,
            .source = QString::fromLatin1(officialSource),
        };
    }
    if (parameter == Parameter::MixerReset) {
        return Mapping {
            .midiChannel = globalChannel(),
            .messageType = MessageType::ProgramChange,
            .number = 0,
            .minimum = 16,
            .maximum = 16,
            .parameter = parameter,
            .source = QString::fromLatin1(officialSource),
        };
    }
    if (parameter == Parameter::FxMute) {
        return controlChange(globalChannel(), 1, parameter);
    }
    if (parameter == Parameter::TapTempo) {
        return Mapping {
            .midiChannel = globalChannel(),
            .messageType = MessageType::NoteOn,
            .number = 0,
            .minimum = 1,
            .maximum = 127,
            .parameter = parameter,
            .source = QString::fromLatin1(officialSource),
        };
    }
    return std::nullopt;
}

} // namespace flow8::midi
