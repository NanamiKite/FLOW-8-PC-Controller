#include "ble/ble_device.h"
#include "ble/ble_services.h"
#include "ble/ble_transport.h"
#include "core/flow8_device.h"
#include "model/channel.h"
#include "protocol/command_codec.h"
#include "protocol/command_stream_decoder.h"
#include "protocol/flow8_command.h"
#include "protocol/packet.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMetaObject>
#include <QPointer>
#include <QSaveFile>
#include <QTextStream>
#include <QTimer>

#include <array>
#include <iostream>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>
#include <thread>

namespace {

QString utcNow()
{
    return QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);
}

QString transportStateName(const flow8::Flow8Transport::State state)
{
    using State = flow8::Flow8Transport::State;
    switch (state) {
    case State::Disconnected: return QStringLiteral("Disconnected");
    case State::Scanning: return QStringLiteral("Scanning");
    case State::Connecting: return QStringLiteral("Connecting");
    case State::RequestingMtu: return QStringLiteral("RequestingMtu");
    case State::DiscoveringServices: return QStringLiteral("DiscoveringServices");
    case State::WaitingForHandshake: return QStringLiteral("WaitingForHandshake");
    case State::WaitingForHandshakeReply: return QStringLiteral("WaitingForHandshakeReply");
    case State::Connected: return QStringLiteral("Connected");
    case State::Reconnecting: return QStringLiteral("Reconnecting");
    case State::Error: return QStringLiteral("Error");
    }
    return QStringLiteral("Unknown");
}

QString deviceStateName(const flow8::ConnectionState state)
{
    using State = flow8::ConnectionState;
    switch (state) {
    case State::Disconnected: return QStringLiteral("Disconnected");
    case State::Scanning: return QStringLiteral("Scanning");
    case State::Connecting: return QStringLiteral("Connecting");
    case State::Authenticating: return QStringLiteral("Authenticating");
    case State::Synchronizing: return QStringLiteral("Synchronizing");
    case State::Connected: return QStringLiteral("Connected");
    case State::Ready: return QStringLiteral("Ready");
    case State::Error: return QStringLiteral("Error");
    }
    return QStringLiteral("Unknown");
}

QString uuidText(const QBluetoothUuid& uuid)
{
    return uuid.toString(QUuid::WithoutBraces);
}

class HardwareLogger final {
public:
    explicit HardwareLogger(QString outputPath)
        : outputPath_(std::move(outputPath))
        , file_(outputPath_)
    {
    }

    bool open(QString& error)
    {
        const QFileInfo info(outputPath_);
        if (!QDir().mkpath(info.absolutePath())) {
            error = QStringLiteral("cannot create log directory: %1").arg(info.absolutePath());
            return false;
        }
        if (!file_.open(QIODevice::WriteOnly | QIODevice::Text | QIODevice::NewOnly)) {
            error = file_.errorString();
            return false;
        }
        return true;
    }

    void record(QJsonObject object)
    {
        if (!object.contains(QStringLiteral("timestamp"))) {
            object.insert(QStringLiteral("timestamp"), utcNow());
        }
        const QByteArray line = QJsonDocument(object).toJson(QJsonDocument::Compact) + '\n';
        QTextStream out(stdout);
        out << QString::fromUtf8(line);
        out.flush();
        file_.write(line);
        file_.flush();
    }

    [[nodiscard]] const QString& outputPath() const noexcept { return outputPath_; }

private:
    QString outputPath_;
    QFile file_;
};

bool saveTextFile(const QString& path, const QByteArray& data, QString& error)
{
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text)) {
        error = file.errorString();
        return false;
    }
    if (file.write(data) != data.size() || !file.commit()) {
        error = file.errorString();
        return false;
    }
    return true;
}

class MixerStateCapture final {
public:
    MixerStateCapture(QString directory, QString firmware, HardwareLogger& logger)
        : directory_(std::move(directory))
        , firmware_(std::move(firmware))
        , logger_(logger)
    {
    }

    void setDevice(const QBluetoothDeviceInfo& info)
    {
        device_ = flow8::ble::BleDevice(info);
    }

