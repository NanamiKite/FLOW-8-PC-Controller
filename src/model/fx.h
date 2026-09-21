#pragma once

#include "model/state_value.h"

#include <QString>

namespace flow8::model {

struct FxState {
    int index {};
    StateValue<int> preset;
    StateValue<QString> presetName;
    StateValue<double> parameter1;
    StateValue<double> parameter2;
    StateValue<int> presetReferenceIndex;
    StateValue<double> parameter1Percent;
    StateValue<double> parameter2Percent;
};

} // namespace flow8::model
