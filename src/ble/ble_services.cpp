#include "ble/ble_services.h"

namespace flow8::ble {

QBluetoothUuid flow8ServiceUuid()
{
    // INFERRED from reference/flow-8-midi; BLOCKED: NEED_HARDWARE in this project.
    return QBluetoothUuid(QStringLiteral("14839ad4-8d7e-415c-9a42-167340cf2339"));
}

QBluetoothUuid flow8CharacteristicUuid()
{
    // INFERRED from reference/flow-8-midi; BLOCKED: NEED_HARDWARE in this project.
    return QBluetoothUuid(QStringLiteral("0034594a-a8e7-4b1a-a6b1-cd5243059a57"));
}

QString flow8AdvertisedName()
{
    // INFERRED from reference/flow-8-midi; BLOCKED: NEED_HARDWARE in this project.
    return QStringLiteral("FLOW 8 LE");
}

bool isFlow8Candidate(const QBluetoothDeviceInfo& device)
{
    if (device.name().compare(flow8AdvertisedName(), Qt::CaseInsensitive) == 0) {
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
