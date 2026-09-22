#pragma once

#include <QBluetoothDeviceInfo>
#include <QBluetoothUuid>
#include <QLowEnergyCharacteristic>
#include <QStringList>

namespace flow8::ble {

[[nodiscard]] QBluetoothUuid flow8ServiceUuid();
[[nodiscard]] QBluetoothUuid flow8CharacteristicUuid();
[[nodiscard]] QString flow8ApkDeviceName();
[[nodiscard]] QString flow8LegacyReferenceName();
[[nodiscard]] QString flow8AdvertisedName();
[[nodiscard]] constexpr int flow8ApkRequestedMtu() noexcept { return 255; }
[[nodiscard]] constexpr int flow8ApkScanTimeoutMs() noexcept { return 5'000; }
[[nodiscard]] bool isFlow8Candidate(const QBluetoothDeviceInfo& device);
[[nodiscard]] QStringList characteristicPropertyNames(
    QLowEnergyCharacteristic::PropertyTypes properties);

} // namespace flow8::ble
