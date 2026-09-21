#include "core/flow8_state.h"
#include "protocol/codec.h"
#include "protocol/packet.h"
#include "protocol/sysex.h"
#include "support/capture_fixture.h"

#include <QJsonObject>
#include <QJsonDocument>
#include <QFile>
#include <QTest>

#include <cmath>

namespace {

QString fixturePath(const QString& relativePath)
{
    return QStringLiteral(FLOW8_TEST_FIXTURES_DIR) + QLatin1Char('/') + relativePath;
}

std::optional<double> parameterValue(const flow8::protocol::ParsedSysExState& parsed,
                                     const QString& path)
{
    for (const auto& parameter : parsed.parameters) {
        if (parameter.path == path) return parameter.value;
    }
    return std::nullopt;
}

std::optional<bool> flagValue(const flow8::protocol::ParsedSysExState& parsed,
                              const QString& path)
{
    for (const auto& flag : parsed.flags) {
        if (flag.path == path) return flag.value;
    }
    return std::nullopt;
}

} // namespace

class CaptureReplayTest final : public QObject {
    Q_OBJECT

private slots:
    void replaysReferencePackets_data();
    void replaysReferencePackets();
    void replaysSyntheticSysExIntoState();
    void refusesInvalidAndPartialDumps();
    void inferredReplayCannotOverwriteVerifiedState();
};

void CaptureReplayTest::replaysReferencePackets_data()
{
    QTest::addColumn<QString>("manifest");
    QTest::newRow("0x37 session start")
        << QStringLiteral("protocol/reference-session-start.json");
    QTest::newRow("0x39 authentication")
        << QStringLiteral("protocol/reference-authentication.json");
}

void CaptureReplayTest::replaysReferencePackets()
{
    QFETCH(QString, manifest);
    const auto loaded = flow8::test::loadCaptureFixture(fixturePath(manifest));
    QVERIFY2(loaded.ok(), qPrintable(loaded.error));
    QCOMPARE(loaded.fixture->captureKind, QStringLiteral("reference_capture"));
    QCOMPARE(loaded.fixture->evidence, flow8::model::EvidenceStatus::Inferred);

    const auto decoded = flow8::protocol::parsePacket(loaded.fixture->raw);
    QVERIFY2(decoded.ok(), qPrintable(decoded.message));
    const QJsonObject expected = loaded.fixture->expected;
    QCOMPARE(decoded.packet->type,
             static_cast<quint8>(expected.value(QStringLiteral("packet_type")).toInt()));
    QCOMPARE(decoded.packet->discriminator,
             static_cast<quint8>(expected.value(QStringLiteral("discriminator")).toInt()));
    QCOMPARE(decoded.packet->payload.toHex(),
             expected.value(QStringLiteral("payload_hex")).toString().toLatin1());
    QCOMPARE(flow8::protocol::hasValidChecksum(loaded.fixture->raw), true);
}

void CaptureReplayTest::replaysSyntheticSysExIntoState()
{
    const auto loaded = flow8::test::loadCaptureFixture(
        fixturePath(QStringLiteral("sysex/synthetic-reference-layout.json")));
    QVERIFY2(loaded.ok(), qPrintable(loaded.error));
    QCOMPARE(loaded.fixture->captureKind, QStringLiteral("synthetic"));
    QCOMPARE(loaded.fixture->raw.size(), flow8::protocol::referenceStateDumpByteCount);

    const auto parsed = flow8::protocol::parseReferenceStateDump(loaded.fixture->raw);
    QCOMPARE(parsed.completeness, flow8::protocol::DumpCompleteness::CompleteReferenceLayout);
    const QJsonObject expectedParameters =
        loaded.fixture->expected.value(QStringLiteral("parameters")).toObject();
    for (auto it = expectedParameters.constBegin(); it != expectedParameters.constEnd(); ++it) {
        const auto actual = parameterValue(parsed, it.key());
        QVERIFY2(actual.has_value(), qPrintable(it.key()));
        QVERIFY2(std::abs(*actual - it.value().toDouble()) < 0.00001, qPrintable(it.key()));
    }
    const QJsonObject expectedFlags =
        loaded.fixture->expected.value(QStringLiteral("flags")).toObject();
    for (auto it = expectedFlags.constBegin(); it != expectedFlags.constEnd(); ++it) {
        QCOMPARE(flagValue(parsed, it.key()), std::optional(it.value().toBool()));
    }

    flow8::Flow8State state;
    const auto result = state.applySysExState(parsed);
    QVERIFY2(result.applied, qPrintable(result.reason));
    QVERIFY(result.fieldsApplied > 0);
    const auto* channel = state.channel(0);
    QVERIFY(channel != nullptr);
    QCOMPARE(channel->levelDb.value, std::optional(-12.0));
    QCOMPARE(channel->pan.value, std::optional(0.25));
    QCOMPARE(channel->compressor.amount.value, std::optional(0.75));
    QVERIFY(channel->lowCutHz.has_value());
    QCOMPARE(channel->lowCutHz->value, std::optional<quint16>(120));
    QCOMPARE(channel->eq.gainDb[1].value, std::optional(-3.5));
    QCOMPARE(channel->sendLevelDb[0].value, std::optional(-18.0));
    QCOMPARE(channel->muted.value, std::optional(true));
    QCOMPARE(channel->soloed.value, std::optional(true));
    QVERIFY(channel->phantom48V.has_value());
    QCOMPARE(channel->phantom48V->value, std::optional(true));
    QCOMPARE(state.buses().at(0).levelDb.value, std::optional(-6.0));
    QVERIFY(state.buses().at(0).eq.has_value());
    QCOMPARE(state.buses().at(0).eq->gainDb[0].value, std::optional(4.5));
    QCOMPARE(state.effects().at(0).parameter1Percent.value, std::optional(75.0));
    QCOMPARE(state.effects().at(0).parameter2Percent.value, std::optional(25.0));
    QCOMPARE(state.effects().at(0).presetReferenceIndex.value, std::optional(2));

    QFile expectedStateFile(
        fixturePath(QStringLiteral("state/synthetic-reference-layout.expected.json")));
    QVERIFY(expectedStateFile.open(QIODevice::ReadOnly));
    const QJsonDocument expectedStateDocument =
        QJsonDocument::fromJson(expectedStateFile.readAll());
    QVERIFY(expectedStateDocument.isObject());
    QCOMPARE(expectedStateDocument.object().value(QStringLiteral("fixture")).toString(),
             loaded.fixture->id);
    QCOMPARE(expectedStateDocument.object()
                 .value(QStringLiteral("expected_state")).toObject().size(),
             14);
}

