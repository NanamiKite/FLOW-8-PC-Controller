#pragma once

#include <QStringView>

namespace flow8::model {

enum class EvidenceStatus {
    // Direct observation from a physical FLOW 8. No current project value may
    // use this status until a reproducible hardware capture exists.
    VerifiedFromDevice,
    // Static semantics recovered from the official FLOW Mix APK. This proves
    // application intent, not the on-wire payload or hardware behaviour.
    VerifiedFromApk,
    // Deterministic offline evidence such as an official document or a
    // validated codec fixture. Kept separate from device verification.
    VerifiedOffline,
    // Source compatibility for older call sites. New code should choose one
    // of the explicit statuses above.
    Verified = VerifiedOffline,
    Inferred,
    Unknown,
    Blocked,
    Synthetic,
};

[[nodiscard]] constexpr QStringView evidenceStatusName(const EvidenceStatus status) noexcept
{
    switch (status) {
    case EvidenceStatus::VerifiedFromDevice:
        return u"VERIFIED_FROM_DEVICE";
    case EvidenceStatus::VerifiedFromApk:
        return u"VERIFIED_FROM_APK";
    case EvidenceStatus::VerifiedOffline:
        return u"VERIFIED_OFFLINE";
    case EvidenceStatus::Inferred:
        return u"INFERRED";
    case EvidenceStatus::Unknown:
        return u"UNKNOWN";
    case EvidenceStatus::Blocked:
        return u"BLOCKED";
    case EvidenceStatus::Synthetic:
        return u"SYNTHETIC";
    }

    return u"UNKNOWN";
}

} // namespace flow8::model
