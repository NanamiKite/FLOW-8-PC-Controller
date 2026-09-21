#include "protocol/apk_command_catalog.h"
#include "protocol/codec.h"
#include "protocol/flow8_protocol.h"
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
    void recordsApkSemanticsWithoutInventingPayloads();
    void routeLevelSchemaRecordsOnlyApkConfirmedFacts();
    void routeLevelAcceptsConfirmedSemanticRelationships();
    void routeLevelRejectsInvalidSemanticsBeforePayloadEncoding();
};

void ProtocolTest::checksumWrapsModulo256()
{
    QCOMPARE(flow8::protocol::checksum(QByteArray::fromHex("ffff02")), quint8(0x00));
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

void ProtocolTest::recordsApkSemanticsWithoutInventingPayloads()
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
        QVERIFY(!descriptor->payloadLayoutKnown);
        QCOMPARE(descriptor->payloadEvidence, flow8::model::EvidenceStatus::Unknown);
    }
    const auto route = flow8::protocol::apkCommandDescriptor(0x06);
    QVERIFY(route.has_value());
    QCOMPARE(route->id, flow8::protocol::ApkCommandId::RouteLevel);
    QCOMPARE(route->evidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QVERIFY(!route->payloadLayoutKnown);
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

void ProtocolTest::routeLevelSchemaRecordsOnlyApkConfirmedFacts()
{
    const auto schema = flow8::protocol::routeLevelPayloadSchema();
    QCOMPARE(schema.command, flow8::protocol::ApkCommandId::RouteLevel);
    QCOMPARE(static_cast<quint8>(schema.command), quint8(0x06));
    QCOMPARE(schema.commandEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QCOMPARE(schema.semanticEvidence, flow8::model::EvidenceStatus::VerifiedFromApk);
    QCOMPARE(schema.payloadEvidence, flow8::model::EvidenceStatus::Unknown);
    QVERIFY(schema.sourceEndpointSemanticKnown);
    QVERIFY(schema.destinationEndpointSemanticKnown);
    QVERIFY(schema.normalizedInputDomainKnown);
    QVERIFY(schema.normalizedToDbBeforeSerializationObserved);
    QVERIFY(!schema.wireFieldOrderKnown);
    QVERIFY(!schema.wireFieldWidthsKnown);
    QVERIFY(!schema.wireValueEncodingKnown);
    QVERIFY(!schema.wireByteOrderKnown);
    QVERIFY(!schema.commandFragmentationKnown);
}

void ProtocolTest::routeLevelAcceptsConfirmedSemanticRelationships()
{
    using flow8::model::EndpointId;
    using flow8::protocol::RouteLevelCodecError;
    using flow8::protocol::RouteLevelCommand;

    constexpr std::array destinations {
        EndpointId::Monitor1, EndpointId::Monitor2, EndpointId::Fx1,
        EndpointId::Fx2, EndpointId::MainLr,
    };
    constexpr std::array values {0.0, 0.5, 1.0};

    for (const auto destination : destinations) {
        for (const double value : values) {
            const RouteLevelCommand route {
                .sourceEndpoint = EndpointId::Input1,
                .destinationEndpoint = destination,
                .normalizedValue = value,
            };
            QVERIFY(!flow8::protocol::validateRouteLevelCommand(route).has_value());
            const auto encodedRoute = flow8::protocol::encodeRouteLevel(route);
            QVERIFY(!encodedRoute.ok());
            QVERIFY(!encodedRoute.packet.has_value());
            QCOMPARE(encodedRoute.error, RouteLevelCodecError::UnknownPayloadLayout);

            const RouteLevelCommand master {
                .sourceEndpoint = destination,
                .destinationEndpoint = destination,
                .normalizedValue = value,
            };
            QVERIFY(master.isDestinationMaster());
            QVERIFY(!flow8::protocol::validateRouteLevelCommand(master).has_value());
            const auto encodedMaster = flow8::protocol::encodeRouteLevel(master);
            QVERIFY(!encodedMaster.ok());
            QVERIFY(!encodedMaster.packet.has_value());
            QCOMPARE(encodedMaster.error, RouteLevelCodecError::UnknownPayloadLayout);
        }
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
}

QTEST_GUILESS_MAIN(ProtocolTest)

#include "protocol_test.moc"
