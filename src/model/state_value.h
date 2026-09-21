#pragma once

#include "model/evidence_status.h"

#include <QString>

#include <optional>
#include <utility>

namespace flow8::model {

[[nodiscard]] constexpr int evidencePriority(const EvidenceStatus status) noexcept
{
    switch (status) {
    case EvidenceStatus::VerifiedFromDevice: return 6;
    case EvidenceStatus::VerifiedOffline: return 5;
    case EvidenceStatus::VerifiedFromApk: return 4;
    case EvidenceStatus::Inferred: return 3;
    case EvidenceStatus::Synthetic: return 2;
    case EvidenceStatus::Unknown: return 1;
    case EvidenceStatus::Blocked: return 0;
    }
    return 0;
}

template<typename T>
struct StateValue {
    std::optional<T> value;
    EvidenceStatus evidence {EvidenceStatus::Unknown};
    QString source;

    [[nodiscard]] bool isKnown() const noexcept { return value.has_value(); }

    static StateValue known(T newValue, EvidenceStatus status, QString sourceDescription = {})
    {
        return StateValue {
            .value = std::move(newValue),
            .evidence = status,
            .source = std::move(sourceDescription),
        };
    }
};

template<typename T>
[[nodiscard]] bool mergeObservedValue(StateValue<T>& target, T newValue,
                                      EvidenceStatus evidence, QString source)
{
    if (evidence == EvidenceStatus::Blocked) {
        return false;
    }
    // Simulator callers historically supplied UNKNOWN plus an explicit
    // SYNTHETIC source. Normalize that combination at the state boundary so
    // simulated values can never be mistaken for hardware observations.
    if (evidence == EvidenceStatus::Unknown
        && source.contains(QStringLiteral("SYNTHETIC"), Qt::CaseInsensitive)) {
        evidence = EvidenceStatus::Synthetic;
    }
    if (target.value.has_value()
        && evidencePriority(evidence) < evidencePriority(target.evidence)) {
        return false;
    }
    target = StateValue<T>::known(std::move(newValue), evidence, std::move(source));
    return true;
}

} // namespace flow8::model
