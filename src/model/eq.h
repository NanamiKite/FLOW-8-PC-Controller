#pragma once

#include "model/state_value.h"

#include <array>

namespace flow8::model {

inline constexpr std::array<double, 4> defaultChannelEqFrequenciesHz {
    80.0, 400.0, 2500.0, 10000.0};

inline constexpr std::array<double, 9> busEqFrequenciesHz {
    62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0};

struct EqState {
    std::array<StateValue<double>, 4> gainDb;
    std::array<StateValue<double>, 4> frequencyHz;
    std::array<StateValue<double>, 4> q;
};

struct BusEqState {
    std::array<StateValue<double>, 9> gainDb;
    std::array<StateValue<double>, 9> frequencyHz;
};

} // namespace flow8::model
