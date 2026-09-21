#pragma once

#include "model/evidence_status.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QString>
#include <QVector>

#include <array>
#include <optional>

namespace flow8::protocol {

inline constexpr qsizetype referenceStateDumpByteCount = 3068;

enum class DumpCompleteness {
    Invalid,
    Partial,
    CompleteReferenceLayout,
};

struct SysExValidationResult {
    bool valid {};
    bool isFlow8 {};
    QString message;
};

struct PackedFloatLayout {
    qsizetype msbOffset {};
    std::array<qsizetype, 4> dataOffsets {};
    std::array<int, 4> bitIndices {};
};

struct ExtractedParameter {
    QString path;
    double value {};
    model::EvidenceStatus evidence {model::EvidenceStatus::Inferred};
    QString source;
    QVector<qsizetype> offsets;
    qsizetype encodedWidth {};
    QString encoding;
    QString decodedType;
};

struct ExtractedFlag {
    QString path;
    bool value {};
    model::EvidenceStatus evidence {model::EvidenceStatus::Inferred};
    QString source;
    QVector<qsizetype> offsets;
    qsizetype encodedWidth {};
    QString encoding;
    QString decodedType;
};

struct ParsedSysExState {
    SysExValidationResult validation;
    DumpCompleteness completeness {DumpCompleteness::Invalid};
    QByteArray raw;
    QVector<std::optional<QString>> channelNames;
    QVector<ExtractedParameter> parameters;
    QVector<ExtractedFlag> flags;
};

[[nodiscard]] QStringView dumpCompletenessName(DumpCompleteness completeness) noexcept;

[[nodiscard]] SysExValidationResult validateSysEx(QByteArrayView bytes) noexcept;
[[nodiscard]] std::optional<float> decodePackedFloat(QByteArrayView bytes,
                                                     const PackedFloatLayout& layout) noexcept;
[[nodiscard]] ParsedSysExState parseReferenceStateDump(QByteArrayView bytes);

} // namespace flow8::protocol
