#pragma once

#include "model/evidence_status.h"
#include "model/endpoint.h"
#include "protocol/apk_command_catalog.h"

#include <QByteArray>
#include <QString>

#include <array>
#include <optional>

namespace flow8::protocol {

// Transport-neutral semantic input to the APK-confirmed 0x06 codec. The
// endpoint enum keeps protocol numbers out of UI and mixer code.
struct RouteLevelCommand {
    model::EndpointId sourceEndpoint {model::EndpointId::Input1};
    model::EndpointId destinationEndpoint {model::EndpointId::MainLr};
    double normalizedValue {};
    // Evidence for the operation's meaning only. It does not describe the
    // caller-provided value or promote it to an observed device state.
    model::EvidenceStatus semanticEvidence {model::EvidenceStatus::VerifiedFromApk};

    [[nodiscard]] bool isDestinationMaster() const noexcept
    {
        return sourceEndpoint == destinationEndpoint
            && model::isDestinationEndpoint(destinationEndpoint);
    }
};

// Facts recovered from the pinned APK/native serializer. This metadata says
// nothing about acceptance by a physical FLOW 8.
struct RouteLevelPayloadSchema {
    ApkCommandId command {ApkCommandId::RouteLevel};
    model::EvidenceStatus commandEvidence {model::EvidenceStatus::VerifiedFromApk};
    model::EvidenceStatus semanticEvidence {model::EvidenceStatus::VerifiedFromApk};
    model::EvidenceStatus payloadEvidence {model::EvidenceStatus::VerifiedFromApk};
    bool sourceEndpointSemanticKnown {true};
    bool destinationEndpointSemanticKnown {true};
    bool normalizedInputDomainKnown {true};
    bool normalizedToDbBeforeSerializationObserved {true};
    bool wireFieldOrderKnown {true};
    bool wireFieldWidthsKnown {true};
    bool wireValueEncodingKnown {true};
    bool wireByteOrderKnown {true};
    bool commandFragmentationKnown {true};
};

enum class RouteLevelCodecError {
    None,
    InvalidDestinationEndpoint,
    InvalidRouteRelationship,
    InvalidNormalizedValue,
};

struct RouteLevelEncodeResult {
    std::optional<QByteArray> packet;
    RouteLevelCodecError error {RouteLevelCodecError::None};
    QString message;

    [[nodiscard]] bool ok() const noexcept { return packet.has_value(); }
};

[[nodiscard]] constexpr RouteLevelPayloadSchema routeLevelPayloadSchema() noexcept
{
    return {};
}

[[nodiscard]] std::optional<RouteLevelCodecError> validateRouteLevelCommand(
    const RouteLevelCommand& command) noexcept;

// APK x86_64 uses binary32 throughout this conversion. The PC API rejects
// non-finite/out-of-range values before narrowing double to float; that
// rejection is a PC safety rule, not an APK-behaviour claim.
[[nodiscard]] std::optional<float> routeLevelNormalizedToDb(double normalized) noexcept;

// The format-3 table is generated at compile time from the documented native
// binary32 fader curve sampled over all 256 byte codes. Encoding still uses
// the recovered table search rather than an arithmetic byte shortcut.
[[nodiscard]] const std::array<float, 256>& routeLevelFix8DbTable() noexcept;
[[nodiscard]] std::optional<quint8> encodeRouteLevelFix8(float decibels) noexcept;

// Encodes the APK-confirmed single-fragment raw packet:
// 06 01 endpointA endpointB level checksum.
[[nodiscard]] RouteLevelEncodeResult encodeRouteLevel(
    const RouteLevelCommand& command) noexcept;

} // namespace flow8::protocol
