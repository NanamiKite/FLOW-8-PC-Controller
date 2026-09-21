#include "protocol/apk_command_catalog.h"
#include "protocol/codec.h"
#include "protocol/flow8_protocol.h"
#include "protocol/gain_codec.h"
#include "protocol/packet.h"
#include "protocol/route_level_codec.h"

#include <QTest>

#include <array>
#include <cmath>
#include <limits>

class ProtocolTest final : public QObject {
    Q_OBJECT

private slots:
    void checksumWrapsModulo256();
    void validatesReferenceChecksums();
    void recognizesOnlyDocumentedPacketTypes();
    void decodesCapturedFaderPacket();
    void preservesUnknownStatePayload();
    void preservesMultiFragmentHeadersWithoutGuessingMeaning();
    void rejectsBadChecksum();
    void legacyReferenceUnitIntervalRoundTrip_data();
    void legacyReferenceUnitIntervalRoundTrip();
    void packetSemanticRoundTrip();
    void recordsApkCommandCatalogEvidence();
    void gainSchemaRecordsApkSerializer();
    void gainFix8Format5MatchesNativeRules();
    void gainEncodesExactApkVectors();
    void gainSeparatesWireEndpointFromCapability();
    void gainRejectsInvalidInputs();
    void routeLevelSchemaRecordsApkSerializer();
    void routeLevelConversionMatchesApkVectors();
    void routeLevelEncodesExactApkVectors();
    void routeLevelEndpointAndMasterVectors();
    void routeLevelRejectsInvalidSemanticsBeforePayloadEncoding();
};

void ProtocolTest::checksumWrapsModulo256()
{
    QCOMPARE(flow8::protocol::checksum(QByteArray::fromHex("ffff02")), quint8(0x00));
    QCOMPARE(flow8::protocol::checksum(QByteArray::fromHex("0601000f7f")), quint8(0x95));
    QCOMPARE(flow8::protocol::checksum(QByteArray::fromHex("06010a0a7f")), quint8(0x9a));
    QCOMPARE(flow8::protocol::checksum(QByteArray::fromHex("02010050")), quint8(0x53));
    QCOMPARE(flow8::protocol::checksum(QByteArray::fromHex("02010478")), quint8(0x7f));
    QVERIFY(!flow8::protocol::hasValidChecksum(QByteArray::fromHex("01")));
}

void ProtocolTest::validatesReferenceChecksums()
{
    // REFERENCE CAPTURE: exact packets listed in flow8-midi-implementation.md.
    const QList<QByteArray> packets {
        QByteArray::fromHex("370138"),
        QByteArray::fromHex("2601b0d7"),
        QByteArray::fromHex("260180a7"),
        QByteArray::fromHex("4b014c"),
        QByteArray::fromHex("3901fd062b0639f17fe7b7278b8f355a495c2a"),
    };
    for (const auto& packet : packets) {
        QVERIFY2(flow8::protocol::hasValidChecksum(packet), packet.toHex().constData());
    }
    QCOMPARE(flow8::protocol::referenceSessionStartPacket(), QByteArray::fromHex("370138"));
    QCOMPARE(flow8::protocol::referenceConfigRequestPacket(), QByteArray::fromHex("070108"));
    QCOMPARE(flow8::protocol::referenceDumpTriggerPacket(), QByteArray::fromHex("4b014c"));
    QCOMPARE(flow8::protocol::referenceAuthenticationPacket(), packets.constLast());
}

