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

struct LowCutLayout {
    qsizetype lowOffset;
    qsizetype highOffset;
};

constexpr std::array<LowCutLayout, 6> channelLowCutLayouts {{
    {0x0742, 0x0743}, {0x0784, 0x0785}, {0x07C5, 0x07C6},
    {0x0806, 0x0807}, {0x0848, 0x0849}, {0x0889, 0x088A},
}};

constexpr std::array<PackedFloatLayout, 7> channelEqLowLayouts {{
    {0x0744, {0x0742, 0x0743, 0x0749, 0x074A}, {3, 2, 4, 5}},
    {0x078A, {0x0785, 0x0786, 0x078B, 0x078C}, {6, 5, 0, 1}},
    {0x07C9, {0x07C5, 0x07C6, 0x07CC, 0x07CD}, {5, 4, 2, 3}},
    {0x0808, {0x0806, 0x0807, 0x080D, 0x080E}, {3, 2, 4, 5}},
    {0x084E, {0x0849, 0x084A, 0x084F, 0x0850}, {6, 5, 0, 1}},
    {0x088D, {0x0889, 0x088A, 0x0890, 0x0891}, {5, 4, 2, 3}},
    {0x08CC, {0x08CA, 0x08CB, 0x08D1, 0x08D2}, {3, 2, 4, 5}},
}};

constexpr std::array<PackedFloatLayout, 7> channelEqLowMidLayouts {{
    {0x074B, {0x0747, 0x0748, 0x074E, 0x074F}, {5, 4, 2, 3}},
    {0x078A, {0x0788, 0x0789, 0x078F, 0x0790}, {3, 2, 4, 5}},
    {0x07D0, {0x07CB, 0x07CC, 0x07D1, 0x07D2}, {6, 5, 0, 1}},
    {0x080F, {0x080B, 0x080C, 0x0812, 0x0813}, {5, 4, 2, 3}},
    {0x084E, {0x084C, 0x084D, 0x0853, 0x0854}, {3, 2, 4, 5}},
    {0x0894, {0x088F, 0x0890, 0x0895, 0x0896}, {6, 5, 0, 1}},
    {0x08D3, {0x08CF, 0x08D0, 0x08D6, 0x08D7}, {5, 4, 2, 3}},
}};

constexpr std::array<PackedFloatLayout, 7> channelEqHighMidLayouts {{
    {0x0752, {0x074D, 0x074E, 0x0753, 0x0754}, {6, 5, 0, 1}},
    {0x0791, {0x078D, 0x078E, 0x0794, 0x0795}, {5, 4, 2, 3}},
    {0x07D0, {0x07CE, 0x07CF, 0x07D5, 0x07D6}, {3, 2, 4, 5}},
    {0x0816, {0x0811, 0x0812, 0x0817, 0x0818}, {6, 5, 0, 1}},
    {0x0855, {0x0851, 0x0852, 0x0858, 0x0859}, {5, 4, 2, 3}},
    {0x0894, {0x0892, 0x0893, 0x0899, 0x089A}, {3, 2, 4, 5}},
    {0x08DA, {0x08D5, 0x08D6, 0x08DB, 0x08DC}, {6, 5, 0, 1}},
}};

constexpr std::array<PackedFloatLayout, 7> channelEqHighLayouts {{
    {0x0752, {0x0750, 0x0751, 0x0757, 0x0758}, {3, 2, 4, 5}},
    {0x0798, {0x0793, 0x0794, 0x0799, 0x079A}, {6, 5, 0, 1}},
    {0x07D7, {0x07D3, 0x07D4, 0x07DA, 0x07DB}, {5, 4, 2, 3}},
    {0x0816, {0x0814, 0x0815, 0x081B, 0x081C}, {3, 2, 4, 5}},
    {0x085C, {0x0857, 0x0858, 0x085D, 0x085E}, {6, 5, 0, 1}},
    {0x089B, {0x0897, 0x0898, 0x089E, 0x089F}, {5, 4, 2, 3}},
    {0x08DA, {0x08D8, 0x08D9, 0x08DF, 0x08E0}, {3, 2, 4, 5}},
}};

