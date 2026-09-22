#pragma once

#include "model/routing.h"
#include "model/state_value.h"

#include <QVector>

namespace flow8::model {

// High-rate observations are intentionally separate from control state. Meter
// updates are transient, are not snapshot data, and do not enter state history.
struct InputMeterState {
    int sourceIndex {};
    StateValue<double> level;
    StateValue<double> peak;
    StateValue<bool> clipping;
    StateValue<double> gainReductionDb;
    StateValue<quint8> gainReductionCode;
};

struct OutputMeterState {
    RoutingDestination destination {RoutingDestination::Main};
    StateValue<double> level;
    StateValue<double> peak;
    StateValue<bool> clipping;
    StateValue<double> gainReductionDb;
    StateValue<quint8> gainReductionCode;
};

struct MeterState {
    QVector<InputMeterState> inputs;
    QVector<OutputMeterState> outputs;
};

} // namespace flow8::model
