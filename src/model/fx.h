#pragma once

#include "model/state_value.h"

#include <QString>

#include <array>

namespace flow8::model {

struct FxParameterState {
    int index {};
    StateValue<QString> name;
    StateValue<double> value;
};

struct FxState {
    int index {};
    StateValue<double> master;
    StateValue<int> preset;
    StateValue<QString> presetName;
    StateValue<double> parameter1;
    StateValue<double> parameter2;
    StateValue<int> presetReferenceIndex;
    StateValue<double> parameter1Percent;
    StateValue<double> parameter2Percent;
    StateValue<QString> effectType;
    StateValue<bool> muted;
    StateValue<double> tapTempoBpm;
    std::array<FxParameterState, 2> parameters;
};

struct GlobalTempoState {
    StateValue<double> bpm;
};

inline constexpr int fxPresetCount = 16;

} // namespace flow8::model