    void accept(const QByteArray& raw)
    {
        const auto parsed = flow8::protocol::parsePacket(raw);
        if (parsed.ok() && parsed.packet->type == 0x38) {
            if (parsed.packet->fragmentCount > 1) {
                const quint8 sequence = parsed.packet->sequenceId.value_or(0);
                if (!currentSequence_.has_value() || *currentSequence_ != sequence) {
                    frames_.clear();
                    currentSequence_ = sequence;
                }
            } else {
                frames_.clear();
                currentSequence_.reset();
            }
            // Preserve arrival order as well as the raw fragment header. The
            // protocol reassembler independently validates order/count.
            frames_.append(raw);
        }

        const auto decoded = decoder_.accept(raw);
        if (!decoded.ok()) {
            if (!decoded.awaitingFragments && !decoded.message.isEmpty()) {
                logger_.record({
                    {QStringLiteral("event"), QStringLiteral("protocol_decode_error")},
                    {QStringLiteral("message"), decoded.message},
                    {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE_RAW_ONLY")},
                });
            }
            return;
        }

        logger_.record({
            {QStringLiteral("event"), QStringLiteral("protocol_command_decoded")},
            {QStringLiteral("command"),
             flow8::protocol::commandSemanticName(decoded.command->value)},
            {QStringLiteral("command_id"),
             QStringLiteral("0x%1").arg(flow8::protocol::commandId(decoded.command->value),
                                       2, 16, QLatin1Char('0'))},
            {QStringLiteral("payload_size"), decoded.command->payload.size()},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
        });

        if (const auto* host =
                std::get_if<flow8::protocol::HandshakeHostCommand>(&decoded.command->value)) {
            QByteArray deviceId;
            deviceId.reserve(16);
            for (const quint8 byte : host->deviceId) {
                deviceId.append(static_cast<char>(byte));
            }
            logger_.record({
                {QStringLiteral("event"), QStringLiteral("handshake_host")},
                {QStringLiteral("device_id_hex"), QString::fromLatin1(deviceId.toHex())},
                {QStringLiteral("pairing_any"), host->pairingAny},
                {QStringLiteral("protocol_version"), host->protocolVersion},
                {QStringLiteral("firmware_build"), host->firmwareBuild},
                {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
            });
        }

        if (std::holds_alternative<flow8::protocol::MixerStateCommand>(
                decoded.command->value)) {
            saveMixerState(decoded.command->payload);
            frames_.clear();
            currentSequence_.reset();
        }
    }

private:
    void saveMixerState(const QByteArray& payload)
    {
        if (directory_.isEmpty()) {
            logger_.record({
                {QStringLiteral("event"), QStringLiteral("mixer_state_complete")},
                {QStringLiteral("payload_size"), payload.size()},
                {QStringLiteral("frame_count"), frames_.size()},
                {QStringLiteral("capture_saved"), false},
                {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
            });
            return;
        }

        if (!QDir().mkpath(directory_)) {
            logger_.record({
                {QStringLiteral("event"), QStringLiteral("capture_error")},
                {QStringLiteral("message"),
                 QStringLiteral("cannot create capture directory: %1").arg(directory_)},
            });
            return;
        }

        const QString stamp =
            QDateTime::currentDateTimeUtc().toString(QStringLiteral("yyyyMMdd-HHmmss-zzz"));
        const QString base = QDir(directory_).filePath(
            QStringLiteral("mixer-state-%1").arg(stamp));

        QByteArray frameText;
        QJsonArray frameSizes;
        QJsonArray frameHex;
        for (int index = 0; index < frames_.size(); ++index) {
            const QByteArray hex = frames_.at(index).toHex();
            frameText += QByteArray::number(index) + ": " + hex + '\n';
            frameSizes.append(frames_.at(index).size());
            frameHex.append(QString::fromLatin1(hex));
        }

        QJsonObject metadata {
            {QStringLiteral("timestamp"), utcNow()},
            {QStringLiteral("source"), QStringLiteral("hardware capture")},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
            {QStringLiteral("platform"), QStringLiteral("Windows 11 / Qt Bluetooth")},
            {QStringLiteral("device_name"), device_.info().name()},
            {QStringLiteral("device_identifier"), device_.identifier()},
            {QStringLiteral("firmware_note"), firmware_},
            {QStringLiteral("command"), QStringLiteral("0x38")},
            {QStringLiteral("frame_count"), frames_.size()},
            {QStringLiteral("frame_sizes"), frameSizes},
            {QStringLiteral("raw_frames_hex"), frameHex},
            {QStringLiteral("reassembled_payload_size"), payload.size()},
            {QStringLiteral("reassembled_payload_hex"),
             QString::fromLatin1(payload.toHex())},
        };

        QString error;
        const bool framesOk = saveTextFile(base + QStringLiteral(".frames.hex"),
                                           frameText, error);
        const bool payloadOk = framesOk && saveTextFile(
            base + QStringLiteral(".payload.hex"), payload.toHex() + '\n', error);
        const QByteArray metadataBytes =
            QJsonDocument(metadata).toJson(QJsonDocument::Indented);
        const bool metadataOk = payloadOk && saveTextFile(
            base + QStringLiteral(".json"), metadataBytes, error);

        logger_.record({
            {QStringLiteral("event"),
             metadataOk ? QStringLiteral("mixer_state_capture_saved")
                        : QStringLiteral("capture_error")},
            {QStringLiteral("base_path"), base},
            {QStringLiteral("payload_size"), payload.size()},
            {QStringLiteral("frame_count"), frames_.size()},
            {QStringLiteral("message"), metadataOk ? QString() : error},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
        });
    }

    QString directory_;
    QString firmware_;
    HardwareLogger& logger_;
    flow8::ble::BleDevice device_;
    flow8::protocol::CommandStreamDecoder decoder_;
    QVector<QByteArray> frames_;
    std::optional<quint8> currentSequence_;
};

std::optional<flow8::model::RoutingDestination> destinationFromText(QString value)
{
    value = value.trimmed().toUpper();
    if (value == QStringLiteral("MAIN")) return flow8::model::RoutingDestination::Main;
    if (value == QStringLiteral("MON1")) return flow8::model::RoutingDestination::Monitor1;
    if (value == QStringLiteral("MON2")) return flow8::model::RoutingDestination::Monitor2;
    if (value == QStringLiteral("FX1")) return flow8::model::RoutingDestination::Fx1;
    if (value == QStringLiteral("FX2")) return flow8::model::RoutingDestination::Fx2;
    return std::nullopt;
}

template<typename Command>
bool sendTyped(flow8::ble::BleTransport& transport, const Command& command,
               const QString& semantic, HardwareLogger& logger)
{
    const auto encoded =
        flow8::protocol::encodeCommand(flow8::protocol::Flow8Command {command});
    if (!encoded.ok()) {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("typed_command_rejected")},
            {QStringLiteral("semantic"), semantic},
            {QStringLiteral("message"), encoded.message},
        });
        return false;
    }
    for (const auto& packet : encoded.packets) {
        if (!transport.send(packet)) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("transport_write_rejected")},
                {QStringLiteral("semantic"), semantic},
                {QStringLiteral("raw_hex"), QString::fromLatin1(packet.toHex())},
            });
            return false;
        }
    }
    logger.record({
        {QStringLiteral("event"), QStringLiteral("typed_command_queued")},
        {QStringLiteral("semantic"), semantic},
        {QStringLiteral("packet_count"), encoded.packets.size()},
        {QStringLiteral("evidence"), QStringLiteral("PC_TX_ATTEMPT")},
    });
    return true;
}