constexpr std::array<PackedFloatLayout, 7> channelSendMonitor1Layouts {{
    {0x0052, {0x0050, 0x0051, 0x0053, 0x0054}, {3, 2, 0, 1}},
    {0x0098, {0x009B, 0x009C, 0x009D, 0x009E}, {2, 3, 4, 5}},
    {0x00E5, {0x00E6, 0x00E7, 0x00E8, 0x00E9}, {0, 1, 2, 3}},
    {0x0132, {0x0130, 0x0131, 0x0133, 0x0134}, {3, 2, 0, 1}},
    {0x0178, {0x017B, 0x017C, 0x017D, 0x017E}, {2, 3, 4, 5}},
    {0x01C5, {0x01C6, 0x01C7, 0x01C8, 0x01C9}, {0, 1, 2, 3}},
    {0x0212, {0x0210, 0x0211, 0x0213, 0x0214}, {3, 2, 0, 1}},
}};

constexpr std::array<PackedFloatLayout, 7> channelSendMonitor2Layouts {{
    {0x0052, {0x0053, 0x0054, 0x0057, 0x0058}, {0, 1, 4, 5}},
    {0x0098, {0x0096, 0x0097, 0x009D, 0x009E}, {3, 2, 4, 5}},
    {0x00EC, {0x00E8, 0x00E9, 0x00ED, 0x00EE}, {5, 4, 0, 1}},
    {0x0132, {0x0133, 0x0134, 0x0137, 0x0138}, {0, 1, 4, 5}},
    {0x0178, {0x0176, 0x0177, 0x017D, 0x017E}, {3, 2, 4, 5}},
    {0x01CC, {0x01C8, 0x01C9, 0x01CD, 0x01CE}, {5, 4, 0, 1}},
    {0x0212, {0x0213, 0x0214, 0x0217, 0x0218}, {0, 1, 4, 5}},
}};

constexpr std::array<PackedFloatLayout, 7> channelSendFx1Layouts {{
    {0x0059, {0x005A, 0x005B, 0x005C, 0x005D}, {0, 1, 2, 3}},
    {0x00A6, {0x00A4, 0x00A5, 0x00A7, 0x00A8}, {3, 2, 0, 1}},
    {0x00EC, {0x00EF, 0x00F0, 0x00F1, 0x00F2}, {2, 3, 4, 5}},
    {0x0139, {0x013A, 0x013B, 0x013C, 0x013D}, {0, 1, 2, 3}},
    {0x0186, {0x0184, 0x0185, 0x0187, 0x0188}, {3, 2, 0, 1}},
    {0x01CC, {0x01CF, 0x01D0, 0x01D1, 0x01D2}, {2, 3, 4, 5}},
    {0x0219, {0x021A, 0x021B, 0x021C, 0x021D}, {0, 1, 2, 3}},
}};

