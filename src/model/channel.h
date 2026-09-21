#pragma once

#include "model/compressor.h"
#include "model/eq.h"
#include "model/state_value.h"

#include <QString>

namespace flow8::model {

struct ChannelState {
    int index {};
    StateValue<QString> name;
    StateValue<double> gain;
    StateValue<double> fader;
    StateValue<bool> muted;
    StateValue<bool> soloed;
    StateValue<double> pan;
    // Explicit engineering-unit observations from state dumps. These do not
    // overwrite the normalized live-control values above.
    StateValue<double> levelDb;
    StateValue<double> gainDb;
    StateValue<quint16> lowCutHz;
    StateValue<bool> phantom48V;
    std::array<StateValue<double>, 4> sendLevelDb;
    EqState eq;
    CompressorState compressor;
};

} // namespace flow8::model
