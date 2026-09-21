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
    EqState eq;
    CompressorState compressor;
};

} // namespace flow8::model
