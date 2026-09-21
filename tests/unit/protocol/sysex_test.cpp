#include "protocol/sysex.h"

#include <QTest>

#include <bit>

namespace {

void encodeSyntheticFloat(QByteArray& bytes, const flow8::protocol::PackedFloatLayout& layout,
                          const float value)
{
    const quint32 bits = std::bit_cast<quint32>(value);
    quint8 msb = static_cast<quint8>(bytes[layout.msbOffset]);
    for (std::size_t index = 0; index < 4; ++index) {
        const quint8 decoded = static_cast<quint8>((bits >> (index * 8U)) & 0xFFU);
        bytes[layout.dataOffsets[index]] = static_cast<char>(decoded & 0x7FU);
        if ((decoded & 0x80U) != 0U) {
            msb = static_cast<quint8>(msb | (1U << layout.bitIndices[index]));
        }
    }
    bytes[layout.msbOffset] = static_cast<char>(msb);
}

std::optional<double> parameterValue(const flow8::protocol::ParsedSysExState& state,
                                     const QString& path)
{
    for (const auto& parameter : state.parameters) {
        if (parameter.path == path) {
            return parameter.value;
        }
    }
    return std::nullopt;
}

std::optional<bool> flagValue(const flow8::protocol::ParsedSysExState& state,
                              const QString& path)
{
    for (const auto& flag : state.flags) {
        if (flag.path == path) {
            return flag.value;
        }
    }
    return std::nullopt;
}

} // namespace

class SysExTest final : public QObject {
    Q_OBJECT

private slots:
    void validatesFramingAndIdentity();
    void decodesSyntheticPackedFloat();
    void rejectsInvalidPackedFloatLayout();
    void parsesSyntheticReferenceLayout();
    void extractsDocumentedReferenceOffsets();
};

void SysExTest::validatesFramingAndIdentity()
{
    const QByteArray header = QByteArray::fromHex("f00020322100f7");
    const auto valid = flow8::protocol::validateSysEx(header);
    QVERIFY(valid.valid);
    QVERIFY(valid.isFlow8);

    const auto invalid = flow8::protocol::validateSysEx(QByteArray::fromHex("f00020322200f7"));
    QVERIFY(invalid.valid);
    QVERIFY(!invalid.isFlow8);
}

void SysExTest::decodesSyntheticPackedFloat()
{
    // SYNTHETIC: exercises the documented 7-bit packing rule, not a device capture.
    QByteArray bytes(16, '\0');
    const flow8::protocol::PackedFloatLayout layout {2, {3, 4, 5, 6}, {0, 1, 2, 3}};
    encodeSyntheticFloat(bytes, layout, -12.5F);
    const auto decoded = flow8::protocol::decodePackedFloat(bytes, layout);
    QVERIFY(decoded.has_value());
    QCOMPARE(*decoded, -12.5F);
}

void SysExTest::rejectsInvalidPackedFloatLayout()
{
    const QByteArray bytes(16, '\0');
    const flow8::protocol::PackedFloatLayout negativeOffset {2, {-1, 4, 5, 6}, {0, 1, 2, 3}};
    QVERIFY(!flow8::protocol::decodePackedFloat(bytes, negativeOffset).has_value());

    const flow8::protocol::PackedFloatLayout invalidBit {2, {3, 4, 5, 6}, {0, 1, 2, 8}};
    QVERIFY(!flow8::protocol::decodePackedFloat(bytes, invalidBit).has_value());
}

void SysExTest::parsesSyntheticReferenceLayout()
{
    // SYNTHETIC: only the offsets originate from reference calibration evidence.
    QByteArray dump(0x700, '\0');
    dump[0] = static_cast<char>(0xF0);
    dump[1] = 0x00;
    dump[2] = 0x20;
    dump[3] = 0x32;
    dump[4] = 0x21;
    dump[dump.size() - 1] = static_cast<char>(0xF7);
    const flow8::protocol::PackedFloatLayout channelOne {0x0067,
                                                         {0x0068, 0x0069, 0x006A, 0x006B},
                                                         {0, 1, 2, 3}};
    encodeSyntheticFloat(dump, channelOne, -20.0F);

    const auto parsed = flow8::protocol::parseReferenceStateDump(dump);
    QVERIFY(parsed.validation.valid);
    QVERIFY(parsed.validation.isFlow8);
    QVERIFY(!parsed.parameters.isEmpty());
    QCOMPARE(parsed.parameters.first().path, QStringLiteral("channel.0.level_db"));
    QCOMPARE(parsed.parameters.first().value, -20.0);
    QCOMPARE(parsed.parameters.first().evidence, flow8::model::EvidenceStatus::Inferred);
}

void SysExTest::extractsDocumentedReferenceOffsets()
{
    // SYNTHETIC: values exercise reference-derived offsets; this is not a device dump.
    QByteArray dump(0x0C00, '\0');
    dump[0] = static_cast<char>(0xF0);
    dump[1] = 0x00;
    dump[2] = 0x20;
    dump[3] = 0x32;
    dump[4] = 0x21;
    dump[dump.size() - 1] = static_cast<char>(0xF7);

    encodeSyntheticFloat(dump, {0x0739, {0x0736, 0x073A, 0x073B, 0x073C}, {4, 0, 1, 2}},
                         18.0F);
    encodeSyntheticFloat(dump, {0x04C7, {0x04C8, 0x04C9, 0x04CA, 0x04CB}, {0, 1, 2, 3}},
                         -6.0F);
    encodeSyntheticFloat(dump, {0x054C, {0x0547, 0x0548, 0x0549, 0x054E}, {6, 5, 4, 1}},
                         0.25F);
    dump[0x04CC] = 0x01;
    dump[0x04DF] = 0x01;
    dump[0x0737] = 0x01;
    dump[0x0BC5] = 75;
    dump[0x0BC6] = 25;
    dump[0x0BC9] = 3;

    const auto parsed = flow8::protocol::parseReferenceStateDump(dump);
    QCOMPARE(parameterValue(parsed, QStringLiteral("channel.0.gain_db")), std::optional(18.0));
    QCOMPARE(parameterValue(parsed, QStringLiteral("bus.0.level_db")), std::optional(-6.0));
    QCOMPARE(parameterValue(parsed, QStringLiteral("bus.0.balance")), std::optional(0.25));
    QCOMPARE(parameterValue(parsed, QStringLiteral("fx.0.parameter1_percent")),
             std::optional(75.0));
    QCOMPARE(parameterValue(parsed, QStringLiteral("fx.0.preset_reference_index")),
             std::optional(2.0));
    QCOMPARE(flagValue(parsed, QStringLiteral("channel.0.muted")), std::optional(true));
    QCOMPARE(flagValue(parsed, QStringLiteral("channel.0.soloed")), std::optional(true));
    QCOMPARE(flagValue(parsed, QStringLiteral("channel.0.phantom_48v")), std::optional(true));
}

QTEST_GUILESS_MAIN(SysExTest)

#include "sysex_test.moc"