void ProtocolTest::recognizesOnlyDocumentedPacketTypes()
{
    const QList<quint8> apkDocumented {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x27, 0x29, 0x30, 0x31,
        0x32, 0x33, 0x37, 0x38, 0x40, 0x41, 0x4A,
    };
    for (const quint8 type : apkDocumented) {
        QVERIFY(flow8::protocol::knownPacketType(type).has_value());
        QCOMPARE(flow8::protocol::packetEvidence(type),
                 flow8::model::EvidenceStatus::VerifiedFromApk);
    }
    const QList<quint8> referenceOnly {0x26, 0x35, 0x36, 0x39, 0x4B};
    for (const quint8 type : referenceOnly) {
        QVERIFY(flow8::protocol::knownPacketType(type).has_value());
        QCOMPARE(flow8::protocol::packetEvidence(type),
                 flow8::model::EvidenceStatus::Inferred);
    }
    QVERIFY(!flow8::protocol::knownPacketType(0xFF).has_value());
    QCOMPARE(flow8::protocol::packetEvidence(0xFF), flow8::model::EvidenceStatus::Unknown);
}

void ProtocolTest::decodesCapturedFaderPacket()
{
    // REFERENCE CAPTURE: Ch1 fader at maximum.
    const QByteArray raw = QByteArray::fromHex("0601010fff16");
    const auto result = flow8::protocol::parsePacket(raw);
    QVERIFY(result.ok());
    const auto change = flow8::protocol::decodeLegacyReferenceParameterChange(*result.packet);
    QVERIFY(change.has_value());
    QCOMPARE(change->channel, quint8(1));
    QCOMPARE(change->parameter, flow8::protocol::legacyReferenceFaderLevelParameter);
    QCOMPARE(change->value, quint8(255));
    QCOMPARE(change->evidence, flow8::model::EvidenceStatus::Inferred);
}

void ProtocolTest::preservesUnknownStatePayload()
{
    // SYNTHETIC framing test: payload meaning is deliberately not interpreted.
    const QByteArray payload = QByteArray::fromHex("0201020304");
    const QByteArray raw = flow8::protocol::frameSingleFragment(0x38, payload);
    const auto result = flow8::protocol::parsePacket(raw);
    QVERIFY(result.ok());
    QCOMPARE(result.packet->type, quint8(0x38));
    QCOMPARE(result.packet->fragmentCount, quint8(0x01));
    QCOMPARE(result.packet->payload, payload);
    QCOMPARE(result.packet->payloadEvidence, flow8::model::EvidenceStatus::Unknown);
}

void ProtocolTest::preservesMultiFragmentHeadersWithoutGuessingMeaning()
{
    // SYNTHETIC structure-only vector. Header A/B meaning remains unknown.
    QByteArray raw = QByteArray::fromHex("3802aabb0102");
    raw.append(static_cast<char>(flow8::protocol::checksum(raw)));
    const auto result = flow8::protocol::parsePacket(raw);
    QVERIFY(result.ok());
    QCOMPARE(result.packet->fragmentCount, quint8(2));
    QCOMPARE(result.packet->fragmentHeaderA, std::optional<quint8>(0xaa));
    QCOMPARE(result.packet->fragmentHeaderB, std::optional<quint8>(0xbb));
    QCOMPARE(result.packet->payload, QByteArray::fromHex("0102"));
    QCOMPARE(result.packet->payloadEvidence, flow8::model::EvidenceStatus::Unknown);
}

void ProtocolTest::rejectsBadChecksum()
{
    QByteArray raw = QByteArray::fromHex("370100");
    const auto result = flow8::protocol::parsePacket(raw);
    QVERIFY(!result.ok());
    QCOMPARE(result.error, std::optional(flow8::protocol::PacketError::ChecksumMismatch));
}

void ProtocolTest::legacyReferenceUnitIntervalRoundTrip_data()
{
    QTest::addColumn<double>("value");
    QTest::newRow("minimum") << 0.0;
    QTest::newRow("one-third") << (1.0 / 3.0);
    QTest::newRow("half") << 0.5;
    QTest::newRow("maximum") << 1.0;
}

