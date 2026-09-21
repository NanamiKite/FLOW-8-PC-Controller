#include "support/capture_fixture.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QRegularExpression>

namespace flow8::test {
namespace {

std::optional<model::EvidenceStatus> parseEvidence(const QString& value)
{
    if (value == QStringLiteral("VERIFIED")) return model::EvidenceStatus::Verified;
    if (value == QStringLiteral("INFERRED")) return model::EvidenceStatus::Inferred;
    if (value == QStringLiteral("UNKNOWN")) return model::EvidenceStatus::Unknown;
    if (value == QStringLiteral("BLOCKED")) return model::EvidenceStatus::Blocked;
    return std::nullopt;
}

} // namespace

FixtureLoadResult loadCaptureFixture(const QString& manifestPath)
{
    QFile manifestFile(manifestPath);
    if (!manifestFile.open(QIODevice::ReadOnly)) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("cannot open manifest: %1")
                             .arg(manifestFile.errorString())};
    }
    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(manifestFile.readAll(), &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("invalid manifest JSON: %1")
                             .arg(parseError.errorString())};
    }

    const QJsonObject object = document.object();
    const QString id = object.value(QStringLiteral("id")).toString();
    const QString source = object.value(QStringLiteral("source")).toString();
    const QString description = object.value(QStringLiteral("description")).toString();
    const QString captureKind = object.value(QStringLiteral("capture_kind")).toString();
    const QString rawFileName = object.value(QStringLiteral("raw_file")).toString();
    const QString rawEncoding = object.value(QStringLiteral("raw_encoding")).toString();
    const QString expectedHash = object.value(QStringLiteral("sha256")).toString().toLower();
    const auto evidence = parseEvidence(object.value(QStringLiteral("evidence_status")).toString());
    if (object.value(QStringLiteral("schema_version")).toInt() != 1 || id.isEmpty()
        || source.isEmpty() || description.isEmpty() || captureKind.isEmpty()
        || rawFileName.isEmpty() || expectedHash.size() != 64 || !evidence.has_value()) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("manifest is missing required fixture metadata")};
    }
    if (rawEncoding != QStringLiteral("hex")) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("unsupported raw encoding: %1").arg(rawEncoding)};
    }

    const QFileInfo manifestInfo(manifestPath);
    QFile rawFile(manifestInfo.dir().filePath(rawFileName));
    if (!rawFile.open(QIODevice::ReadOnly)) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("cannot open raw fixture: %1")
                             .arg(rawFile.errorString())};
    }
    QString hex = QString::fromLatin1(rawFile.readAll());
    hex.remove(QRegularExpression(QStringLiteral("\\s")));
    if (hex.isEmpty() || (hex.size() % 2) != 0
        || !QRegularExpression(QStringLiteral("^[0-9A-Fa-f]+$")).match(hex).hasMatch()) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("raw fixture is not strict hexadecimal text")};
    }
    const QByteArray raw = QByteArray::fromHex(hex.toLatin1());
    const QString actualHash = QString::fromLatin1(
        QCryptographicHash::hash(raw, QCryptographicHash::Sha256).toHex());
    if (actualHash != expectedHash) {
        return {.fixture = std::nullopt,
                .error = QStringLiteral("raw fixture SHA-256 mismatch")};
    }

    return {
        .fixture = CaptureFixture {
            .id = id,
            .source = source,
            .description = description,
            .captureKind = captureKind,
            .evidence = *evidence,
            .raw = raw,
            .expected = object.value(QStringLiteral("expected")).toObject(),
        },
        .error = {},
    };
}

} // namespace flow8::test
