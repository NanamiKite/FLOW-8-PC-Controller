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
};

} // namespace flow8::model
