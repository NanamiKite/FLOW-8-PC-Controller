#include "model/evidence_status.h"
#include "protocol/codec.h"
#include "protocol/packet.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QTextStream>

#include <optional>

namespace {

std::optional<QByteArray> parseHex(QString text)
{
    text.remove(QRegularExpression(QStringLiteral("[\\s:-]")));
    if (text.isEmpty() || (text.size() % 2) != 0
        || !QRegularExpression(QStringLiteral("^[0-9A-Fa-f]+$")).match(text).hasMatch()) {
        return std::nullopt;
    }
    return QByteArray::fromHex(text.toLatin1());
}

} // namespace

int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("flow8-packet-decoder"));
    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral(
        "Validate and conservatively decode FLOW 8 packet hex without hardware."));
    parser.addHelpOption();
    parser.addOption({QStringLiteral("json"), QStringLiteral("Emit newline-delimited JSON.")});
    parser.addPositionalArgument(QStringLiteral("hex"),
                                 QStringLiteral("One or more complete hex packets."),
                                 QStringLiteral("hex..."));
    parser.process(app);

    if (parser.positionalArguments().isEmpty()) {
        parser.showHelp(2);
    }
    const bool json = parser.isSet(QStringLiteral("json"));
    bool failed = false;
    QTextStream out(stdout);
    QTextStream error(stderr);
    for (const auto& argument : parser.positionalArguments()) {
        const auto bytes = parseHex(argument);
        if (!bytes.has_value()) {
            error << "Invalid hex input: " << argument << '\n';
            failed = true;
            continue;
        }
        const auto result = flow8::protocol::parsePacket(*bytes);
        QJsonObject object {
            {QStringLiteral("raw"),
             QString::fromLatin1(flow8::protocol::toHexBytes(*bytes))},
            {QStringLiteral("valid"), result.ok()},
            {QStringLiteral("error"), result.message},
        };
        if (result.ok()) {
            const auto& packet = *result.packet;
            object.insert(QStringLiteral("type"), static_cast<int>(packet.type));
            object.insert(QStringLiteral("typeHex"),
                          QStringLiteral("0x%1").arg(packet.type, 2, 16, QLatin1Char('0')));
            object.insert(QStringLiteral("name"), flow8::protocol::packetTypeName(packet.type));
            object.insert(QStringLiteral("fragmentCount"), static_cast<int>(packet.fragmentCount));
            if (packet.fragmentHeaderA.has_value()) {
                object.insert(QStringLiteral("fragmentHeaderA"),
                              static_cast<int>(*packet.fragmentHeaderA));
                object.insert(QStringLiteral("fragmentHeaderB"),
                              static_cast<int>(*packet.fragmentHeaderB));
            }
            object.insert(QStringLiteral("payload"),
                          QString::fromLatin1(flow8::protocol::toHexBytes(packet.payload)));
            object.insert(QStringLiteral("evidence"),
                          flow8::model::evidenceStatusName(packet.evidence).toString());
            object.insert(QStringLiteral("payloadEvidence"),
                          flow8::model::evidenceStatusName(packet.payloadEvidence).toString());
        } else {
            failed = true;
        }
        if (json) {
            out << QJsonDocument(object).toJson(QJsonDocument::Compact) << '\n';
        } else if (result.ok()) {
            out << object.value(QStringLiteral("typeHex")).toString() << ' '
                << object.value(QStringLiteral("name")).toString() << " fragments="
                << object.value(QStringLiteral("fragmentCount")).toInt() << " payload="
                << object.value(QStringLiteral("payload")).toString() << " evidence="
                << object.value(QStringLiteral("evidence")).toString()
                << " payloadEvidence="
                << object.value(QStringLiteral("payloadEvidence")).toString() << '\n';
        } else {
            out << "INVALID raw=" << object.value(QStringLiteral("raw")).toString()
                << " error=" << result.message << '\n';
        }
    }
    return failed ? 1 : 0;
}
