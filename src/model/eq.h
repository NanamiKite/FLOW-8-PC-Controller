#pragma once

#include "model/state_value.h"

#include <array>

namespace flow8::model {

struct EqState {
    std::array<StateValue<double>, 4> gainDb;
};

struct GraphicEqState {
    std::array<StateValue<double>, 9> gainDb;
};

} // namespace flow8::model
