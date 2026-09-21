#pragma once

#include "model/evidence_status.h"

#include <QByteArray>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>

#include <optional>

namespace flow8::test {

struct CaptureFixture {
    QString id;
    QString source;
    QString description;
    QString captureKind;
    model::EvidenceStatus evidence {model::EvidenceStatus::Unknown};
    QByteArray raw;
    QJsonObject expected;
};

struct FixtureLoadResult {
    std::optional<CaptureFixture> fixture;
    QString error;

    [[nodiscard]] bool ok() const noexcept { return fixture.has_value(); }
};

[[nodiscard]] FixtureLoadResult loadCaptureFixture(const QString& manifestPath);

} // namespace flow8::test
