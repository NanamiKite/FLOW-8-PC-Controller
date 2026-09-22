#pragma once

#include "model/state_value.h"

namespace flow8::model {

struct CompressorState {
    // The official MIDI chart exposes a single COMP amount. The detailed
    // controls below are optional model/UI concepts and remain UNKNOWN for
    // hardware until a reliable source identifies their transport semantics.
    StateValue<double> amount;
    StateValue<double> thresholdDb;
    StateValue<double> ratio;
    StateValue<double> attackMs;
    StateValue<double> releaseMs;
    StateValue<double> makeupGainDb;
    StateValue<double> gainReductionDb;
};

} // namespace flow8::model
