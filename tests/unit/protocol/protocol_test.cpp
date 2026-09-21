#include "protocol/codec.h"
#include "protocol/flow8_protocol.h"
#include "protocol/packet.h"

#include <QTest>

class ProtocolTest final : public QObject {
    Q_OBJECT

private slots:
    void validatesReferenceChecksums();
    void recognizesOnlyDocumentedPacketTypes();
    void decodesCapturedFaderPacket();
    void preservesUnknownStatePayload();
    void rejectsBadChecksum();
};

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
    QCOMPARE(flow8::protocol::sessionStartPacket(), QByteArray::fromHex("370138"));
    QCOMPARE(flow8::protocol::configRequestPacket(), QByteArray::fromHex("070108"));
    QCOMPARE(flow8::protocol::dumpTriggerPacket(), QByteArray::fromHex("4b014c"));
    QCOMPARE(flow8::protocol::referenceAuthenticationPacket(), packets.constLast());
}

void ProtocolTest::recognizesOnlyDocumentedPacketTypes()
{
    const QList<quint8> documented {0x06, 0x07, 0x21, 0x22, 0x25, 0x26, 0x27,
                                    0x35, 0x36, 0x37, 0x38, 0x39, 0x4B};
    for (const quint8 type : documented) {
        QVERIFY(flow8::protocol::knownPacketType(type).has_value());
        QCOMPARE(flow8::protocol::packetEvidence(type), flow8::model::EvidenceStatus::Inferred);
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
    const auto change = flow8::protocol::decodeParameterChange(*result.packet);
    QVERIFY(change.has_value());
    QCOMPARE(change->channel, quint8(1));
    QCOMPARE(change->parameter, flow8::protocol::faderLevelParameter);
    QCOMPARE(change->value, quint8(255));
    QCOMPARE(change->evidence, flow8::model::EvidenceStatus::Inferred);
}

void ProtocolTest::preservesUnknownStatePayload()
{
    // SYNTHETIC framing test: payload meaning is deliberately not interpreted.
    const QByteArray payload = QByteArray::fromHex("0201020304");
    const QByteArray raw = flow8::protocol::framePacket(0x38, 0x04, payload);
    const auto result = flow8::protocol::parsePacket(raw);
    QVERIFY(result.ok());
    QCOMPARE(result.packet->type, quint8(0x38));
    QCOMPARE(result.packet->discriminator, quint8(0x04));
    QCOMPARE(result.packet->payload, payload);
}

void ProtocolTest::rejectsBadChecksum()
{
    QByteArray raw = QByteArray::fromHex("370100");
    const auto result = flow8::protocol::parsePacket(raw);
    QVERIFY(!result.ok());
    QCOMPARE(result.error, std::optional(flow8::protocol::PacketError::ChecksumMismatch));
}

QTEST_GUILESS_MAIN(ProtocolTest)

#include "protocol_test.moc"
