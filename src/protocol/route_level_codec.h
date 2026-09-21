#pragma once

#include "model/evidence_status.h"
#include "model/endpoint.h"
#include "protocol/apk_command_catalog.h"

#include <QByteArray>
#include <QString>

#include <optional>

namespace flow8::protocol {

// Transport-neutral semantic input to the 0x06 codec boundary. Endpoint
// values are the APK-confirmed semantic IDs; this structure is not a BLE
// payload definition.
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

// Facts known about command 0x06 after the APK/native handoff. The internal
// xairbt_cmd fields are known semantically, but none of the false wire fields
// may be promoted without the descriptor mapping or a verified capture.
struct RouteLevelPayloadSchema {
    ApkCommandId command {ApkCommandId::RouteLevel};
    model::EvidenceStatus commandEvidence {model::EvidenceStatus::VerifiedFromApk};
    model::EvidenceStatus semanticEvidence {model::EvidenceStatus::VerifiedFromApk};
    model::EvidenceStatus payloadEvidence {model::EvidenceStatus::Unknown};
    bool sourceEndpointSemanticKnown {true};
    bool destinationEndpointSemanticKnown {true};
    bool normalizedInputDomainKnown {true};
    bool normalizedToDbBeforeSerializationObserved {true};
    bool wireFieldOrderKnown {};
    bool wireFieldWidthsKnown {};
    bool wireValueEncodingKnown {};
    bool wireByteOrderKnown {};
    bool commandFragmentationKnown {};
};

enum class RouteLevelCodecError {
    None,
    InvalidDestinationEndpoint,
    InvalidRouteRelationship,
    InvalidNormalizedValue,
    UnknownPayloadLayout,
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

// A valid semantic command currently returns UnknownPayloadLayout. In
// particular, this function never calls frameSingleFragment with an empty or
// guessed payload and never reuses the legacy reference 0x06 layout.
[[nodiscard]] RouteLevelEncodeResult encodeRouteLevel(
    const RouteLevelCommand& command) noexcept;

} // namespace flow8::protocol
