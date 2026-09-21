#include "ble/ble_device.h"

#include "ble/ble_services.h"

#include <QBluetoothAddress>
#include <QJsonArray>

namespace flow8::ble {

BleDevice::BleDevice(QBluetoothDeviceInfo info)
    : info_(std::move(info))
{
}

const QBluetoothDeviceInfo& BleDevice::info() const noexcept
{
    return info_;
}

QString BleDevice::identifier() const
{
    if (!info_.address().isNull()) {
        return info_.address().toString();
    }
    if (!info_.deviceUuid().isNull()) {
        return info_.deviceUuid().toString(QUuid::WithoutBraces);
    }
    return QStringLiteral("unknown");
}

bool BleDevice::isFlow8Candidate() const
{
    return ble::isFlow8Candidate(info_);
}

QJsonObject BleDevice::toJson() const
{
    QJsonArray services;
    for (const auto& uuid : info_.serviceUuids()) {
        services.append(uuid.toString(QUuid::WithoutBraces));
    }
    return {
        {QStringLiteral("name"), info_.name()},
        {QStringLiteral("identifier"), identifier()},
        {QStringLiteral("rssi"), info_.rssi()},
        {QStringLiteral("flow8Candidate"), isFlow8Candidate()},
        {QStringLiteral("advertisedServices"), services},
    };
}

QString BleDevice::toText() const
{
    QStringList services;
    for (const auto& uuid : info_.serviceUuids()) {
        services.append(uuid.toString(QUuid::WithoutBraces));
    }
    return QStringLiteral("name=%1 address=%2 rssi=%3 candidate=%4 services=[%5]")
        .arg(info_.name().isEmpty() ? QStringLiteral("<unnamed>") : info_.name(), identifier())
        .arg(info_.rssi())
        .arg(isFlow8Candidate() ? QStringLiteral("yes") : QStringLiteral("no"),
             services.join(QLatin1Char(',')));
}

} // namespace flow8::ble
