#pragma once

#include <QString>
#include <QStringView>

namespace flow8::model {

// Capability evidence describes what the product exposes. It is deliberately
// separate from protocol EvidenceStatus: an official feature does not prove a
// BLE address, packet layout, or successful hardware operation.
enum class CapabilitySource {
    OfficialManual,
    OfficialApk,
    ReferenceProject,
    ProjectDesign,
    Unknown,
};

[[nodiscard]] constexpr QStringView capabilitySourceName(
    const CapabilitySource source) noexcept
{
    switch (source) {
    case CapabilitySource::OfficialManual: return u"OfficialManual";
    case CapabilitySource::OfficialApk: return u"OfficialApk";
    case CapabilitySource::ReferenceProject: return u"ReferenceProject";
    case CapabilitySource::ProjectDesign: return u"ProjectDesign";
    case CapabilitySource::Unknown: return u"Unknown";
    }
    return u"Unknown";
}

struct CapabilityEvidence {
    CapabilitySource source {CapabilitySource::Unknown};
    QString reference;
};

} // namespace flow8::model
