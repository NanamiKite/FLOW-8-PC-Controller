#pragma once

#include <QBluetoothDeviceInfo>
#include <QJsonObject>
#include <QString>

namespace flow8::ble {

class BleDevice final {
public:
    explicit BleDevice(QBluetoothDeviceInfo info = {});

    [[nodiscard]] const QBluetoothDeviceInfo& info() const noexcept;
    [[nodiscard]] QString identifier() const;
    [[nodiscard]] bool isFlow8Candidate() const;
    [[nodiscard]] QJsonObject toJson() const;
    [[nodiscard]] QString toText() const;

private:
    QBluetoothDeviceInfo info_;
};

} // namespace flow8::ble
