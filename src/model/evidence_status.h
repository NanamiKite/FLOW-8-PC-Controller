#pragma once

#include <QStringView>

namespace flow8::model {

enum class EvidenceStatus {
    Verified,
    Inferred,
    Unknown,
    Blocked,
};

[[nodiscard]] constexpr QStringView evidenceStatusName(const EvidenceStatus status) noexcept
{
    switch (status) {
    case EvidenceStatus::Verified:
        return u"VERIFIED";
    case EvidenceStatus::Inferred:
        return u"INFERRED";
    case EvidenceStatus::Unknown:
        return u"UNKNOWN";
    case EvidenceStatus::Blocked:
        return u"BLOCKED";
    }

    return u"UNKNOWN";
}

} // namespace flow8::model
