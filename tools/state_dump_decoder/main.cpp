#include "model/evidence_status.h"
#include "protocol/sysex.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QCryptographicHash>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QTextStream>

int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("flow8-state-dump-decoder"));
    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral(
        "Inspect a binary or hex-text SysEx dump using only documented inferred offsets."));
    parser.addHelpOption();
    parser.addOption({QStringLiteral("hex"), QStringLiteral("Input file contains hexadecimal text.")});
    parser.addOption({QStringLiteral("json"), QStringLiteral("Emit JSON output.")});
    parser.addPositionalArgument(QStringLiteral("file"), QStringLiteral("SysEx dump file."));
    parser.process(app);
    if (parser.positionalArguments().size() != 1) {
        parser.showHelp(2);
    }

    QFile file(parser.positionalArguments().constFirst());
    if (!file.open(QIODevice::ReadOnly)) {
        QTextStream(stderr) << "Cannot open input: " << file.errorString() << '\n';
        return 2;
    }
    QByteArray bytes = file.readAll();
    if (parser.isSet(QStringLiteral("hex"))) {
        QString text = QString::fromUtf8(bytes);
        text.remove(QRegularExpression(QStringLiteral("[\\s:-]")));
        if (text.isEmpty() || (text.size() % 2) != 0
            || !QRegularExpression(QStringLiteral("^[0-9A-Fa-f]+$")).match(text).hasMatch()) {
            QTextStream(stderr) << "Input is not valid hexadecimal text\n";
            return 2;
        }
        bytes = QByteArray::fromHex(text.toLatin1());
    }

    const auto parsed = flow8::protocol::parseReferenceStateDump(bytes);
    QJsonArray names;
    for (const auto& name : parsed.channelNames) {
        names.append(name.has_value() ? QJsonValue(*name) : QJsonValue(QJsonValue::Null));
    }
    QJsonArray parameters;
    for (const auto& parameter : parsed.parameters) {
        QJsonArray offsets;
        for (const qsizetype offset : parameter.offsets) offsets.append(offset);
        parameters.append(QJsonObject {
            {QStringLiteral("path"), parameter.path},
            {QStringLiteral("value"), parameter.value},
            {QStringLiteral("evidence"),
             flow8::model::evidenceStatusName(parameter.evidence).toString()},
            {QStringLiteral("source"), parameter.source},
            {QStringLiteral("offsets"), offsets},
            {QStringLiteral("encodedWidth"), parameter.encodedWidth},
            {QStringLiteral("encoding"), parameter.encoding},
            {QStringLiteral("decodedType"), parameter.decodedType},
        });
    }
    QJsonArray flags;
    for (const auto& flag : parsed.flags) {
        QJsonArray offsets;
        for (const qsizetype offset : flag.offsets) offsets.append(offset);
        flags.append(QJsonObject {
            {QStringLiteral("path"), flag.path},
            {QStringLiteral("value"), flag.value},
            {QStringLiteral("evidence"),
             flow8::model::evidenceStatusName(flag.evidence).toString()},
            {QStringLiteral("source"), flag.source},
            {QStringLiteral("offsets"), offsets},
            {QStringLiteral("encodedWidth"), flag.encodedWidth},
            {QStringLiteral("encoding"), flag.encoding},
            {QStringLiteral("decodedType"), flag.decodedType},
        });
    }
    const QJsonObject object {
        {QStringLiteral("validSysEx"), parsed.validation.valid},
        {QStringLiteral("flow8Header"), parsed.validation.isFlow8},
        {QStringLiteral("message"), parsed.validation.message},
        {QStringLiteral("completeness"),
         flow8::protocol::dumpCompletenessName(parsed.completeness).toString()},
        {QStringLiteral("rawByteCount"), bytes.size()},
        {QStringLiteral("sha256"),
         QString::fromLatin1(QCryptographicHash::hash(bytes, QCryptographicHash::Sha256).toHex())},
        {QStringLiteral("channelNames"), names},
        {QStringLiteral("parameters"), parameters},
        {QStringLiteral("flags"), flags},
    };

    QTextStream out(stdout);
    if (parser.isSet(QStringLiteral("json"))) {
        out << QJsonDocument(object).toJson(QJsonDocument::Indented);
    } else {
        out << "valid=" << (parsed.validation.valid ? "yes" : "no")
            << " flow8Header=" << (parsed.validation.isFlow8 ? "yes" : "no")
            << " completeness=" << flow8::protocol::dumpCompletenessName(parsed.completeness)
            << " bytes=" << bytes.size() << " sha256="
            << object.value(QStringLiteral("sha256")).toString() << '\n'
            << parsed.validation.message << '\n';
        for (qsizetype index = 0; index < parsed.channelNames.size(); ++index) {
            if (parsed.channelNames[index].has_value()) {
                out << "channel." << index << ".name=" << *parsed.channelNames[index]
                    << " evidence=INFERRED\n";
            }
        }
        for (const auto& parameter : parsed.parameters) {
            out << parameter.path << '=' << parameter.value << " evidence="
                << flow8::model::evidenceStatusName(parameter.evidence) << '\n';
        }
        for (const auto& flag : parsed.flags) {
            out << flag.path << '=' << (flag.value ? "true" : "false") << " evidence="
                << flow8::model::evidenceStatusName(flag.evidence) << '\n';
        }
    }
    return parsed.validation.valid && parsed.validation.isFlow8 ? 0 : 1;
}
