#pragma once

#include "model/eq.h"
#include "model/state_value.h"

#include <QString>

namespace flow8::model {

struct BusState {
    int index {};
    StateValue<QString> name;
    StateValue<double> fader;
    StateValue<double> pan;
    StateValue<double> limiter;
    StateValue<double> levelDb;
    StateValue<double> balance;
    StateValue<double> limiterDb;
    GraphicEqState eq;
};

struct MainState {
    StateValue<double> fader;
    StateValue<bool> muted;
    StateValue<double> pan;
};

struct MonitorState {
    int index {};
    StateValue<double> fader;
};

} // namespace flow8::model