void CaptureReplayTest::refusesInvalidAndPartialDumps()
{
    const auto loaded = flow8::test::loadCaptureFixture(
        fixturePath(QStringLiteral("sysex/synthetic-reference-layout.json")));
    QVERIFY(loaded.ok());

    flow8::Flow8State state;
    flow8::model::ChannelState channel;
    channel.index = 0;
    state.replaceChannels({channel});
    QVERIFY(state.setChannelPan(0, 0.6, flow8::model::EvidenceStatus::Verified,
                                QStringLiteral("test trusted state")));

    QByteArray partial = loaded.fixture->raw.left(512);
    partial[partial.size() - 1] = static_cast<char>(0xF7);
    const auto partialParsed = flow8::protocol::parseReferenceStateDump(partial);
    QCOMPARE(partialParsed.completeness, flow8::protocol::DumpCompleteness::Partial);
    QVERIFY(!state.applySysExState(partialParsed).applied);
    QCOMPARE(state.channel(0)->pan.value, std::optional(0.6));
    QCOMPARE(state.channels().size(), 1);

    auto invalidParsed = flow8::protocol::parseReferenceStateDump(loaded.fixture->raw);
    invalidParsed.raw[0] = 0x00;
    QVERIFY(!state.applySysExState(invalidParsed).applied);
    QCOMPARE(state.channel(0)->pan.value, std::optional(0.6));
    QCOMPARE(state.channels().size(), 1);

    auto injected = flow8::protocol::parseReferenceStateDump(loaded.fixture->raw);
    injected.parameters.append(flow8::protocol::ExtractedParameter {
        .path = QStringLiteral("channel.0.pan"),
        .value = -0.9,
        .evidence = flow8::model::EvidenceStatus::Verified,
        .source = QStringLiteral("injected decoded field"),
        .offsets = {},
        .encodedWidth = 0,
        .encoding = {},
        .decodedType = {},
    });
    QVERIFY(state.applySysExState(injected).applied);
    QCOMPARE(state.channel(0)->pan.value, std::optional(0.6));
    QCOMPARE(state.channel(0)->pan.source, QStringLiteral("test trusted state"));
}

void CaptureReplayTest::inferredReplayCannotOverwriteVerifiedState()
{
    const auto loaded = flow8::test::loadCaptureFixture(
        fixturePath(QStringLiteral("sysex/synthetic-reference-layout.json")));
    QVERIFY(loaded.ok());
    const auto parsed = flow8::protocol::parseReferenceStateDump(loaded.fixture->raw);

    flow8::Flow8State state;
    flow8::model::ChannelState channel;
    channel.index = 0;
    state.replaceChannels({channel});
    QVERIFY(state.setChannelPan(0, 0.8, flow8::model::EvidenceStatus::Verified,
                                QStringLiteral("hardware capture placeholder for precedence test")));
    const auto result = state.applySysExState(parsed);
    QVERIFY(result.applied);
    QVERIFY(result.fieldsRejected > 0);
    QCOMPARE(state.channel(0)->pan.value, std::optional(0.8));
    QCOMPARE(state.channel(0)->pan.evidence, flow8::model::EvidenceStatus::Verified);
}

QTEST_GUILESS_MAIN(CaptureReplayTest)

#include "capture_replay_test.moc"
