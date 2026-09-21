#pragma once

#include "model/state_value.h"

namespace flow8::model {

struct CompressorState {
    StateValue<double> amount;
};

} // namespace flow8::model
