#pragma once

#include "model/state_value.h"

#include <QByteArray>
#include <QDateTime>
#include <QMap>
#include <QString>

namespace flow8::model {

enum class SnapshotStorage {
    HardwareSlot,
    AppLibrary,
};

enum class SnapshotScope {
    Full,
    Fx,
    Channel,
    Main,
    Monitor,
    Routing,
};

struct SnapshotState {
    int index {};
    QString id;
    SnapshotStorage storage {SnapshotStorage::HardwareSlot};
    StateValue<QString> name;
    StateValue<QDateTime> timestamp;
    StateValue<SnapshotScope> scope;
    StateValue<QByteArray> data;
    QMap<QString, QString> metadata;
    StateValue<bool> shareable;
    StateValue<QString> minimumFirmware;
};

} // namespace flow8::model
