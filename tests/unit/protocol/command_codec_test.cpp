#include "core/flow8_state.h"
#include "protocol/command_codec.h"
#include "protocol/command_stream_decoder.h"
#include "protocol/codec.h"
#include "protocol/field_codec.h"
#include "protocol/packet.h"

#include <QSet>
#include <QTest>

#include <array>

namespace {

using flow8::protocol::Flow8Command;

void expectExact(const Flow8Command& command, const char* expectedHex)
{
    const auto encoded = flow8::protocol::encodeCommand(command);
    QVERIFY2(encoded.ok(), qPrintable(encoded.message));
    QCOMPARE(encoded.packets.size(), 1);
    QCOMPARE(encoded.packets.constFirst(), QByteArray::fromHex(expectedHex));

    const auto frame = flow8::protocol::parsePacket(encoded.packets.constFirst());
    QVERIFY2(frame.ok(), qPrintable(frame.message));
    const auto decoded = flow8::protocol::decodeCommandPayload(
        frame.packet->type, frame.packet->payload);
    QVERIFY2(decoded.ok(), qPrintable(decoded.message));
    QCOMPARE(flow8::protocol::commandId(decoded.command->value),
             flow8::protocol::commandId(command));
}

Flow8Command commandFromExactFrame(const char* expectedHex)
{
    const QByteArray frame = QByteArray::fromHex(expectedHex);
    const auto packet = flow8::protocol::parsePacket(frame);
    if (!packet.ok()) qFatal("invalid test vector frame");
    const auto decoded = flow8::protocol::decodeCommandPayload(
        packet.packet->type, packet.packet->payload);
    if (!decoded.ok()) {
        qFatal("invalid test vector payload for 0x%02x: %s",
               packet.packet->type, qPrintable(decoded.message));
    }
    return decoded.command->value;
}

flow8::protocol::MixerStateCommand validMixerState()
{
    flow8::protocol::MixerStateCommand state;
    for (std::size_t index = 0; index < state.inputs.size(); ++index) {
        state.inputs[index].id = static_cast<quint8>(index);
        state.inputs[index].label.endpoint = static_cast<quint8>(index);
    }
    state.outputs[0].id = 0x0f;
    state.outputs[1].id = 0x0a;
    state.outputs[2].id = 0x0b;
    state.effects[0].id = 0x0c;
    state.effects[1].id = 0x0d;
    return state;
}

} // namespace

class CommandCodecTest final : public QObject {
    Q_OBJECT

private slots:
    void primitiveFix8FormatsMatchApkExamples();
    void all31TargetCommandsMatchExactNativeVectors();
    void mixerStateMatchesExactNativeFragmentVector();
    void reassemblesMixerStateAndRejectsBrokenSequences();
    void appliesAtomicAndCompositeRxToOneConfirmedState();
};

void CommandCodecTest::primitiveFix8FormatsMatchApkExamples()
{
    using flow8::protocol::Fix8Format;
    using flow8::protocol::encodeFix8;
    QCOMPARE(encodeFix8(Fix8Format::Pan, 0.0), std::optional<quint8>(0x7f));
    QCOMPARE(encodeFix8(Fix8Format::UnitInterval, 0.5), std::optional<quint8>(0x7f));
    QCOMPARE(encodeFix8(Fix8Format::EqGainDb, 0.0), std::optional<quint8>(0x7f));
    QCOMPARE(encodeFix8(Fix8Format::GainDb, -30.0), std::optional<quint8>(0x3c));
    QCOMPARE(encodeFix8(Fix8Format::Q, 1.0), std::optional<quint8>(0x08));
    QCOMPARE(encodeFix8(Fix8Format::Q, 20.0), std::optional<quint8>(0x6f));
}

