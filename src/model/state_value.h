#pragma once

#include "model/evidence_status.h"

#include <QString>

#include <optional>
#include <utility>

namespace flow8::model {

[[nodiscard]] constexpr int evidencePriority(const EvidenceStatus status) noexcept
{
    switch (status) {
    case EvidenceStatus::Verified: return 3;
    case EvidenceStatus::Inferred: return 2;
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
                                      const EvidenceStatus evidence, QString source)
{
    if (evidence == EvidenceStatus::Blocked) {
        return false;
    }
    if (target.value.has_value()
        && evidencePriority(evidence) < evidencePriority(target.evidence)) {
        return false;
    }
    target = StateValue<T>::known(std::move(newValue), evidence, std::move(source));
    return true;
}

} // namespace flow8::model
