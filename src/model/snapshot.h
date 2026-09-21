#pragma once

#include "model/state_value.h"

#include <QString>

namespace flow8::model {

struct SnapshotState {
    int index {};
    StateValue<QString> name;
};

} // namespace flow8::model