void CommandCodecTest::all31TargetCommandsMatchExactNativeVectors()
{
    using namespace flow8::protocol;
    using flow8::model::EndpointId;
    QSet<quint8> covered;
    const auto check = [&covered](const Flow8Command& command, const char* bytes) {
        expectExact(command, bytes);
        covered.insert(commandId(command));
    };

    check(PanCommand {0x04, 0.0}, "0001047f84");
    check(SoloCommand {0x00, true}, "0101000103");
    check(GainCommand {.inputEndpoint = EndpointId::Input1, .gainDb = 0.0},
          "020100787b");
    check(GraphicEqCommand {0x0f, 8, 16000, 1.0, 0.0},
          "03010f083e80087f60");
    check(HighPassFilterCommand {0x00, true, 600}, "04010001025860");
    check(LabelCommand {{0x00, 0x1234, QByteArray("CH1")}},
          "0501001234034348310b");
    check(RouteLevelCommand {
              .sourceEndpoint = EndpointId::Input1,
              .destinationEndpoint = EndpointId::MainLr,
              .normalizedValue = 0.5},
          "0601000f7f95");
    check(MuteCommand {0x0c, true}, "08010c0116");
    check(ParametricEqCommand {0x00, 2, 1000, 1.0, 0.0},
          "0901000203e8087f7e");
    check(PhaseCommand {0x00, true}, "1001000112");
    check(FxSetupCommand {0x0c, 50, 1, 7, 0x05}, "11010c320107055d");
    check(SnapshotDeleteCommand {3}, "12010316");
    check(CompressorCommand {0x00, 0.5}, "1301007f93");
    check(LimiterCommand {0x0f, -30.0}, "14010f3c60");
    check(PhantomCommand {0x00, true}, "1501000117");
    check(GetChannelStateCommand {3}, "1601031a");

    constexpr auto inputVector =
        "17010000780000007f7f7f7f7f000000000000000000000000000000000b";
    check(commandFromExactFrame(inputVector), inputVector);
    constexpr auto outputVector =
        "1801" "0000" "bf7f78" "bfbfbfbfbfbfbf"
        "7f7f7f7f7f7f7f7f7f"
        "000000000000000000000000000000000000"
        "000000000000000000" "00000000" "7f";
    check(commandFromExactFrame(outputVector), outputVector);
    check(SnapshotSaveCommand {3, QByteArray("Band")}, "1901030442616e6496");
    check(SnapshotLoadCommand {0x7f}, "20017fa0");

    MeterRequestCommand meterRequest;
    meterRequest.count = 8;
    meterRequest.channelCodes = {0x40, 0x41, 0x42, 0x43, 0xc4, 0xc5, 0xc6,
                                 0x8f, 0, 0, 0, 0, 0, 0, 0};
    check(meterRequest,
          "21010840414243c4c5c68f" "00000000000000" "0e");
    constexpr auto meterVector =
        "220100858c92999fa5acb2b8bfc5ccd2d81234f9";
    check(commandFromExactFrame(meterVector), meterVector);
    check(SettingCommand {7, QByteArray::fromHex("01")}, "25010701012f");
    check(FactoryResetCommand {}, "29012a");
    constexpr auto fxStateVector =
        "30010000bf7f00000000bfbfbfbfbfbfbfbfbf26";
    check(commandFromExactFrame(fxStateVector), fxStateVector);
    check(FxPresetCommand {0x0d, 4}, "31010d0443");
    check(ChannelConnectionCommand {0x04, 1, true}, "33010401013a");

    // 0x38 exact fragmented bytes are checked separately below.
    covered.insert(0x38);
    check(FxTempoCommand {120}, "40010078b9");
    check(SelectOutputCommand {0x0f}, "41010f51");
    check(ChannelDelayCommand {0x0f, 0x01020304}, "4a010f0102030464");

    const QSet<quint8> expected {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x08, 0x09, 0x10,
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x20,
        0x21, 0x22, 0x25, 0x29, 0x30, 0x31, 0x33, 0x38, 0x40, 0x41,
        0x4a,
    };
    QCOMPARE(covered, expected);
}

