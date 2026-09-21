#include "ble/ble_device.h"
#include "ble/ble_transport.h"
#include "protocol/codec.h"
#include "protocol/packet.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QDateTime>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTextStream>
#include <QTimer>

#include <memory>

namespace {

void logTraffic(const QString& direction, const QBluetoothUuid& service,
                const QBluetoothUuid& characteristic, const QString& operation,
                const QByteArray& payload, const bool json, const bool decode)
{
    const QString timestamp = QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);
    const QString hex = QString::fromLatin1(flow8::protocol::toHexBytes(payload));
    QString decoded;
    if (decode && !payload.isEmpty()) {
        const auto parsed = flow8::protocol::parsePacket(payload);
        decoded = parsed.ok() ? flow8::protocol::packetTypeName(parsed.packet->type)
                              : QStringLiteral("unparsed: %1").arg(parsed.message);
    }
    QTextStream stream(stdout);
    if (json) {
        const QJsonObject object {
            {QStringLiteral("timestamp"), timestamp},
            {QStringLiteral("direction"), direction},
            {QStringLiteral("service"), service.toString(QUuid::WithoutBraces)},
            {QStringLiteral("characteristic"), characteristic.toString(QUuid::WithoutBraces)},
            {QStringLiteral("operation"), operation},
            {QStringLiteral("payload"), hex},
            {QStringLiteral("decoded"), decoded},
        };
        stream << QJsonDocument(object).toJson(QJsonDocument::Compact) << '\n';
    } else {
        stream << timestamp << ' ' << direction << " service="
               << service.toString(QUuid::WithoutBraces) << " characteristic="
               << characteristic.toString(QUuid::WithoutBraces) << " operation=" << operation
               << " payload=" << hex;
        if (!decoded.isEmpty()) {
            stream << " decoded=" << decoded;
        }
        stream << '\n';
    }
}

} // namespace

int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("flow8-ble-monitor"));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral(
        "Record complete BLE traffic for a discovered FLOW 8 candidate. No handshake is sent."));
    parser.addHelpOption();
    parser.addOption({QStringLiteral("json"), QStringLiteral("Emit newline-delimited JSON.")});
    parser.addOption({QStringLiteral("decode"),
                      QStringLiteral("Append conservative packet-type decoding.")});
    parser.addOption({QStringLiteral("timeout"), QStringLiteral("Discovery timeout in milliseconds."),
                      QStringLiteral("ms"), QStringLiteral("15000")});
    parser.process(app);

    bool validTimeout = false;
    const int timeout = parser.value(QStringLiteral("timeout")).toInt(&validTimeout);
    if (!validTimeout || timeout < 100) {
        QTextStream(stderr) << "Invalid --timeout value\n";
        return 2;
    }
    const bool json = parser.isSet(QStringLiteral("json"));
    const bool decode = parser.isSet(QStringLiteral("decode"));

    flow8::ble::BleTransport scanner;
    std::unique_ptr<flow8::ble::BleTransport> monitor;
    bool selected = false;
    int exitCode = 0;
    QObject::connect(&scanner, &flow8::ble::BleTransport::deviceDiscovered, &app,
                     [&](const QBluetoothDeviceInfo& info, const bool candidate) {
        if (!candidate || selected) {
            return;
        }
        selected = true;
        scanner.stopScan();
        QTextStream(stderr) << "Candidate: " << flow8::ble::BleDevice(info).toText() << '\n';
        monitor = std::make_unique<flow8::ble::BleTransport>(info);
        QObject::connect(monitor.get(), &flow8::ble::BleTransport::trafficEvent, &app,
                         [&](const QString& direction, const QBluetoothUuid& service,
                             const QBluetoothUuid& characteristic, const QString& operation,
                             const QByteArray& payload) {
            logTraffic(direction, service, characteristic, operation, payload, json, decode);
        });
        QObject::connect(monitor.get(), &flow8::Flow8Transport::errorOccurred, &app,
                         [&](const QString& message) {
                             exitCode = 1;
                             QTextStream(stderr) << message << '\n';
                         });
        monitor->connectTransport();
    });
    QObject::connect(&scanner, &flow8::ble::BleTransport::scanFinished, &app, [&] {
        if (!selected) {
            exitCode = 1;
            QTextStream(stderr) << "No FLOW 8 candidate discovered (BLOCKED: NEED_HARDWARE).\n";
            app.quit();
        }
    });
    QObject::connect(&scanner, &flow8::Flow8Transport::errorOccurred, &app,
                     [&](const QString& message) {
                         exitCode = 1;
                         QTextStream(stderr) << message << '\n';
                     });
    QTimer::singleShot(timeout + 1000, &app, [&] {
        if (!selected) {
            exitCode = 1;
            app.quit();
        }
    });
    QTimer::singleShot(0, &app, [&scanner, timeout] { scanner.startScan(timeout); });
    const int applicationExit = app.exec();
    return exitCode != 0 ? exitCode : applicationExit;
}
