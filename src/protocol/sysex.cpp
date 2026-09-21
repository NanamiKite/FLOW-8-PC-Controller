#include "protocol/sysex.h"

#include <QtEndian>

#include <algorithm>
#include <bit>
#include <cmath>

namespace flow8::protocol {
namespace {

constexpr quint8 sysExStart = 0xF0;
constexpr quint8 sysExEnd = 0xF7;
constexpr std::array<quint8, 3> behringerId {0x00, 0x20, 0x32};
constexpr quint8 flow8Model = 0x21;
constexpr qsizetype namesStart = 0x0554;
constexpr qsizetype namesStride = 0x1E;
constexpr qsizetype nameScanLength = 14;

// INFERRED: derived from the bundled reference calibration on one setup.
constexpr std::array<PackedFloatLayout, 7> channelLevelLayouts {{
    {0x0067, {0x0068, 0x0069, 0x006A, 0x006B}, {0, 1, 2, 3}},
    {0x00B4, {0x00B2, 0x00B3, 0x00B5, 0x00B6}, {3, 2, 0, 1}},
    {0x00FA, {0x00FD, 0x00FE, 0x00FF, 0x0100}, {2, 3, 4, 5}},
    {0x0147, {0x0148, 0x0149, 0x014A, 0x014B}, {0, 1, 2, 3}},
    {0x0194, {0x0192, 0x0193, 0x0195, 0x0196}, {3, 2, 0, 1}},
    {0x01DA, {0x01DD, 0x01DE, 0x01DF, 0x01E0}, {2, 3, 4, 5}},
    {0x0227, {0x0228, 0x0229, 0x022A, 0x022B}, {0, 1, 2, 3}},
}};

constexpr std::array<PackedFloatLayout, 6> channelGainLayouts {{
    {0x0739, {0x0736, 0x073A, 0x073B, 0x073C}, {4, 0, 1, 2}},
    {0x077A, {0x077B, 0x077C, 0x077D, 0x077E}, {0, 1, 2, 3}},
    {0x07BD, {0x07BB, 0x07BC, 0x07BE, 0x07BF}, {3, 2, 0, 1}},
    {0x07FD, {0x07FA, 0x07FE, 0x07FF, 0x0800}, {4, 0, 1, 2}},
    {0x083E, {0x083F, 0x0840, 0x0841, 0x0842}, {0, 1, 2, 3}},
    {0x0881, {0x087F, 0x0880, 0x0882, 0x0883}, {3, 2, 0, 1}},
}};

constexpr std::array<PackedFloatLayout, 7> channelPanLayouts {{
    {0x0506, {0x0501, 0x0502, 0x0503, 0x0508}, {6, 5, 4, 1}},
    {0x0506, {0x0504, 0x0505, 0x0507, 0x050C}, {3, 2, 0, 5}},
    {0x050C, {0x050A, 0x050B, 0x050D, 0x0511}, {3, 2, 0, 4}},
    {0x0514, {0x050F, 0x0510, 0x0511, 0x0516}, {6, 5, 4, 1}},
    {0x0514, {0x0512, 0x0513, 0x0515, 0x051A}, {3, 2, 0, 5}},
    {0x051A, {0x0518, 0x0519, 0x051B, 0x051F}, {3, 2, 0, 4}},
    {0x0522, {0x051D, 0x051E, 0x051F, 0x0524}, {6, 5, 4, 1}},
}};

constexpr std::array<PackedFloatLayout, 6> channelCompressorLayouts {{
    {0x073D, {0x073E, 0x073F, 0x0740, 0x0741}, {0, 1, 2, 3}},
    {0x077C, {0x077F, 0x0780, 0x0781, 0x0782}, {2, 3, 4, 5}},
    {0x07C0, {0x07C1, 0x07C2, 0x07C3, 0x07C4}, {0, 1, 2, 3}},
    {0x0801, {0x0802, 0x0803, 0x0804, 0x0805}, {0, 1, 2, 3}},
    {0x0840, {0x0843, 0x0844, 0x0845, 0x0846}, {2, 3, 4, 5}},
    {0x0884, {0x0885, 0x0886, 0x0887, 0x0888}, {0, 1, 2, 3}},
}};

constexpr std::array<PackedFloatLayout, 5> busLevelLayouts {{
    {0x04C7, {0x04C8, 0x04C9, 0x04CA, 0x04CB}, {0, 1, 2, 3}},
    {0x0338, {0x033B, 0x033C, 0x033D, 0x033E}, {2, 3, 4, 5}},
    {0x0338, {0x0336, 0x0337, 0x033D, 0x033E}, {3, 2, 4, 5}},
    {0x03D2, {0x03D0, 0x03D1, 0x03D3, 0x03D4}, {3, 2, 0, 1}},
    {0x0418, {0x041B, 0x041C, 0x041D, 0x041E}, {2, 3, 4, 5}},
}};

constexpr PackedFloatLayout mainBalanceLayout {
    0x054C, {0x0547, 0x0548, 0x0549, 0x054E}, {6, 5, 4, 1}};

constexpr std::array<PackedFloatLayout, 3> busLimiterLayouts {{
    {0x0B42, {0x0B45, 0x0B46, 0x0B47, 0x0B48}, {2, 3, 4, 5}},
    {0x08FD, {0x08FE, 0x08FF, 0x0900, 0x0901}, {0, 1, 2, 3}},
    {0x08FD, {0x08F9, 0x08FA, 0x0900, 0x0901}, {5, 4, 2, 3}},
}};

constexpr std::array<qsizetype, 7> channelMuteOffsets {
    0x04CC, 0x04CD, 0x04CF, 0x04D0, 0x04D1, 0x04D2, 0x04D3};
constexpr std::array<qsizetype, 7> channelSoloOffsets {
    0x04DF, 0x04E0, 0x04E1, 0x04E2, 0x04E4, 0x04E5, 0x04E6};
constexpr std::array<qsizetype, 2> channelPhantomOffsets {0x0737, 0x0778};

struct FxLayout {
    qsizetype parameter1Offset;
    qsizetype parameter2Offset;
    qsizetype presetOffset;
};

constexpr std::array<FxLayout, 2> fxLayouts {{
    {0x0BC5, 0x0BC6, 0x0BC9},
    {0x0BCD, 0x0BCF, 0x0BD1},
}};

constexpr auto referenceOffsetSource = "reference SysEx calibration offsets; NEED_HARDWARE";

void appendParameter(ParsedSysExState& parsed, const QString& path, const double value)
{
    parsed.parameters.append(ExtractedParameter {
        .path = path,
        .value = value,
        .evidence = model::EvidenceStatus::Inferred,
        .source = QString::fromLatin1(referenceOffsetSource),
    });
}

template<std::size_t N>
void appendFloatSeries(ParsedSysExState& parsed, const QByteArrayView bytes,
                       const std::array<PackedFloatLayout, N>& layouts,
                       const QString& group, const QString& field)
{
    for (std::size_t index = 0; index < layouts.size(); ++index) {
        if (const auto value = decodePackedFloat(bytes, layouts[index]); value.has_value()) {
            appendParameter(parsed,
                            QStringLiteral("%1.%2.%3").arg(group).arg(index).arg(field),
                            static_cast<double>(*value));
        }
    }
}

template<std::size_t N>
void appendFlagSeries(ParsedSysExState& parsed, const QByteArrayView bytes,
                      const std::array<qsizetype, N>& offsets,
                      const QString& group, const QString& field)
{
    for (std::size_t index = 0; index < offsets.size(); ++index) {
        if (offsets[index] < 0 || offsets[index] >= bytes.size()) {
            continue;
        }
        parsed.flags.append(ExtractedFlag {
            .path = QStringLiteral("%1.%2.%3").arg(group).arg(index).arg(field),
            .value = static_cast<quint8>(bytes[offsets[index]]) != 0U,
            .evidence = model::EvidenceStatus::Inferred,
            .source = QString::fromLatin1(referenceOffsetSource),
        });
    }
}

std::optional<quint8> restorePackedByte(const QByteArrayView data, const qsizetype position)
{
    const qsizetype groupPosition = (position + 2) % 7;
    if (groupPosition == 0) {
        return std::nullopt;
    }
    if (position < groupPosition) {
        return static_cast<quint8>(data[position]);
    }
    const qsizetype msbOffset = position - groupPosition;
    if (msbOffset >= data.size()) {
        return static_cast<quint8>(data[position]);
    }
    quint8 value = static_cast<quint8>(data[position]);
    const quint8 msb = static_cast<quint8>(data[msbOffset]);
    const auto bitIndex = static_cast<unsigned int>(groupPosition - 1);
    if ((msb & static_cast<quint8>(1U << bitIndex)) != 0U) {
        value = static_cast<quint8>(value | 0x80U);
    }
    return value;
}

std::optional<QString> decodeChannelName(const QByteArrayView bytes, const qsizetype start)
{
    if (start < 0 || start + nameScanLength > bytes.size()) {
        return std::nullopt;
    }

    QByteArray restored;
    restored.reserve(nameScanLength);
    for (qsizetype offset = start; offset < start + nameScanLength; ++offset) {
        if (const auto value = restorePackedByte(bytes, offset); value.has_value()) {
            restored.append(static_cast<char>(*value));
        }
    }

    qsizetype firstPrintable = 0;
    while (firstPrintable < restored.size()
           && static_cast<quint8>(restored[firstPrintable]) < 0x20U) {
        ++firstPrintable;
    }
    const qsizetype nullPosition = restored.indexOf('\0', firstPrintable);
    const qsizetype end = nullPosition >= 0 ? nullPosition : restored.size();
    const QByteArray name = restored.mid(firstPrintable, end - firstPrintable);
    if (name.isEmpty()) {
        return std::nullopt;
    }
    return QString::fromUtf8(name);
}

} // namespace

SysExValidationResult validateSysEx(const QByteArrayView bytes) noexcept
{
    if (bytes.size() < 6) {
        return {false, false, QStringLiteral("SysEx message is shorter than 6 bytes")};
    }
    if (static_cast<quint8>(bytes.front()) != sysExStart) {
        return {false, false, QStringLiteral("missing SysEx start byte")};
    }
    if (static_cast<quint8>(bytes.back()) != sysExEnd) {
        return {false, false, QStringLiteral("missing SysEx end byte")};
    }

    const bool manufacturerMatches = static_cast<quint8>(bytes[1]) == behringerId[0]
        && static_cast<quint8>(bytes[2]) == behringerId[1]
        && static_cast<quint8>(bytes[3]) == behringerId[2];
    const bool modelMatches = static_cast<quint8>(bytes[4]) == flow8Model;
    if (!manufacturerMatches || !modelMatches) {
        return {true, false, QStringLiteral("valid SysEx framing but not a known FLOW 8 header")};
    }
    return {true, true, QStringLiteral("FLOW 8 SysEx header matches reference evidence")};
}

std::optional<float> decodePackedFloat(const QByteArrayView bytes,
                                       const PackedFloatLayout& layout) noexcept
{
    if (layout.msbOffset < 0 || layout.msbOffset >= bytes.size()) {
        return std::nullopt;
    }

    for (std::size_t index = 0; index < layout.dataOffsets.size(); ++index) {
        if (layout.dataOffsets[index] < 0 || layout.dataOffsets[index] >= bytes.size()
            || layout.bitIndices[index] < 0 || layout.bitIndices[index] > 7) {
            return std::nullopt;
        }
    }

    const quint8 msb = static_cast<quint8>(bytes[layout.msbOffset]);
    std::array<quint8, 4> decoded {};
    for (std::size_t index = 0; index < decoded.size(); ++index) {
        quint8 value = static_cast<quint8>(bytes[layout.dataOffsets[index]]);
        const auto bit = static_cast<unsigned int>(layout.bitIndices[index]);
        if ((msb & static_cast<quint8>(1U << bit)) != 0U) {
            value = static_cast<quint8>(value | 0x80U);
        }
        decoded[index] = value;
    }

    const quint32 bits = qFromLittleEndian<quint32>(decoded.data());
    const float value = std::bit_cast<float>(bits);
    if (!std::isfinite(value)) {
        return std::nullopt;
    }
    return value;
}

ParsedSysExState parseReferenceStateDump(const QByteArrayView bytes)
{
    ParsedSysExState parsed;
    parsed.validation = validateSysEx(bytes);
    parsed.raw = QByteArray(bytes.data(), bytes.size());
    if (!parsed.validation.valid || !parsed.validation.isFlow8) {
        return parsed;
    }

    parsed.channelNames.reserve(7);
    for (qsizetype index = 0; index < 7; ++index) {
        parsed.channelNames.append(decodeChannelName(bytes, namesStart + index * namesStride));
    }

    appendFloatSeries(parsed, bytes, channelLevelLayouts, QStringLiteral("channel"),
                      QStringLiteral("level_db"));
    appendFloatSeries(parsed, bytes, channelGainLayouts, QStringLiteral("channel"),
                      QStringLiteral("gain_db"));
    appendFloatSeries(parsed, bytes, channelPanLayouts, QStringLiteral("channel"),
                      QStringLiteral("pan"));
    appendFloatSeries(parsed, bytes, channelCompressorLayouts, QStringLiteral("channel"),
                      QStringLiteral("compressor"));
    appendFlagSeries(parsed, bytes, channelMuteOffsets, QStringLiteral("channel"),
                     QStringLiteral("muted"));
    appendFlagSeries(parsed, bytes, channelSoloOffsets, QStringLiteral("channel"),
                     QStringLiteral("soloed"));
    appendFlagSeries(parsed, bytes, channelPhantomOffsets, QStringLiteral("channel"),
                     QStringLiteral("phantom_48v"));

    appendFloatSeries(parsed, bytes, busLevelLayouts, QStringLiteral("bus"),
                      QStringLiteral("level_db"));
    if (const auto balance = decodePackedFloat(bytes, mainBalanceLayout); balance.has_value()) {
        appendParameter(parsed, QStringLiteral("bus.0.balance"), static_cast<double>(*balance));
    }
    appendFloatSeries(parsed, bytes, busLimiterLayouts, QStringLiteral("bus"),
                      QStringLiteral("limiter_db"));

    for (std::size_t index = 0; index < fxLayouts.size(); ++index) {
        const auto& layout = fxLayouts[index];
        if (layout.parameter1Offset >= bytes.size() || layout.parameter2Offset >= bytes.size()
            || layout.presetOffset >= bytes.size()) {
            continue;
        }
        appendParameter(parsed, QStringLiteral("fx.%1.parameter1_percent").arg(index),
                        static_cast<quint8>(bytes[layout.parameter1Offset]));
        appendParameter(parsed, QStringLiteral("fx.%1.parameter2_percent").arg(index),
                        static_cast<quint8>(bytes[layout.parameter2Offset]));
        const quint8 preset = static_cast<quint8>(bytes[layout.presetOffset]);
        if (preset > 0U && preset <= 16U) {
            appendParameter(parsed, QStringLiteral("fx.%1.preset_reference_index").arg(index),
                            static_cast<double>(preset - 1U));
        }
    }
    return parsed;
}

} // namespace flow8::protocol
