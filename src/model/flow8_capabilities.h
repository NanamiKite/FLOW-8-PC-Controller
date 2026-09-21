#pragma once

#include "model/bus.h"
#include "model/channel.h"
#include "model/routing.h"
#include "model/snapshot.h"

#include <QVector>

namespace flow8::model {

inline constexpr int inputStripCount = 7;
inline constexpr int busCount = 5;
inline constexpr int fxEngineCount = 2;
inline constexpr int hardwareSnapshotSlotCount = 15;

[[nodiscard]] QVector<ChannelState> createOfficialInputProfile();
[[nodiscard]] QVector<BusState> createOfficialBusProfile();
[[nodiscard]] QVector<SnapshotState> createHardwareSnapshotProfile();
[[nodiscard]] RoutingState createRoutingProfile();

} // namespace flow8::model
