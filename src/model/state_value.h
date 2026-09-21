#pragma once

#include "model/evidence_status.h"

#include <QString>

#include <optional>
#include <utility>

namespace flow8::model {

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

} // namespace flow8::model