void printCommands()
{
    QTextStream out(stderr);
    out << "Commands:\n"
        << "  status\n"
        << "  notify on|off\n"
        << "  read\n"
        << "  show <input 1..7> <MAIN|MON1|MON2|FX1|FX2>\n"
        << "  handshake <32 hex chars client-id>\n"
        << "  request-state\n"
        << "  route <input 1..7> <MAIN|MON1|MON2|FX1|FX2> <0.0..1.0>\n"
        << "  gain <input 1..6> <-20..60 dB>\n"
        << "  mute <input 1..7> <on|off>\n"
        << "  quit\n"
        << "No command is sent automatically in manual mode.\n";
    out.flush();
}

} // namespace

int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("flow8-hardware-bringup"));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral(
        "Staged FLOW 8 hardware bring-up. Default mode is observe-only and manual."));
    parser.addHelpOption();
    parser.addOption({QStringLiteral("output"),
                      QStringLiteral("Required new JSONL hardware log file."),
                      QStringLiteral("file")});
    parser.addOption({QStringLiteral("capture-dir"),
                      QStringLiteral("Directory for raw 0x38 capture artifacts."),
                      QStringLiteral("directory")});
    parser.addOption({QStringLiteral("firmware"),
                      QStringLiteral("Firmware note stored with captures."),
                      QStringLiteral("value"), QStringLiteral("unknown")});
    parser.addOption({QStringLiteral("adapter"),
                      QStringLiteral("Bluetooth adapter note stored in the session log."),
                      QStringLiteral("value"), QStringLiteral("unknown")});
    parser.addOption({QStringLiteral("device"),
                      QStringLiteral("Optional exact device name or Windows identifier filter."),
                      QStringLiteral("value")});
    parser.addOption({QStringLiteral("timeout"),
                      QStringLiteral("BLE scan timeout in milliseconds."),
                      QStringLiteral("ms"), QStringLiteral("15000")});
    parser.addOption({QStringLiteral("auto-notify"),
                      QStringLiteral("Subscribe immediately after GATT discovery.")});
    parser.addOption({QStringLiteral("auto-state-recovery"),
                      QStringLiteral("Automatically send 0x37 after a 0x36 reply.")});
    parser.addOption({QStringLiteral("write-mode"),
                      QStringLiteral("automatic, with-response, or without-response."),
                      QStringLiteral("mode"), QStringLiteral("automatic")});
    parser.process(app);

    if (!parser.isSet(QStringLiteral("output"))) {
        QTextStream(stderr) << "--output is required; hardware runs must preserve raw bytes.\n";
        return 2;
    }

    bool timeoutOk = false;
    const int timeout = parser.value(QStringLiteral("timeout")).toInt(&timeoutOk);
    if (!timeoutOk || timeout < 100) {
        QTextStream(stderr) << "Invalid --timeout value.\n";
        return 2;
    }

    const QString writeMode = parser.value(QStringLiteral("write-mode")).toLower();
    flow8::ble::BleTransport::WritePreference writePreference;
    if (writeMode == QStringLiteral("automatic")) {
        writePreference = flow8::ble::BleTransport::WritePreference::Automatic;
    } else if (writeMode == QStringLiteral("with-response")) {
        writePreference = flow8::ble::BleTransport::WritePreference::WithResponse;
    } else if (writeMode == QStringLiteral("without-response")) {
        writePreference = flow8::ble::BleTransport::WritePreference::WithoutResponse;
    } else {
        QTextStream(stderr) << "Invalid --write-mode value.\n";
        return 2;
    }

    HardwareLogger logger(parser.value(QStringLiteral("output")));
    QString logError;
    if (!logger.open(logError)) {
        QTextStream(stderr) << "Cannot create log: " << logError << '\n';
        return 2;
    }

    logger.record({
        {QStringLiteral("event"), QStringLiteral("session_start")},
        {QStringLiteral("platform"), QStringLiteral("Windows 11 / Qt Bluetooth expected")},
        {QStringLiteral("adapter_note"), parser.value(QStringLiteral("adapter"))},
        {QStringLiteral("firmware_note"), parser.value(QStringLiteral("firmware"))},
        {QStringLiteral("automatic_notify"), parser.isSet(QStringLiteral("auto-notify"))},
        {QStringLiteral("automatic_state_recovery"),
         parser.isSet(QStringLiteral("auto-state-recovery"))},
        {QStringLiteral("write_mode"), writeMode},
        {QStringLiteral("hardware_result"), QStringLiteral("NOT_YET_OBSERVED")},
    });

    flow8::Flow8Device device;
    device.setAutomaticStateRecovery(parser.isSet(QStringLiteral("auto-state-recovery")));
    flow8::ble::BleTransport scanner;
    scanner.setAutomaticReconnect(false);
    flow8::ble::BleTransport* liveTransport = nullptr;
    bool selected = false;
    int exitCode = 0;
    const QString deviceFilter = parser.value(QStringLiteral("device"));
    MixerStateCapture mixerCapture(parser.value(QStringLiteral("capture-dir")),
                                   parser.value(QStringLiteral("firmware")), logger);

    QObject::connect(&device.state(), &flow8::Flow8State::connectionStateChanged, &app,
                     [&](const flow8::ConnectionState state) {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("device_state")},
            {QStringLiteral("state"), deviceStateName(state)},
        });
    });
    QObject::connect(&device.state(), &flow8::Flow8State::stateReset, &app, [&] {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("confirmed_state_reset")},
            {QStringLiteral("channels"), device.state().channels().size()},
            {QStringLiteral("buses"), device.state().buses().size()},
            {QStringLiteral("effects"), device.state().effects().size()},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
        });
    });
    QObject::connect(&device.state(), &flow8::Flow8State::channelChanged, &app,
                     [&](const int index) {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("confirmed_channel_changed")},
            {QStringLiteral("channel_index"), index},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
        });
    });
    QObject::connect(&device.state(), &flow8::Flow8State::busChanged, &app,
                     [&](const int index) {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("confirmed_bus_changed")},
            {QStringLiteral("bus_index"), index},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
        });
    });
    QObject::connect(&device, &flow8::Flow8Device::protocolError, &app,
                     [&](const QString& message) {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("protocol_error")},
            {QStringLiteral("message"), message},
        });
    });

    const auto startConnection = [&](const QBluetoothDeviceInfo& info) {
        mixerCapture.setDevice(info);
        auto transport = std::make_unique<flow8::ble::BleTransport>(info);
        liveTransport = transport.get();
        liveTransport->setAutomaticReconnect(false);
        liveTransport->setAutomaticNotificationSubscription(
            parser.isSet(QStringLiteral("auto-notify")));
        liveTransport->setWritePreference(writePreference);

        QObject::connect(liveTransport, &flow8::Flow8Transport::stateChanged, &app,
                         [&](const flow8::Flow8Transport::State state) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("transport_state")},
                {QStringLiteral("state"), transportStateName(state)},
            });
        });
        QObject::connect(liveTransport, &flow8::ble::BleTransport::serviceDiscovered, &app,
                         [&](const QBluetoothUuid& service) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("service_discovered")},
                {QStringLiteral("service_uuid"), uuidText(service)},
                {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
            });
        });
        QObject::connect(liveTransport,
                         &flow8::ble::BleTransport::characteristicDiscovered, &app,
                         [&](const QBluetoothUuid& service,
                             const QBluetoothUuid& characteristic,
                             const QLowEnergyCharacteristic::PropertyTypes properties) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("characteristic_discovered")},
                {QStringLiteral("service_uuid"), uuidText(service)},
                {QStringLiteral("characteristic_uuid"), uuidText(characteristic)},
                {QStringLiteral("properties"),
                 flow8::ble::characteristicPropertyNames(properties)
                     .join(QLatin1Char(','))},
                {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
            });
        });
        QObject::connect(liveTransport, &flow8::ble::BleTransport::descriptorDiscovered,
                         &app, [&](const QBluetoothUuid& service,
                                   const QBluetoothUuid& characteristic,
                                   const QBluetoothUuid& descriptor) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("descriptor_discovered")},
                {QStringLiteral("service_uuid"), uuidText(service)},
                {QStringLiteral("characteristic_uuid"), uuidText(characteristic)},
                {QStringLiteral("descriptor_uuid"), uuidText(descriptor)},
                {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
            });
        });
        QObject::connect(liveTransport, &flow8::ble::BleTransport::gattReady, &app, [&] {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("gatt_ready")},
                {QStringLiteral("service_uuid"),
                 uuidText(flow8::ble::flow8ServiceUuid())},
                {QStringLiteral("characteristic_uuid"),
                 uuidText(flow8::ble::flow8CharacteristicUuid())},
                {QStringLiteral("automatic_notify"),
                 parser.isSet(QStringLiteral("auto-notify"))},
            });
            if (!parser.isSet(QStringLiteral("auto-notify"))) {
                QTextStream(stderr)
                    << "GATT ready. Type 'notify on' to write CCCD when ready.\n";
            }
        });
        QObject::connect(liveTransport,
                         &flow8::ble::BleTransport::notificationSubscriptionChanged,
                         &app, [&](const bool enabled, const QString& mode) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("notification_subscription")},
                {QStringLiteral("enabled"), enabled},
                {QStringLiteral("mode"), mode},
                {QStringLiteral("evidence"), QStringLiteral("WINDOWS_GATT_CONFIRMED")},
            });
        });
        QObject::connect(liveTransport, &flow8::ble::BleTransport::negotiatedMtuChanged,
                         &app, [&](const int mtu) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("att_mtu")},
                {QStringLiteral("value"), mtu},
                {QStringLiteral("note"),
                 QStringLiteral("Qt backend observation; not an Android MTU assumption")},
            });
        });
        QObject::connect(liveTransport, &flow8::ble::BleTransport::trafficEvent, &app,
                         [&](const QString& direction, const QBluetoothUuid& service,
                             const QBluetoothUuid& characteristic,
                             const QString& operation, const QByteArray& payload) {
            QString decoded;
            const auto parsedPacket = flow8::protocol::parsePacket(payload);
            if (parsedPacket.ok()) {
                decoded = flow8::protocol::packetTypeName(parsedPacket.packet->type);
            } else if (!payload.isEmpty()) {
                decoded = QStringLiteral("unparsed: %1").arg(parsedPacket.message);
            }
            logger.record({
                {QStringLiteral("event"), QStringLiteral("gatt_traffic")},
                {QStringLiteral("direction"), direction},
                {QStringLiteral("operation"), operation},
                {QStringLiteral("service_uuid"), uuidText(service)},
                {QStringLiteral("characteristic_uuid"), uuidText(characteristic)},
                {QStringLiteral("raw_hex"), QString::fromLatin1(payload.toHex())},
                {QStringLiteral("decoded"), decoded},
                {QStringLiteral("evidence"),
                 direction == QStringLiteral("RX")
                     && (operation == QStringLiteral("notify")
                         || operation == QStringLiteral("read"))
                     ? QStringLiteral("VERIFIED_FROM_DEVICE")
                     : QStringLiteral("PC_OR_WINDOWS_GATT_EVENT")},
            });
            if (direction == QStringLiteral("RX")
                && (operation == QStringLiteral("notify")
                    || operation == QStringLiteral("read"))
                && !payload.isEmpty()) {
                mixerCapture.accept(payload);
            }
        });
        QObject::connect(liveTransport, &flow8::Flow8Transport::errorOccurred, &app,
                         [&](const QString& message) {
            exitCode = 1;
            logger.record({
                {QStringLiteral("event"), QStringLiteral("ble_error")},
                {QStringLiteral("message"), message},
            });
        });

        device.setTransport(std::move(transport));
        device.connectDevice();
    };

    QObject::connect(&scanner, &flow8::ble::BleTransport::deviceDiscovered, &app,
                     [&](const QBluetoothDeviceInfo& info, const bool candidate) {
        flow8::ble::BleDevice found(info);
        QJsonObject record = found.toJson();
        record.insert(QStringLiteral("event"), QStringLiteral("advertisement"));
        record.insert(QStringLiteral("evidence"),
                      QStringLiteral("VERIFIED_FROM_DEVICE"));
        logger.record(record);

        const bool filterMatches = deviceFilter.isEmpty()
            || found.identifier().compare(deviceFilter, Qt::CaseInsensitive) == 0
            || info.name().compare(deviceFilter, Qt::CaseInsensitive) == 0;
        if (selected || !candidate || !filterMatches) {
            return;
        }
        selected = true;
        scanner.stopScan();
        logger.record({
            {QStringLiteral("event"), QStringLiteral("candidate_selected")},
            {QStringLiteral("name"), info.name()},
            {QStringLiteral("identifier"), found.identifier()},
            {QStringLiteral("evidence"), QStringLiteral("VERIFIED_FROM_DEVICE")},
        });
        QTimer::singleShot(0, &app, [&, info] { startConnection(info); });
    });
    QObject::connect(&scanner, &flow8::ble::BleTransport::scanFinished, &app, [&] {
        logger.record({
            {QStringLiteral("event"), QStringLiteral("scan_finished")},
            {QStringLiteral("candidate_selected"), selected},
        });
        if (!selected) {
            exitCode = 1;
            QTimer::singleShot(0, &app, &QCoreApplication::quit);
        }
    });
    QObject::connect(&scanner, &flow8::Flow8Transport::errorOccurred, &app,
                     [&](const QString& message) {
        exitCode = 1;
        logger.record({
            {QStringLiteral("event"), QStringLiteral("scan_error")},
            {QStringLiteral("message"), message},
        });
    });

    auto handleCommand = [&](const QString& rawLine) {
        const QString line = rawLine.trimmed();
        if (line.isEmpty()) return;
        const QStringList parts = line.split(QLatin1Char(' '), Qt::SkipEmptyParts);
        const QString command = parts.constFirst().toLower();
        logger.record({
            {QStringLiteral("event"), QStringLiteral("operator_command")},
            {QStringLiteral("command"), command},
        });

        if (command == QStringLiteral("help")) {
            printCommands();
            return;
        }
        if (command == QStringLiteral("quit")) {
            app.quit();
            return;
        }
        if (liveTransport == nullptr) {
            QTextStream(stderr) << "No connected candidate yet.\n";
            return;
        }
        if (command == QStringLiteral("status")) {
            logger.record({
                {QStringLiteral("event"), QStringLiteral("status")},
                {QStringLiteral("transport_state"),
                 transportStateName(liveTransport->state())},
                {QStringLiteral("device_state"),
                 deviceStateName(device.state().connectionState())},
                {QStringLiteral("notifications"),
                 liveTransport->notificationsEnabled()},
                {QStringLiteral("mtu"), liveTransport->negotiatedMtu()},
                {QStringLiteral("write_mode"), writeMode},
            });
            return;
        }
        if (command == QStringLiteral("show") && parts.size() == 3) {
            bool inputOk = false;
            const int input = parts.at(1).toInt(&inputOk);
            const auto destination = destinationFromText(parts.at(2));
            const auto* channel = inputOk ? device.state().channel(input - 1) : nullptr;
            const auto* route = destination
                ? device.state().routeLevel(input - 1, *destination) : nullptr;
            if (input < 1 || input > 7 || channel == nullptr || route == nullptr) {
                QTextStream(stderr)
                    << "Usage: show <input 1..7> <MAIN|MON1|MON2|FX1|FX2>\n";
                return;
            }
            QJsonObject values {
                {QStringLiteral("event"), QStringLiteral("confirmed_values")},
                {QStringLiteral("input"), input},
                {QStringLiteral("destination"), parts.at(2).toUpper()},
            };
            if (channel->gainDb.value.has_value()) {
                values.insert(QStringLiteral("gain_db"), *channel->gainDb.value);
            }
            if (channel->muted.value.has_value()) {
                values.insert(QStringLiteral("muted"), *channel->muted.value);
            }
            if (route->confirmed.value.has_value()) {
                values.insert(QStringLiteral("route_confirmed"),
                              *route->confirmed.value);
            }
            if (route->pending.has_value()) {
                values.insert(QStringLiteral("route_pending"), *route->pending);
            }
            values.insert(QStringLiteral("route_evidence"),
                          flow8::model::evidenceStatusName(
                              route->confirmed.evidence).toString());
            logger.record(values);
            return;
        }
        if (command == QStringLiteral("notify") && parts.size() == 2) {
            const QString value = parts.at(1).toLower();
            if (value != QStringLiteral("on") && value != QStringLiteral("off")) {
                QTextStream(stderr) << "Usage: notify on|off\n";
                return;
            }
            if (!liveTransport->subscribeNotifications(value == QStringLiteral("on"))) {
                QTextStream(stderr) << "CCCD request rejected by current GATT state.\n";
            }
            return;
        }
        if (command == QStringLiteral("read")) {
            if (!liveTransport->read()) {
                QTextStream(stderr) << "Characteristic does not support read or is not ready.\n";
            }
            return;
        }
        if (command == QStringLiteral("handshake") && parts.size() == 2) {
            const QByteArray bytes = QByteArray::fromHex(parts.at(1).toLatin1());
            if (bytes.size() != 16 || parts.at(1).size() != 32) {
                QTextStream(stderr) << "Client ID must be exactly 16 bytes / 32 hex chars.\n";
                return;
            }
            flow8::protocol::HandshakeClientCommand handshake;
            for (int index = 0; index < 16; ++index) {
                handshake.clientId[static_cast<std::size_t>(index)] =
                    static_cast<quint8>(bytes.at(index));
            }
            if (sendTyped(*liveTransport, handshake,
                          QStringLiteral("HandshakeClient 0x39"), logger)) {
                liveTransport->markHandshakeClientSent();
            }
            return;
        }
        if (command == QStringLiteral("request-state") && parts.size() == 1) {
            if (!device.requestMixerState()) {
                QTextStream(stderr)
                    << "State request rejected by the current protocol state.\n";
            }
            return;
        }
        if (command == QStringLiteral("route") && parts.size() == 4) {
            bool sourceOk = false;
            bool levelOk = false;
            const int source = parts.at(1).toInt(&sourceOk);
            const auto destination = destinationFromText(parts.at(2));
            const double level = parts.at(3).toDouble(&levelOk);
            if (!sourceOk || source < 1 || source > 7 || !destination
                || !levelOk || level < 0.0 || level > 1.0) {
                QTextStream(stderr)
                    << "Usage: route <input 1..7> <MAIN|MON1|MON2|FX1|FX2> <0.0..1.0>\n";
                return;
            }
            if (!device.setRouteLevel(source - 1, *destination, level)) {
                QTextStream(stderr)
                    << "Route command rejected; complete 0x38 sync/Ready first.\n";
            }
            return;
        }
        if (command == QStringLiteral("gain") && parts.size() == 3) {
            bool inputOk = false;
            bool gainOk = false;
            const int input = parts.at(1).toInt(&inputOk);
            const double gainDb = parts.at(2).toDouble(&gainOk);
            if (!inputOk || input < 1 || input > 6 || !gainOk
                || gainDb < flow8::model::inputGainMinimumDb
                || gainDb > flow8::model::inputGainMaximumDb) {
                QTextStream(stderr) << "Usage: gain <input 1..6> <-20..60 dB>\n";
                return;
            }
            if (!device.setChannelGain(
                    input - 1, flow8::model::normalizedInputGainFromDb(gainDb))) {
                QTextStream(stderr)
                    << "Gain command rejected; complete 0x38 sync/Ready first.\n";
            }
            return;
        }
        if (command == QStringLiteral("mute") && parts.size() == 3) {
            bool inputOk = false;
            const int input = parts.at(1).toInt(&inputOk);
            const QString value = parts.at(2).toLower();
            if (!inputOk || input < 1 || input > 7
                || (value != QStringLiteral("on")
                    && value != QStringLiteral("off"))) {
                QTextStream(stderr) << "Usage: mute <input 1..7> <on|off>\n";
                return;
            }
            if (!device.setChannelMuted(input - 1, value == QStringLiteral("on"))) {
                QTextStream(stderr)
                    << "Mute command rejected; complete 0x38 sync/Ready first.\n";
            }
            return;
        }

        QTextStream(stderr) << "Unknown or malformed command. Type 'help'.\n";
    };

    QPointer<QCoreApplication> applicationGuard(&app);
    std::thread([applicationGuard, &handleCommand] {
        std::string line;
        while (std::getline(std::cin, line)) {
            if (!applicationGuard) {
                return;
            }
            const QString command = QString::fromUtf8(line);
            QMetaObject::invokeMethod(applicationGuard, [command, &handleCommand] {
                handleCommand(command);
            }, Qt::QueuedConnection);
        }
    }).detach();

    printCommands();
    QTimer::singleShot(0, &app, [&] { scanner.startScan(timeout); });
    const int applicationResult = app.exec();
    logger.record({
        {QStringLiteral("event"), QStringLiteral("session_end")},
        {QStringLiteral("exit_code"), exitCode != 0 ? exitCode : applicationResult},
    });
    return exitCode != 0 ? exitCode : applicationResult;
}
