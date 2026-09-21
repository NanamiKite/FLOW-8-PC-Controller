#pragma once

#include "model/capability.h"
#include "model/channel.h"
#include "model/eq.h"
#include "model/state_value.h"

#include <QString>

#include <optional>

namespace flow8::model {

enum class BusId {
    Main,
    Monitor1,
    Monitor2,
    Fx1,
    Fx2,
};

struct BusCapabilities {
    bool mute {};
    bool balance {};
    bool equalizer {};
    bool limiter {};
    bool fxEngine {};
    bool outputDelay {};
    CapabilityEvidence evidence;
};

struct OutputDelayState {
    StateValue<bool> enabled;
    StateValue<double> milliseconds;
};

struct BusState {
    int index {};
    BusId busId {BusId::Main};
    BusCapabilities capabilities;
    StateValue<QString> name;
    StateValue<double> fader;
    std::optional<StateValue<bool>> muted;
    StateValue<double> levelDb;
    std::optional<StateValue<double>> balance;
    std::optional<StateValue<double>> limiterDb;
    std::optional<BusEqState> eq;
    std::optional<OutputDelayState> outputDelay;
};

} // namespace flow8::model