constexpr std::array<PackedFloatLayout, 7> channelSendFx2Layouts {{
    {0x0060, {0x005E, 0x005F, 0x0061, 0x0062}, {3, 2, 0, 1}},
    {0x00A6, {0x00A9, 0x00AA, 0x00AB, 0x00AC}, {2, 3, 4, 5}},
    {0x00F3, {0x00F4, 0x00F5, 0x00F6, 0x00F7}, {0, 1, 2, 3}},
    {0x0140, {0x013E, 0x013F, 0x0141, 0x0142}, {3, 2, 0, 1}},
    {0x0186, {0x0189, 0x018A, 0x018B, 0x018C}, {2, 3, 4, 5}},
    {0x01D3, {0x01D4, 0x01D5, 0x01D6, 0x01D7}, {0, 1, 2, 3}},
    {0x0220, {0x021E, 0x021F, 0x0221, 0x0222}, {3, 2, 0, 1}},
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

// The reference controller applies graphic EQ only to Main, Monitor 1, and
// Monitor 2. The reference constants for FX buses are intentionally omitted.
constexpr std::array<PackedFloatLayout, 3> graphicEq62HzLayouts {{
    {0x0B49, {0x0B45, 0x0B46, 0x0B4C, 0x0B4D}, {5, 4, 2, 3}},
    {0x0904, {0x08FF, 0x0900, 0x0905, 0x0906}, {6, 5, 0, 1}},
    {0x0904, {0x08FF, 0x0900, 0x0901, 0x0906}, {6, 5, 4, 1}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq125HzLayouts {{
    {0x0B50, {0x0B4B, 0x0B4C, 0x0B51, 0x0B52}, {6, 5, 0, 1}},
    {0x0904, {0x0902, 0x0903, 0x0909, 0x090A}, {3, 2, 4, 5}},
    {0x0904, {0x0902, 0x0903, 0x0905, 0x090A}, {3, 2, 0, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq250HzLayouts {{
    {0x0B50, {0x0B4E, 0x0B4F, 0x0B55, 0x0B56}, {3, 2, 4, 5}},
    {0x090B, {0x0907, 0x0908, 0x090E, 0x090F}, {5, 4, 2, 3}},
    {0x0909, {0x0907, 0x0908, 0x090B, 0x090F}, {3, 2, 1, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq500HzLayouts {{
    {0x0B57, {0x0B53, 0x0B54, 0x0B5A, 0x0B5B}, {5, 4, 2, 3}},
    {0x0912, {0x090D, 0x090E, 0x0913, 0x0914}, {6, 5, 0, 1}},
    {0x090E, {0x090D, 0x090F, 0x0912, 0x0914}, {2, 0, 3, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq1KHzLayouts {{
    {0x0B5E, {0x0B59, 0x0B5A, 0x0B5F, 0x0B60}, {6, 5, 0, 1}},
    {0x0912, {0x0910, 0x0911, 0x0917, 0x0918}, {3, 2, 4, 5}},
    {0x090E, {0x090D, 0x090F, 0x0912, 0x0914}, {2, 0, 3, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq2KHzLayouts {{
    {0x0B5E, {0x0B5C, 0x0B5D, 0x0B63, 0x0B64}, {3, 2, 4, 5}},
    {0x0919, {0x0915, 0x0916, 0x091C, 0x091D}, {5, 4, 2, 3}},
    {0x0917, {0x0915, 0x0916, 0x0919, 0x091D}, {3, 2, 1, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq4KHzLayouts {{
    {0x0B65, {0x0B61, 0x0B62, 0x0B68, 0x0B69}, {5, 4, 2, 3}},
    {0x0920, {0x091B, 0x091C, 0x0921, 0x0922}, {6, 5, 0, 1}},
    {0x091C, {0x091B, 0x091D, 0x0920, 0x0922}, {2, 0, 3, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq8KHzLayouts {{
    {0x0B6C, {0x0B67, 0x0B68, 0x0B6D, 0x0B6E}, {6, 5, 0, 1}},
    {0x0920, {0x091E, 0x091F, 0x0925, 0x0926}, {3, 2, 4, 5}},
    {0x091C, {0x091B, 0x091D, 0x0920, 0x0922}, {2, 0, 3, 5}},
}};
constexpr std::array<PackedFloatLayout, 3> graphicEq16KHzLayouts {{
    {0x0B6C, {0x0B6A, 0x0B6B, 0x0B71, 0x0B72}, {3, 2, 4, 5}},
    {0x0927, {0x0923, 0x0924, 0x092A, 0x092B}, {5, 4, 2, 3}},
    {0x0925, {0x0923, 0x0924, 0x0927, 0x092B}, {3, 2, 1, 5}},
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

constexpr auto referenceParserSource =
    "reference/flow-8-midi/src/service/sysex_parser.rs";

QString referenceSource(const char* symbol)
{
    return QStringLiteral("%1:%2; NEED_HARDWARE")
        .arg(QString::fromLatin1(referenceParserSource), QString::fromLatin1(symbol));
}

QVector<qsizetype> floatOffsets(const PackedFloatLayout& layout)
{
    return {layout.msbOffset, layout.dataOffsets[0], layout.dataOffsets[1],
            layout.dataOffsets[2], layout.dataOffsets[3]};
}

void appendFloatParameter(ParsedSysExState& parsed, const QString& path, const double value,
                          const PackedFloatLayout& layout, const char* sourceSymbol)
{
    parsed.parameters.append(ExtractedParameter {
        .path = path,
        .value = value,
        .evidence = model::EvidenceStatus::Inferred,
        .source = referenceSource(sourceSymbol),
        .offsets = floatOffsets(layout),
        .encodedWidth = 5,
        .encoding = QStringLiteral("7-bit packed IEEE-754 little-endian float"),
        .decodedType = QStringLiteral("float32"),
    });
}

void appendByteParameter(ParsedSysExState& parsed, const QString& path, const double value,
                         const qsizetype offset, const char* sourceSymbol,
                         const QString& decodedType)
{
    parsed.parameters.append(ExtractedParameter {
        .path = path,
        .value = value,
        .evidence = model::EvidenceStatus::Inferred,
        .source = referenceSource(sourceSymbol),
        .offsets = {offset},
        .encodedWidth = 1,
        .encoding = QStringLiteral("unsigned 7-bit byte"),
        .decodedType = decodedType,
    });
}

template<std::size_t N>
void appendFloatSeries(ParsedSysExState& parsed, const QByteArrayView bytes,
                       const std::array<PackedFloatLayout, N>& layouts,
                       const QString& group, const QString& field, const char* sourceSymbol)
{
    for (std::size_t index = 0; index < layouts.size(); ++index) {
        if (const auto value = decodePackedFloat(bytes, layouts[index]); value.has_value()) {
            appendFloatParameter(parsed,
                                 QStringLiteral("%1.%2.%3").arg(group).arg(index).arg(field),
                                 static_cast<double>(*value), layouts[index], sourceSymbol);
        }
    }
}

template<std::size_t N>
void appendFlagSeries(ParsedSysExState& parsed, const QByteArrayView bytes,
                      const std::array<qsizetype, N>& offsets,
                      const QString& group, const QString& field, const char* sourceSymbol)
{
    for (std::size_t index = 0; index < offsets.size(); ++index) {
        if (offsets[index] < 0 || offsets[index] >= bytes.size()) {
            continue;
        }
        const quint8 rawValue = static_cast<quint8>(bytes[offsets[index]]);
        if (rawValue > 1U) {
            continue;
        }
        parsed.flags.append(ExtractedFlag {
            .path = QStringLiteral("%1.%2.%3").arg(group).arg(index).arg(field),
            .value = rawValue != 0U,
            .evidence = model::EvidenceStatus::Inferred,
            .source = referenceSource(sourceSymbol),
            .offsets = {offsets[index]},
            .encodedWidth = 1,
            .encoding = QStringLiteral("boolean byte (0x00/0x01)"),
            .decodedType = QStringLiteral("bool"),
        });
    }
}

template<std::size_t N>
void appendGraphicEqSeries(ParsedSysExState& parsed, const QByteArrayView bytes,
                           const std::array<PackedFloatLayout, N>& layouts,
                           const QString& bandName, const char* sourceSymbol)
{
    appendFloatSeries(parsed, bytes, layouts, QStringLiteral("bus"),
                      QStringLiteral("graphic_eq.%1_gain_db").arg(bandName), sourceSymbol);
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

QStringView dumpCompletenessName(const DumpCompleteness completeness) noexcept
{
    switch (completeness) {
    case DumpCompleteness::Invalid: return u"INVALID";
    case DumpCompleteness::Partial: return u"PARTIAL";
    case DumpCompleteness::CompleteReferenceLayout: return u"COMPLETE_REFERENCE_LAYOUT";
    }
    return u"INVALID";
}

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
    for (qsizetype index = 1; index < bytes.size() - 1; ++index) {
        if ((static_cast<quint8>(bytes[index]) & 0x80U) != 0U) {
            return {false, false,
                    QStringLiteral("SysEx data byte at offset %1 is not 7-bit safe")
                        .arg(index)};
        }
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
        parsed.completeness = DumpCompleteness::Invalid;
        return parsed;
    }
    parsed.completeness = bytes.size() == referenceStateDumpByteCount
        ? DumpCompleteness::CompleteReferenceLayout
        : DumpCompleteness::Partial;

    parsed.channelNames.reserve(7);
    for (qsizetype index = 0; index < 7; ++index) {
        parsed.channelNames.append(decodeChannelName(bytes, namesStart + index * namesStride));
    }

    appendFloatSeries(parsed, bytes, channelLevelLayouts, QStringLiteral("channel"),
                      QStringLiteral("level_db"), "CHANNEL_LEVELS");
    appendFloatSeries(parsed, bytes, channelGainLayouts, QStringLiteral("channel"),
                      QStringLiteral("gain_db"), "CHANNEL_GAINS");
    appendFloatSeries(parsed, bytes, channelPanLayouts, QStringLiteral("channel"),
                      QStringLiteral("pan"), "CHANNEL_PANS");
    appendFloatSeries(parsed, bytes, channelCompressorLayouts, QStringLiteral("channel"),
                      QStringLiteral("compressor"), "CHANNEL_COMPRESSORS");

    for (std::size_t index = 0; index < channelLowCutLayouts.size(); ++index) {
        const auto& layout = channelLowCutLayouts[index];
        if (layout.lowOffset < 0 || layout.highOffset < 0 || layout.lowOffset >= bytes.size()
            || layout.highOffset >= bytes.size()) {
            continue;
        }
        const auto low = static_cast<quint8>(bytes[layout.lowOffset]);
        const auto high = static_cast<quint8>(bytes[layout.highOffset]);
        const auto hz = static_cast<quint16>(low + static_cast<quint16>(high) * 128U);
        parsed.parameters.append(ExtractedParameter {
            .path = QStringLiteral("channel.%1.low_cut_hz").arg(index),
            .value = static_cast<double>(hz),
            .evidence = model::EvidenceStatus::Inferred,
            .source = referenceSource("CHANNEL_LOW_CUTS/decode_lowcut"),
            .offsets = {layout.lowOffset, layout.highOffset},
            .encodedWidth = 2,
            .encoding = QStringLiteral("uint14 little-endian 7-bit bytes (lo + hi*128)"),
            .decodedType = QStringLiteral("uint16 Hz"),
        });
    }

    appendFloatSeries(parsed, bytes, channelEqLowLayouts, QStringLiteral("channel"),
                      QStringLiteral("eq.low_gain_db"), "CHANNEL_EQ_LOW");
    appendFloatSeries(parsed, bytes, channelEqLowMidLayouts, QStringLiteral("channel"),
                      QStringLiteral("eq.low_mid_gain_db"), "CHANNEL_EQ_LOW_MID");
    appendFloatSeries(parsed, bytes, channelEqHighMidLayouts, QStringLiteral("channel"),
                      QStringLiteral("eq.high_mid_gain_db"), "CHANNEL_EQ_HI_MID");
    appendFloatSeries(parsed, bytes, channelEqHighLayouts, QStringLiteral("channel"),
                      QStringLiteral("eq.high_gain_db"), "CHANNEL_EQ_HI");
    appendFloatSeries(parsed, bytes, channelSendMonitor1Layouts, QStringLiteral("channel"),
                      QStringLiteral("send.monitor1_level_db"), "CHANNEL_SEND_MON1");
    appendFloatSeries(parsed, bytes, channelSendMonitor2Layouts, QStringLiteral("channel"),
                      QStringLiteral("send.monitor2_level_db"), "CHANNEL_SEND_MON2");
    appendFloatSeries(parsed, bytes, channelSendFx1Layouts, QStringLiteral("channel"),
                      QStringLiteral("send.fx1_level_db"), "CHANNEL_SEND_FX1");
    appendFloatSeries(parsed, bytes, channelSendFx2Layouts, QStringLiteral("channel"),
                      QStringLiteral("send.fx2_level_db"), "CHANNEL_SEND_FX2");

    appendFlagSeries(parsed, bytes, channelMuteOffsets, QStringLiteral("channel"),
                     QStringLiteral("muted"), "CHANNEL_MUTES");
    appendFlagSeries(parsed, bytes, channelSoloOffsets, QStringLiteral("channel"),
                     QStringLiteral("soloed"), "CHANNEL_SOLOS");
    appendFlagSeries(parsed, bytes, channelPhantomOffsets, QStringLiteral("channel"),
                     QStringLiteral("phantom_48v"), "CHANNEL_PHANTOMS");

    appendFloatSeries(parsed, bytes, busLevelLayouts, QStringLiteral("bus"),
                      QStringLiteral("level_db"), "BUS_LEVELS");
    if (const auto balance = decodePackedFloat(bytes, mainBalanceLayout); balance.has_value()) {
        appendFloatParameter(parsed, QStringLiteral("bus.0.balance"),
                             static_cast<double>(*balance), mainBalanceLayout,
                             "BUS_BALANCES[Main]");
    }
    appendFloatSeries(parsed, bytes, busLimiterLayouts, QStringLiteral("bus"),
                      QStringLiteral("limiter_db"), "BUS_LIMITERS");

    appendGraphicEqSeries(parsed, bytes, graphicEq62HzLayouts, QStringLiteral("62hz"),
                          "NINE_BAND_62HZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq125HzLayouts, QStringLiteral("125hz"),
                          "NINE_BAND_125HZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq250HzLayouts, QStringLiteral("250hz"),
                          "NINE_BAND_250HZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq500HzLayouts, QStringLiteral("500hz"),
                          "NINE_BAND_500HZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq1KHzLayouts, QStringLiteral("1khz"),
                          "NINE_BAND_1KHZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq2KHzLayouts, QStringLiteral("2khz"),
                          "NINE_BAND_2KHZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq4KHzLayouts, QStringLiteral("4khz"),
                          "NINE_BAND_4KHZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq8KHzLayouts, QStringLiteral("8khz"),
                          "NINE_BAND_8KHZ[Main,Mon1,Mon2]");
    appendGraphicEqSeries(parsed, bytes, graphicEq16KHzLayouts, QStringLiteral("16khz"),
                          "NINE_BAND_16KHZ[Main,Mon1,Mon2]");

    for (std::size_t index = 0; index < fxLayouts.size(); ++index) {
        const auto& layout = fxLayouts[index];
        if (layout.parameter1Offset >= bytes.size() || layout.parameter2Offset >= bytes.size()
            || layout.presetOffset >= bytes.size()) {
            continue;
        }
        appendByteParameter(parsed, QStringLiteral("fx.%1.parameter1_percent").arg(index),
                            static_cast<quint8>(bytes[layout.parameter1Offset]),
                            layout.parameter1Offset, "FX_SYSEX.param1", QStringLiteral("uint8 percent"));
        appendByteParameter(parsed, QStringLiteral("fx.%1.parameter2_percent").arg(index),
                            static_cast<quint8>(bytes[layout.parameter2Offset]),
                            layout.parameter2Offset, "FX_SYSEX.param2", QStringLiteral("uint8 percent"));
        const quint8 preset = static_cast<quint8>(bytes[layout.presetOffset]);
        if (preset > 0U && preset <= 16U) {
            appendByteParameter(parsed,
                                QStringLiteral("fx.%1.preset_reference_index").arg(index),
                                static_cast<double>(preset - 1U), layout.presetOffset,
                                "FX_SYSEX.preset", QStringLiteral("uint8 zero-based index"));
        }
    }
    return parsed;
}

} // namespace flow8::protocol