void ProtocolTest::legacyReferenceUnitIntervalRoundTrip()
{
    QFETCH(double, value);
    const auto encoded = flow8::protocol::encodeLegacyUnitInterval8(value);
    QVERIFY(encoded.has_value());
    const double decoded = flow8::protocol::decodeLegacyUnitInterval8(*encoded);
    QVERIFY(std::abs(decoded - value) <= (0.5 / 255.0) + 1.0e-12);
    QVERIFY(!flow8::protocol::encodeLegacyUnitInterval8(-0.01).has_value());
    QVERIFY(!flow8::protocol::encodeLegacyUnitInterval8(1.01).has_value());
}

void ProtocolTest::packetSemanticRoundTrip()
{
    const QByteArray captured = QByteArray::fromHex("0601010fff16");
    const auto decoded = flow8::protocol::parsePacket(captured);
    QVERIFY(decoded.ok());
    const QByteArray encoded = flow8::protocol::frameSingleFragment(
        decoded.packet->type, decoded.packet->payload);
    QCOMPARE(encoded, captured);
    QCOMPARE(flow8::protocol::parsePacket(encoded).packet->payload, decoded.packet->payload);
}

void ProtocolTest::recordsApkCommandCatalogEvidence()
{
    const QList<quint8> apkCommands {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x27, 0x29, 0x30, 0x31,
        0x32, 0x33, 0x37, 0x38, 0x40, 0x41, 0x4a,
    };
    for (const quint8 command : apkCommands) {
        const auto descriptor = flow8::protocol::apkCommandDescriptor(command);
        QVERIFY(descriptor.has_value());
        QCOMPARE(descriptor->evidence,
                 flow8::model::EvidenceStatus::VerifiedFromApk);
        QVERIFY(descriptor->commandByteConfirmed);
        const bool payloadKnown = command == 0x02 || command == 0x06;
        QCOMPARE(descriptor->payloadLayoutKnown, payloadKnown);
        QCOMPARE(descriptor->payloadEvidence,
                 payloadKnown ? flow8::model::EvidenceStatus::VerifiedFromApk
                              : flow8::model::EvidenceStatus::Unknown);
    }
    const auto gain = flow8::protocol::apkCommandDescriptor(0x02);
    QVERIFY(gain.has_value());
    QCOMPARE(gain->id, flow8::protocol::ApkCommandId::Gain);
    QVERIFY(gain->payloadLayoutKnown);
    QCOMPARE(gain->payloadEvidence,
             flow8::model::EvidenceStatus::VerifiedFromApk);
    const auto route = flow8::protocol::apkCommandDescriptor(0x06);
    QVERIFY(route.has_value());
    QCOMPARE(route->id, flow8::protocol::ApkCommandId::RouteLevel);
    QCOMPARE(route->evidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QVERIFY(route->payloadLayoutKnown);
    QCOMPARE(route->payloadEvidence,
             flow8::model::EvidenceStatus::VerifiedFromApk);
    QVERIFY(!flow8::protocol::apkCommandDescriptor(0xff).has_value());

    QCOMPARE(flow8::protocol::apkRouteSourceId(6),
             std::optional(flow8::protocol::ApkRouteSourceId::BluetoothUsb));
    QVERIFY(!flow8::protocol::apkRouteSourceId(7).has_value());
    QCOMPARE(flow8::protocol::apkRouteDestinationId(
                 flow8::model::RoutingDestination::Main),
             std::optional(flow8::protocol::ApkRouteDestinationId::MainLr));
    QCOMPARE(flow8::protocol::apkRouteDestinationId(
                 flow8::model::RoutingDestination::Monitor1),
             std::optional(flow8::protocol::ApkRouteDestinationId::Monitor1));
    QCOMPARE(flow8::protocol::apkEndpointId(13),
             std::optional(flow8::protocol::ApkEndpointId::Fx2));
    QVERIFY(!flow8::protocol::apkEndpointId(14).has_value());
}

void ProtocolTest::gainSchemaRecordsApkSerializer()
{
    const auto schema = flow8::protocol::gainPayloadSchema();
    QCOMPARE(schema.command, flow8::protocol::ApkCommandId::Gain);
    QCOMPARE(static_cast<quint8>(schema.command), quint8(0x02));
    QCOMPARE(schema.commandEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QCOMPARE(schema.semanticEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QCOMPARE(schema.payloadEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QVERIFY(schema.endpointFieldKnown);
    QVERIFY(schema.gainIsDb);
    QVERIFY(schema.fix8Format5Known);
    QVERIFY(schema.nativeClampKnown);
    QVERIFY(schema.nativeRoundingKnown);
    QVERIFY(schema.singleFragmentFramingKnown);
    QVERIFY(!schema.codecEnforcesInputCapability);
}

void ProtocolTest::gainFix8Format5MatchesNativeRules()
{
    using flow8::protocol::encodeGainFix8Format5;

    QCOMPARE(encodeGainFix8Format5(-80.0), std::optional<quint8>(0x00));
    QCOMPARE(encodeGainFix8Format5(-60.0), std::optional<quint8>(0x00));
    QCOMPARE(encodeGainFix8Format5(-20.0), std::optional<quint8>(0x50));
    QCOMPARE(encodeGainFix8Format5(0.0), std::optional<quint8>(0x78));
    QCOMPARE(encodeGainFix8Format5(20.0), std::optional<quint8>(0xa0));
    QCOMPARE(encodeGainFix8Format5(40.0), std::optional<quint8>(0xc8));
    QCOMPARE(encodeGainFix8Format5(60.0), std::optional<quint8>(0xf0));
    QCOMPARE(encodeGainFix8Format5(80.0), std::optional<quint8>(0xf0));

    // With the normal round-to-nearest mode used by the APK probe, lrint
    // resolves exact half-way values to the nearest even integer.
    QCOMPARE(encodeGainFix8Format5(-19.75), std::optional<quint8>(0x50));
    QCOMPARE(encodeGainFix8Format5(-19.25), std::optional<quint8>(0x52));
}

void ProtocolTest::gainEncodesExactApkVectors()
{
    using flow8::model::EndpointId;

    struct GainVector {
        EndpointId endpoint;
        double gainDb;
        const char* expectedHex;
    };
    constexpr std::array vectors {
        GainVector {EndpointId::Input1, -20.0, "0201005053"},
        GainVector {EndpointId::Input1, 0.0, "020100787b"},
        GainVector {EndpointId::Input1, 20.0, "020100a0a3"},
        GainVector {EndpointId::Input1, 40.0, "020100c8cb"},
        GainVector {EndpointId::Input1, 60.0, "020100f0f3"},
        GainVector {EndpointId::Input56, -20.0, "0201045057"},
        GainVector {EndpointId::Input56, 0.0, "020104787f"},
        GainVector {EndpointId::Input56, 20.0, "020104a0a7"},
        GainVector {EndpointId::Input56, 40.0, "020104c8cf"},
        GainVector {EndpointId::Input56, 60.0, "020104f0f7"},
    };

    for (const auto& vector : vectors) {
        const auto result = flow8::protocol::encodeGain({
            .inputEndpoint = vector.endpoint,
            .gainDb = vector.gainDb,
        });
        QVERIFY(result.ok());
        QCOMPARE(result.error, flow8::protocol::GainCodecError::None);
        QCOMPARE(*result.packet, QByteArray::fromHex(vector.expectedHex));
        QVERIFY(flow8::protocol::hasValidChecksum(*result.packet));
        const auto parsed = flow8::protocol::parsePacket(*result.packet);
        QVERIFY(parsed.ok());
        QCOMPARE(parsed.packet->payloadEvidence,
                 flow8::model::EvidenceStatus::VerifiedFromApk);
    }

    const auto clampedMinimum = flow8::protocol::encodeGain({
        .inputEndpoint = EndpointId::Input1,
        .gainDb = -60.0,
    });
    QVERIFY(clampedMinimum.ok());
    QCOMPARE(*clampedMinimum.packet, QByteArray::fromHex("0201000003"));
}

void ProtocolTest::gainSeparatesWireEndpointFromCapability()
{
    using flow8::model::EndpointId;

    QVERIFY(flow8::model::isGainCapableInputEndpoint(EndpointId::Input1));
    QVERIFY(flow8::model::isGainCapableInputEndpoint(EndpointId::Input78));
    QVERIFY(!flow8::model::isGainCapableInputEndpoint(EndpointId::BluetoothUsb));

    // The native serializer can mechanically encode endpoint 6. This is a
    // wire fact only and must not be treated as product capability evidence.
    const auto encoded = flow8::protocol::encodeGain({
        .inputEndpoint = EndpointId::BluetoothUsb,
        .gainDb = 0.0,
    });
    QVERIFY(encoded.ok());
    QCOMPARE(*encoded.packet, QByteArray::fromHex("0201067881"));
}

void ProtocolTest::gainRejectsInvalidInputs()
{
    using flow8::model::EndpointId;
    using flow8::protocol::GainCodecError;

    const auto invalidEndpoint = flow8::protocol::encodeGain({
        .inputEndpoint = EndpointId::MainLr,
        .gainDb = 0.0,
    });
    QCOMPARE(invalidEndpoint.error, GainCodecError::InvalidInputEndpoint);
    QVERIFY(!invalidEndpoint.packet.has_value());

    constexpr std::array invalidValues {
        std::numeric_limits<double>::quiet_NaN(),
        std::numeric_limits<double>::infinity(),
        -std::numeric_limits<double>::infinity(),
    };
    for (const double value : invalidValues) {
        const auto result = flow8::protocol::encodeGain({
            .inputEndpoint = EndpointId::Input1,
            .gainDb = value,
        });
        QCOMPARE(result.error, GainCodecError::InvalidGainValue);
        QVERIFY(!result.packet.has_value());
    }
}

void ProtocolTest::routeLevelSchemaRecordsApkSerializer()
{
    const auto schema = flow8::protocol::routeLevelPayloadSchema();
    QCOMPARE(schema.command, flow8::protocol::ApkCommandId::RouteLevel);
    QCOMPARE(static_cast<quint8>(schema.command), quint8(0x06));
    QCOMPARE(schema.commandEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QCOMPARE(schema.semanticEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QCOMPARE(schema.payloadEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QVERIFY(schema.sourceEndpointSemanticKnown);
    QVERIFY(schema.destinationEndpointSemanticKnown);
    QVERIFY(schema.normalizedInputDomainKnown);
    QVERIFY(schema.normalizedToDbBeforeSerializationObserved);
    QVERIFY(schema.wireFieldOrderKnown);
    QVERIFY(schema.wireFieldWidthsKnown);
    QVERIFY(schema.wireValueEncodingKnown);
    QVERIFY(schema.wireByteOrderKnown);
    QVERIFY(schema.commandFragmentationKnown);
}

void ProtocolTest::routeLevelConversionMatchesApkVectors()
{
    struct ConversionVector {
        double normalized;
        float decibels;
        quint8 level;
    };
    constexpr std::array vectors {
        ConversionVector {0.0, -144.0F, 0x00},
        ConversionVector {0.25, -30.0F, 0x3f},
        ConversionVector {0.5, -10.0F, 0x7f},
        ConversionVector {0.75, 0.0F, 0xbf},
        ConversionVector {1.0, 10.0F, 0xff},
    };

    for (const auto& vector : vectors) {
        const auto decibels = flow8::protocol::routeLevelNormalizedToDb(
            vector.normalized);
        QVERIFY(decibels.has_value());
        QCOMPARE(*decibels, vector.decibels);
        QCOMPARE(flow8::protocol::encodeRouteLevelFix8(*decibels),
                 std::optional<quint8>(vector.level));
    }
    QCOMPARE(flow8::protocol::routeLevelNormalizedToDb(1.0 / 1024.0),
             std::optional<float>(-89.53125F));
    QCOMPARE(flow8::protocol::routeLevelNormalizedToDb(0.0625),
             std::optional<float>(-60.0F));

    const auto& table = flow8::protocol::routeLevelFix8DbTable();
    QCOMPARE(table.size(), std::size_t(256));
    QCOMPARE(table.front(), -144.0F);
    QVERIFY(std::abs(table[1] - (-88.117645F)) < 0.00001F);
    QVERIFY(std::abs(table[16] - (-59.960785F)) < 0.00001F);
    QVERIFY(std::abs(table[64] - (-29.921568F)) < 0.00001F);
    QVERIFY(std::abs(table[128] - (-9.921568F)) < 0.00001F);
    QCOMPARE(table[191], 0.0F);
    QCOMPARE(table.back(), 10.0F);
    for (std::size_t index = 1; index < table.size(); ++index) {
        QVERIFY(table[index] >= table[index - 1]);
    }
}

void ProtocolTest::routeLevelEncodesExactApkVectors()
{
    using flow8::model::EndpointId;
    using flow8::protocol::RouteLevelCommand;

    struct PacketVector {
        double normalized;
        const char* expectedHex;
    };
    constexpr std::array vectors {
        PacketVector {0.0, "0601000f0016"},
        PacketVector {0.25, "0601000f3f55"},
        PacketVector {0.5, "0601000f7f95"},
        PacketVector {0.75, "0601000fbfd5"},
        PacketVector {1.0, "0601000fff15"},
    };

    for (const auto& vector : vectors) {
        const auto result = flow8::protocol::encodeRouteLevel({
            .sourceEndpoint = EndpointId::Input1,
            .destinationEndpoint = EndpointId::MainLr,
            .normalizedValue = vector.normalized,
        });
        QVERIFY(result.ok());
        QCOMPARE(result.error, flow8::protocol::RouteLevelCodecError::None);
        QCOMPARE(*result.packet, QByteArray::fromHex(vector.expectedHex));
        QVERIFY(flow8::protocol::hasValidChecksum(*result.packet));
        const auto parsed = flow8::protocol::parsePacket(*result.packet);
        QVERIFY(parsed.ok());
        QCOMPARE(parsed.packet->payloadEvidence,
                 flow8::model::EvidenceStatus::VerifiedFromApk);
    }
}

void ProtocolTest::routeLevelEndpointAndMasterVectors()
{
    using flow8::model::EndpointId;
    using flow8::protocol::RouteLevelCommand;

    struct EndpointVector {
        EndpointId source;
        EndpointId destination;
        const char* expectedHex;
    };
    constexpr std::array vectors {
        EndpointVector {EndpointId::Input1, EndpointId::Monitor1, "0601000a7f90"},
        EndpointVector {EndpointId::Input1, EndpointId::Monitor2, "0601000b7f91"},
        EndpointVector {EndpointId::Input1, EndpointId::Fx1, "0601000c7f92"},
        EndpointVector {EndpointId::Input1, EndpointId::Fx2, "0601000d7f93"},
        EndpointVector {EndpointId::Input1, EndpointId::MainLr, "0601000f7f95"},
        EndpointVector {EndpointId::Monitor1, EndpointId::Monitor1, "06010a0a7f9a"},
        EndpointVector {EndpointId::Monitor2, EndpointId::Monitor2, "06010b0b7f9c"},
        EndpointVector {EndpointId::Fx1, EndpointId::Fx1, "06010c0c7f9e"},
        EndpointVector {EndpointId::Fx2, EndpointId::Fx2, "06010d0d7fa0"},
        EndpointVector {EndpointId::MainLr, EndpointId::MainLr, "06010f0f7fa4"},
    };

    for (const auto& vector : vectors) {
        const RouteLevelCommand command {
            .sourceEndpoint = vector.source,
            .destinationEndpoint = vector.destination,
            .normalizedValue = 0.5,
        };
        QVERIFY(!flow8::protocol::validateRouteLevelCommand(command).has_value());
        QCOMPARE(command.isDestinationMaster(), vector.source == vector.destination);
        const auto encoded = flow8::protocol::encodeRouteLevel(command);
        QVERIFY(encoded.ok());
        QCOMPARE(*encoded.packet, QByteArray::fromHex(vector.expectedHex));
    }

    QCOMPARE(static_cast<quint8>(EndpointId::Input1), quint8(0));
    QCOMPARE(static_cast<quint8>(EndpointId::Monitor1), quint8(10));
    QCOMPARE(static_cast<quint8>(EndpointId::Monitor2), quint8(11));
    QCOMPARE(static_cast<quint8>(EndpointId::Fx1), quint8(12));
    QCOMPARE(static_cast<quint8>(EndpointId::Fx2), quint8(13));
    QCOMPARE(static_cast<quint8>(EndpointId::MainLr), quint8(15));
}

void ProtocolTest::routeLevelRejectsInvalidSemanticsBeforePayloadEncoding()
{
    using flow8::model::EndpointId;
    using flow8::protocol::RouteLevelCodecError;
    using flow8::protocol::RouteLevelCommand;

    const auto invalidDestination = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Input1,
        .destinationEndpoint = EndpointId::Input2,
        .normalizedValue = 0.5,
    });
    QCOMPARE(invalidDestination.error, RouteLevelCodecError::InvalidDestinationEndpoint);
    QVERIFY(!invalidDestination.packet.has_value());

    const auto invalidRelationship = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Monitor1,
        .destinationEndpoint = EndpointId::Monitor2,
        .normalizedValue = 0.5,
    });
    QCOMPARE(invalidRelationship.error, RouteLevelCodecError::InvalidRouteRelationship);
    QVERIFY(!invalidRelationship.packet.has_value());

    const auto invalidSource = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = static_cast<EndpointId>(7),
        .destinationEndpoint = EndpointId::MainLr,
        .normalizedValue = 0.5,
    });
    QCOMPARE(invalidSource.error, RouteLevelCodecError::InvalidRouteRelationship);
    QVERIFY(!invalidSource.packet.has_value());

    const auto invalidValue = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Input1,
        .destinationEndpoint = EndpointId::MainLr,
        .normalizedValue = 1.01,
    });
    QCOMPARE(invalidValue.error, RouteLevelCodecError::InvalidNormalizedValue);
    QVERIFY(!invalidValue.packet.has_value());

    const auto nonFiniteValue = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Input1,
        .destinationEndpoint = EndpointId::MainLr,
        .normalizedValue = std::numeric_limits<double>::quiet_NaN(),
    });
    QCOMPARE(nonFiniteValue.error, RouteLevelCodecError::InvalidNormalizedValue);
    QVERIFY(!nonFiniteValue.packet.has_value());

    const auto positiveInfinity = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Input1,
        .destinationEndpoint = EndpointId::MainLr,
        .normalizedValue = std::numeric_limits<double>::infinity(),
    });
    QCOMPARE(positiveInfinity.error, RouteLevelCodecError::InvalidNormalizedValue);
    QVERIFY(!positiveInfinity.packet.has_value());

    const auto negativeInfinity = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Input1,
        .destinationEndpoint = EndpointId::MainLr,
        .normalizedValue = -std::numeric_limits<double>::infinity(),
    });
    QCOMPARE(negativeInfinity.error, RouteLevelCodecError::InvalidNormalizedValue);
    QVERIFY(!negativeInfinity.packet.has_value());

    const auto belowMinimum = flow8::protocol::encodeRouteLevel({
        .sourceEndpoint = EndpointId::Input1,
        .destinationEndpoint = EndpointId::MainLr,
        .normalizedValue = -0.01,
    });
    QCOMPARE(belowMinimum.error, RouteLevelCodecError::InvalidNormalizedValue);
    QVERIFY(!belowMinimum.packet.has_value());
}

QTEST_GUILESS_MAIN(ProtocolTest)

#include "protocol_test.moc"
