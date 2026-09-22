#include "ble/ble_device.h"
#include "ble/ble_services.h"
#include "ble/ble_transport.h"

#include <QBluetoothAddress>
#include <QSignalSpy>
#include <QTest>

class BleServicesTest final : public QObject {
    Q_OBJECT

private slots:
    void constantsKeepApkAndReferenceEvidenceSeparate();
    void candidateFilterAcceptsNameOrService();
    void propertiesAreRenderedWithoutLosingFlags();
    void transportPolicyIsExplicitAndOfflineSafe();
    void transportRejectsConnectWithoutDevice();
};

void BleServicesTest::constantsKeepApkAndReferenceEvidenceSeparate()
{
    QCOMPARE(flow8::ble::flow8AdvertisedName(), QStringLiteral("FLOW 8"));
    QCOMPARE(flow8::ble::flow8ApkDeviceName(), QStringLiteral("FLOW 8"));
    QCOMPARE(flow8::ble::flow8LegacyReferenceName(), QStringLiteral("FLOW 8 LE"));
    QCOMPARE(flow8::ble::flow8ApkRequestedMtu(), 255);
    QCOMPARE(flow8::ble::flow8ApkScanTimeoutMs(), 5'000);
    QCOMPARE(flow8::ble::flow8ServiceUuid().toString(QUuid::WithoutBraces),
             QStringLiteral("14839ad4-8d7e-415c-9a42-167340cf2339"));
    QCOMPARE(flow8::ble::flow8CharacteristicUuid().toString(QUuid::WithoutBraces),
             QStringLiteral("0034594a-a8e7-4b1a-a6b1-cd5243059a57"));
}

void BleServicesTest::candidateFilterAcceptsNameOrService()
{
    // SYNTHETIC metadata for testing the filter only; not a hardware capture.
    QBluetoothDeviceInfo byName(QBluetoothAddress(QStringLiteral("00:11:22:33:44:55")),
                                QStringLiteral("FLOW 8"), 0);
    QVERIFY(flow8::ble::isFlow8Candidate(byName));

    QBluetoothDeviceInfo byLegacyReferenceName(
        QBluetoothAddress(QStringLiteral("00:11:22:33:44:56")),
        QStringLiteral("FLOW 8 LE"), 0);
    QVERIFY(flow8::ble::isFlow8Candidate(byLegacyReferenceName));

    QBluetoothDeviceInfo byService(QBluetoothAddress(QStringLiteral("00:11:22:33:44:66")),
                                   QStringLiteral("unknown"), 0);
    byService.setServiceUuids({flow8::ble::flow8ServiceUuid()});
    QVERIFY(flow8::ble::isFlow8Candidate(byService));

    QBluetoothDeviceInfo other(QBluetoothAddress(QStringLiteral("00:11:22:33:44:77")),
                               QStringLiteral("Other"), 0);
    QVERIFY(!flow8::ble::BleDevice(other).isFlow8Candidate());
}

void BleServicesTest::propertiesAreRenderedWithoutLosingFlags()
{
    const auto names = flow8::ble::characteristicPropertyNames(
        QLowEnergyCharacteristic::Read | QLowEnergyCharacteristic::Notify
        | QLowEnergyCharacteristic::WriteNoResponse);
    QCOMPARE(names, QStringList({QStringLiteral("read"), QStringLiteral("write-without-response"),
                                 QStringLiteral("notify")}));
}

void BleServicesTest::transportPolicyIsExplicitAndOfflineSafe()
{
    flow8::ble::BleTransport transport;
    QVERIFY(transport.automaticNotificationSubscription());
    QVERIFY(!transport.notificationsEnabled());
    QCOMPARE(transport.negotiatedMtu(), 23);
    QCOMPARE(transport.writePreference(),
             flow8::ble::BleTransport::WritePreference::Automatic);

    transport.setAutomaticNotificationSubscription(false);
    transport.setWritePreference(
        flow8::ble::BleTransport::WritePreference::WithoutResponse);
    QVERIFY(!transport.automaticNotificationSubscription());
    QCOMPARE(transport.writePreference(),
             flow8::ble::BleTransport::WritePreference::WithoutResponse);
}

void BleServicesTest::transportRejectsConnectWithoutDevice()
{
    flow8::ble::BleTransport transport;
    QSignalSpy errors(&transport, &flow8::Flow8Transport::errorOccurred);
    transport.connectTransport();
    QCOMPARE(transport.state(), flow8::Flow8Transport::State::Error);
    QCOMPARE(errors.size(), 1);
    QVERIFY(!transport.send(QByteArray::fromHex("370138")));
}

QTEST_APPLESS_MAIN(BleServicesTest)
#include "ble_services_test.moc"
