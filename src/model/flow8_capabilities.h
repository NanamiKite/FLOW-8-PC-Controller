#pragma once

#include "model/bus.h"
#include "model/channel.h"
#include "model/routing.h"
#include "model/signal_path.h"
#include "model/snapshot.h"

#include <QVector>

namespace flow8::model {

inline constexpr int conventionalMixerInputCount = 7;
inline constexpr int mixerDestinationCount = 5;
inline constexpr int mixBusCount = 3;
// Compatibility name; this is not the count of every FLOW 8 signal source.
inline constexpr int inputStripCount = conventionalMixerInputCount;
inline constexpr int fxEngineCount = 2;
inline constexpr int hardwareSnapshotSlotCount = 15;

[[nodiscard]] QVector<ChannelState> createOfficialInputProfile();
[[nodiscard]] QVector<BusState> createOfficialBusProfile();
[[nodiscard]] QVector<SignalSourceState> createSignalSourceProfile();
[[nodiscard]] QVector<PhysicalOutputState> createPhysicalOutputProfile();
[[nodiscard]] QVector<SnapshotState> createHardwareSnapshotProfile();
[[nodiscard]] RoutingState createRoutingProfile();

} // namespace flow8::model
