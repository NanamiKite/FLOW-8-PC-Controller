#include "ble/ble_device.h"
#include "ble/ble_services.h"
#include "ble/ble_transport.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QDateTime>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTextStream>
#include <QTimer>

#include <memory>

namespace {

void printObject(const QJsonObject& object, const bool json)
{
    QTextStream stream(stdout);
    if (json) {
        stream << QJsonDocument(object).toJson(QJsonDocument::Compact) << '\n';
        return;
    }
    const auto type = object.value(QStringLiteral("type")).toString();
    if (type == QStringLiteral("device")) {
        stream << object.value(QStringLiteral("text")).toString() << '\n';
    } else {
        stream << type << " service=" << object.value(QStringLiteral("service")).toString();
        if (object.contains(QStringLiteral("characteristic"))) {
            stream << " characteristic="
                   << object.value(QStringLiteral("characteristic")).toString()
                   << " properties=" << object.value(QStringLiteral("properties")).toString();
        }
        stream << '\n';
    }
}

} // namespace

int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("flow8-ble-scanner"));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral(
        "BLE/GATT inspection tool. FLOW 8 matches are reference-evidence candidates only."));
    parser.addHelpOption();
    parser.addOption({QStringLiteral("json"), QStringLiteral("Emit newline-delimited JSON.")});
    parser.addOption({QStringLiteral("no-inspect"),
                      QStringLiteral("Do not connect to the first FLOW 8 candidate.")});
    parser.addOption({QStringLiteral("timeout"), QStringLiteral("Scan timeout in milliseconds."),
                      QStringLiteral("ms"), QStringLiteral("10000")});
    parser.process(app);

    bool validTimeout = false;
    const int timeout = parser.value(QStringLiteral("timeout")).toInt(&validTimeout);
    if (!validTimeout || timeout < 100) {
        QTextStream(stderr) << "Invalid --timeout value\n";
        return 2;
    }
    const bool json = parser.isSet(QStringLiteral("json"));
    const bool inspect = !parser.isSet(QStringLiteral("no-inspect"));

    flow8::ble::BleTransport scanner;
    std::unique_ptr<flow8::ble::BleTransport> inspector;
    bool inspectionStarted = false;
    int exitCode = 0;

    QObject::connect(&scanner, &flow8::ble::BleTransport::deviceDiscovered, &app,
                     [&](const QBluetoothDeviceInfo& info, const bool candidate) {
        flow8::ble::BleDevice device(info);
        auto object = device.toJson();
        object.insert(QStringLiteral("type"), QStringLiteral("device"));
        object.insert(QStringLiteral("text"), device.toText());
        object.insert(QStringLiteral("timestamp"),
                      QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));
        printObject(object, json);

        if (!inspect || !candidate || inspectionStarted) {
            return;
        }
        inspectionStarted = true;
        scanner.stopScan();
        inspector = std::make_unique<flow8::ble::BleTransport>(info);
        inspector->setAutomaticReconnect(false);
        QObject::connect(inspector.get(), &flow8::ble::BleTransport::serviceDiscovered, &app,
                         [&](const QBluetoothUuid& uuid) {
            printObject({{QStringLiteral("type"), QStringLiteral("service" )},
                         {QStringLiteral("service"), uuid.toString(QUuid::WithoutBraces)}}, json);
        });
        QObject::connect(inspector.get(), &flow8::ble::BleTransport::characteristicDiscovered,
                         &app, [&](const QBluetoothUuid& service,
                                   const QBluetoothUuid& characteristic,
                                   QLowEnergyCharacteristic::PropertyTypes properties) {
            printObject({{QStringLiteral("type"), QStringLiteral("characteristic")},
                         {QStringLiteral("service"), service.toString(QUuid::WithoutBraces)},
                         {QStringLiteral("characteristic"),
                          characteristic.toString(QUuid::WithoutBraces)},
                         {QStringLiteral("properties"),
                          flow8::ble::characteristicPropertyNames(properties).join(QLatin1Char(','))}},
                        json);
        });
        QObject::connect(inspector.get(), &flow8::Flow8Transport::errorOccurred, &app,
                         [&](const QString& message) {
                             exitCode = 1;
                             QTextStream(stderr) << message << '\n';
                         });
        inspector->connectTransport();
    });
    QObject::connect(&scanner, &flow8::Flow8Transport::errorOccurred, &app,
                     [&](const QString& message) {
                         exitCode = 1;
                         QTextStream(stderr) << message << '\n';
                     });
    QObject::connect(&scanner, &flow8::ble::BleTransport::scanFinished, &app, [&] {
        if (!inspectionStarted) {
            app.quit();
        } else {
            QTimer::singleShot(5000, &app, &QCoreApplication::quit);
        }
    });
    QTimer::singleShot(0, &app, [&scanner, timeout] { scanner.startScan(timeout); });
    const int applicationExit = app.exec();
    return exitCode != 0 ? exitCode : applicationExit;
}
