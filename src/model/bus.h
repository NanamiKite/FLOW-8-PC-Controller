#pragma once

#include "model/capability.h"
#include "model/channel.h"
#include "model/eq.h"
#include "model/state_value.h"

#include <QString>

#include <optional>

namespace flow8::model {

// BusId remains a broad stable target identifier for MIDI/UI compatibility.
// Only values for which isMixBus() is true may appear in Flow8State::buses();
// FX1/FX2 are represented by FxState.
enum class BusId {
    Main,
    Monitor1,
    Monitor2,
    Fx1,
    Fx2,
};

[[nodiscard]] constexpr bool isMixBus(const BusId bus) noexcept
{
    return bus == BusId::Main || bus == BusId::Monitor1 || bus == BusId::Monitor2;
}

struct BusCapabilities {
    bool mute {};
    bool balance {};
    bool equalizer {};
    bool limiter {};
    bool outputDelay {};
    CapabilityEvidence evidence;
};

struct OutputDelayState {
    StateValue<bool> enabled;
    StateValue<double> milliseconds;
};

struct MixBusState {
    int index {};
    BusId busId {BusId::Main};
    BusCapabilities capabilities;
    StateValue<QString> name;
    StateValue<double> fader;
    std::optional<StateValue<bool>> muted;
    // Present in the compound output state. The APK exposes no confirmed
    // atomic output-solo setter, so this is receive-side state only for now.
    StateValue<bool> soloed;
    StateValue<double> levelDb;
    std::optional<StateValue<double>> balance;
    std::optional<StateValue<double>> limiterDb;
    std::optional<BusEqState> eq;
    std::optional<OutputDelayState> outputDelay;
};

// MON1 and MON2 remain two independent MixBusState objects. This object holds
// only their relationship. APK evidence confirms that the link setting exists,
// but not which parameters the device mirrors while linked.
struct MonitorLinkState {
    StateValue<bool> stereoLinked;
    EvidenceStatus propagationEvidence {EvidenceStatus::Unknown};
    QString propagationSource;
    CapabilityEvidence capabilityEvidence;
};

// Compatibility name for existing high-level APIs. The vector now contains
// only MAIN, MON1, and MON2; FX engines live in FxState.
using BusState = MixBusState;

} // namespace flow8::model