void CommandCodecTest::mixerStateMatchesExactNativeFragmentVector()
{
    const auto encoded = flow8::protocol::encodeCommand(
        Flow8Command {flow8::protocol::MixerStateCommand {}});
    QVERIFY2(encoded.ok(), qPrintable(encoded.message));
    QCOMPARE(encoded.packets.size(), 2);

    // Build the external expected vector from the exact nested payload bytes
    // listed by the APK handoff, then apply the documented 246-byte split.
    const QByteArray inputPayload = QByteArray::fromHex(
        "0000780000007f7f7f7f7f00000000000000000000000000000000");
    const QByteArray outputPayload = QByteArray::fromHex(
        "0000" "bf7f78" "bfbfbfbfbfbfbf"
        "7f7f7f7f7f7f7f7f7f"
        "000000000000000000000000000000000000"
        "000000000000000000" "00000000");
    const QByteArray fxPayload = QByteArray::fromHex(
        "0000bf7f00000000bfbfbfbfbfbfbfbfbf");
    QByteArray expectedPayload;
    for (int index = 0; index < 7; ++index) expectedPayload.append(inputPayload);
    for (int index = 0; index < 3; ++index) expectedPayload.append(outputPayload);
    for (int index = 0; index < 2; ++index) expectedPayload.append(fxPayload);
    expectedPayload.append(QByteArray::fromHex("bf0000000000000000"));
    QCOMPARE(inputPayload.size(), 27);
    QCOMPARE(outputPayload.size(), 52);
    QCOMPARE(fxPayload.size(), 17);
    QCOMPARE(expectedPayload.size(), 388);

    QByteArray expected0 = QByteArray::fromHex("38020000");
    expected0.append(expectedPayload.first(246));
    expected0.append(static_cast<char>(0xfb));
    QByteArray expected1 = QByteArray::fromHex("38020001");
    expected1.append(expectedPayload.sliced(246));
    expected1.append(static_cast<char>(0xfa));
    QVERIFY2(encoded.packets[0] == expected0,
             qPrintable(QStringLiteral("fragment0 actual=%1 expected=%2")
                 .arg(QString::fromLatin1(encoded.packets[0].toHex()),
                      QString::fromLatin1(expected0.toHex()))));
    QVERIFY2(encoded.packets[1] == expected1,
             qPrintable(QStringLiteral("fragment1 actual=%1 expected=%2")
                 .arg(QString::fromLatin1(encoded.packets[1].toHex()),
                      QString::fromLatin1(expected1.toHex()))));
    QCOMPARE(encoded.packets[0].size(), 251);
    QCOMPARE(encoded.packets[1].size(), 147);
}

void CommandCodecTest::reassemblesMixerStateAndRejectsBrokenSequences()
{
    const auto encoded = flow8::protocol::encodeCommand(
        Flow8Command {validMixerState()}, 251, 7);
    QVERIFY(encoded.ok());
    QCOMPARE(encoded.packets.size(), 2);

    flow8::protocol::CommandStreamDecoder decoder;
    auto result = decoder.accept(encoded.packets[1]);
    QVERIFY(result.awaitingFragments);
    result = decoder.accept(encoded.packets[0]);
    QVERIFY2(result.ok(), qPrintable(result.message));
    QVERIFY(std::holds_alternative<flow8::protocol::MixerStateCommand>(
        result.command->value));
    QCOMPARE(decoder.pendingAssemblyCount(), 0);

    // Identical duplicates are idempotent while a sequence is incomplete.
    result = decoder.accept(encoded.packets[0]);
    QVERIFY(result.awaitingFragments);
    result = decoder.accept(encoded.packets[0]);
    QVERIFY(result.awaitingFragments);

    QByteArray conflicting = encoded.packets[0];
    conflicting[4] = static_cast<char>(static_cast<quint8>(conflicting[4]) ^ 0x01U);
    conflicting[conflicting.size() - 1] = static_cast<char>(
        flow8::protocol::checksum(QByteArrayView(conflicting).first(conflicting.size() - 1)));
    result = decoder.accept(conflicting);
    QVERIFY(!result.ok());
    QVERIFY(!result.message.isEmpty());

    decoder.reset();
    // A fragment from another sequence occupies a separate native-style slot
    // and must never complete the first sequence.
    QByteArray wrongSequence = encoded.packets[1];
    wrongSequence[2] = static_cast<char>(8);
    wrongSequence[wrongSequence.size() - 1] = static_cast<char>(
        flow8::protocol::checksum(
            QByteArrayView(wrongSequence).first(wrongSequence.size() - 1)));
    result = decoder.accept(encoded.packets[0]);
    QVERIFY(result.awaitingFragments);
    result = decoder.accept(wrongSequence);
    QVERIFY(result.awaitingFragments);
    QCOMPARE(decoder.pendingAssemblyCount(), 2);

    decoder.reset();
    QByteArray badChecksum = encoded.packets[1];
    badChecksum[badChecksum.size() - 1] ^= 0x01;
    result = decoder.accept(badChecksum);
    QVERIFY(!result.ok());
    QVERIFY(!result.message.isEmpty());
}

