#pragma once

#include <QBluetoothDeviceInfo>
#include <QBluetoothUuid>
#include <QLowEnergyCharacteristic>
#include <QStringList>

namespace flow8::ble {

[[nodiscard]] QBluetoothUuid flow8ServiceUuid();
[[nodiscard]] QBluetoothUuid flow8CharacteristicUuid();
[[nodiscard]] QString flow8AdvertisedName();
[[nodiscard]] bool isFlow8Candidate(const QBluetoothDeviceInfo& device);
[[nodiscard]] QStringList characteristicPropertyNames(
    QLowEnergyCharacteristic::PropertyTypes properties);

} // namespace flow8::ble
