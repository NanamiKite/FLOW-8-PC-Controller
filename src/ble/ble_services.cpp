#include "ble/ble_services.h"

namespace flow8::ble {

QBluetoothUuid flow8ServiceUuid()
{
    // VERIFIED_FROM_APK; real GATT presence remains BLOCKED: NEED_HARDWARE.
    return QBluetoothUuid(QStringLiteral("14839ad4-8d7e-415c-9a42-167340cf2339"));
}

QBluetoothUuid flow8CharacteristicUuid()
{
    // VERIFIED_FROM_APK; real GATT presence remains BLOCKED: NEED_HARDWARE.
    return QBluetoothUuid(QStringLiteral("0034594a-a8e7-4b1a-a6b1-cd5243059a57"));
}

QString flow8ApkDeviceName()
{
    // VERIFIED_FROM_APK native constant. This is not an advertisement capture.
    return QStringLiteral("FLOW 8");
}

QString flow8LegacyReferenceName()
{
    // Retained only as an INFERRED reference-project candidate. The current
    // APK artifacts do not contain this string.
    return QStringLiteral("FLOW 8 LE");
}

QString flow8AdvertisedName()
{
    return flow8ApkDeviceName();
}

bool isFlow8Candidate(const QBluetoothDeviceInfo& device)
{
    if (device.name().compare(flow8ApkDeviceName(), Qt::CaseInsensitive) == 0
        || device.name().compare(flow8LegacyReferenceName(), Qt::CaseInsensitive) == 0) {
        return true;
    }
    return device.serviceUuids().contains(flow8ServiceUuid());
}

QStringList characteristicPropertyNames(const QLowEnergyCharacteristic::PropertyTypes properties)
{
    QStringList result;
    const auto add = [&result, properties](const QLowEnergyCharacteristic::PropertyType property,
                                           const char* name) {
        if (properties.testFlag(property)) {
            result.append(QString::fromLatin1(name));
        }
    };
    add(QLowEnergyCharacteristic::Broadcasting, "broadcast");
    add(QLowEnergyCharacteristic::Read, "read");
    add(QLowEnergyCharacteristic::WriteNoResponse, "write-without-response");
    add(QLowEnergyCharacteristic::Write, "write");
    add(QLowEnergyCharacteristic::Notify, "notify");
    add(QLowEnergyCharacteristic::Indicate, "indicate");
    add(QLowEnergyCharacteristic::WriteSigned, "signed-write");
    add(QLowEnergyCharacteristic::ExtendedProperty, "extended");
    if (result.isEmpty()) {
        result.append(QStringLiteral("unknown"));
    }
    return result;
}

} // namespace flow8::ble