void CommandCodecTest::appliesAtomicAndCompositeRxToOneConfirmedState()
{
    flow8::Flow8State state;
    auto mixer = validMixerState();
    mixer.inputs[0].gainDb = 10.0;
    mixer.inputs[0].flags = 0x49; // mute, phantom, solo
    mixer.outputs[0].inputGainsDb[0] = -20.0;
    mixer.effects[0].inputGainsDb[0] = -30.0;
    mixer.effects[0].flags = 0x07; // mute, MAIN and MON1 returns
    mixer.flags[7] = true;
    mixer.tempoBpm = 120;
    mixer.selectedOutput = 0x0f;

    const flow8::protocol::DecodedCommand full {
        .value = Flow8Command {mixer},
        .payload = {},
        .evidence = flow8::model::EvidenceStatus::VerifiedFromApk,
    };
    const auto applied = state.applyProtocolCommand(
        full, flow8::model::EvidenceStatus::VerifiedOffline,
        QStringLiteral("APK native state fixture"));
    QVERIFY(applied.applied);
    QVERIFY(applied.composite);
    QCOMPARE(state.channels().size(), 7);
    QCOMPARE(state.buses().size(), 3);
    QCOMPARE(state.effects().size(), 2);
    QCOMPARE(state.channel(0)->gainDb.value, std::optional(10.0));
    QCOMPARE(state.channel(0)->muted.value, std::optional(true));
    QCOMPARE(state.monitorLink().stereoLinked.value, std::optional(true));
    QCOMPARE(state.globalTempo().bpm.value, std::optional(120.0));
    QVERIFY(state.routing().fxOutputRoute(
        0, flow8::model::FxOutputDestination::Main)->enabled.value.value_or(false));

    // A device-originated atomic value wins over local pending state.
    QVERIFY(state.setRouteLevelPending(
        0, flow8::model::RoutingDestination::Main, 0.95));
    const flow8::protocol::DecodedCommand atomic {
        .value = Flow8Command {flow8::protocol::RouteStateCommand {0, 0x0f, -30.0}},
        .payload = {},
        .evidence = flow8::model::EvidenceStatus::VerifiedFromApk,
    };
    QVERIFY(state.applyProtocolCommand(
        atomic, flow8::model::EvidenceStatus::VerifiedFromDevice,
        QStringLiteral("hardware notification")).applied);
    const auto* route = state.routeLevel(0, flow8::model::RoutingDestination::Main);
    QVERIFY(route != nullptr);
    QVERIFY(!route->pending.has_value());
    QCOMPARE(route->confirmed.evidence,
             flow8::model::EvidenceStatus::VerifiedFromDevice);

    // An invalid composite ID is rejected before any field is applied.
    auto invalidMixer = mixer;
    invalidMixer.inputs[3].id = 99;
    const double before = *state.channel(0)->gainDb.value;
    const auto rejected = state.applyProtocolCommand(
        {.value = Flow8Command {invalidMixer}, .payload = {},
         .evidence = flow8::model::EvidenceStatus::VerifiedFromApk},
        flow8::model::EvidenceStatus::VerifiedOffline,
        QStringLiteral("invalid fixture"));
    QVERIFY(!rejected.applied);
    QCOMPARE(*state.channel(0)->gainDb.value, before);
}

QTEST_GUILESS_MAIN(CommandCodecTest)

#include "command_codec_test.moc"
