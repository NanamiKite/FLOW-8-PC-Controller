#pragma once

#include "model/evidence_status.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QString>
#include <QVector>

#include <array>
#include <optional>

namespace flow8::protocol {

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
};

struct ExtractedFlag {
    QString path;
    bool value {};
    model::EvidenceStatus evidence {model::EvidenceStatus::Inferred};
    QString source;
};

struct ParsedSysExState {
    SysExValidationResult validation;
    QByteArray raw;
    QVector<std::optional<QString>> channelNames;
    QVector<ExtractedParameter> parameters;
    QVector<ExtractedFlag> flags;
};

[[nodiscard]] SysExValidationResult validateSysEx(QByteArrayView bytes) noexcept;
[[nodiscard]] std::optional<float> decodePackedFloat(QByteArrayView bytes,
                                                     const PackedFloatLayout& layout) noexcept;
[[nodiscard]] ParsedSysExState parseReferenceStateDump(QByteArrayView bytes);

} // namespace flow8::protocol
