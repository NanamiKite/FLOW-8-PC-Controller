#include "protocol/route_level_codec.h"

#include <cmath>
#include <utility>

namespace flow8::protocol {

std::optional<RouteLevelCodecError> validateRouteLevelCommand(
    const RouteLevelCommand& command) noexcept
{
    if (!model::isDestinationEndpoint(command.destinationEndpoint)) {
        return RouteLevelCodecError::InvalidDestinationEndpoint;
    }

    const bool inputRoute = model::isInputEndpoint(command.sourceEndpoint);
    const bool destinationMaster = command.isDestinationMaster();
    if (!inputRoute && !destinationMaster) {
        return RouteLevelCodecError::InvalidRouteRelationship;
    }

    if (!std::isfinite(command.normalizedValue)
        || command.normalizedValue < 0.0 || command.normalizedValue > 1.0) {
        return RouteLevelCodecError::InvalidNormalizedValue;
    }
    return std::nullopt;
}

RouteLevelEncodeResult encodeRouteLevel(const RouteLevelCommand& command) noexcept
{
    if (const auto validationError = validateRouteLevelCommand(command);
        validationError.has_value()) {
        QString message;
        switch (*validationError) {
        case RouteLevelCodecError::InvalidDestinationEndpoint:
            message = QStringLiteral("0x06 destination is not an APK-confirmed destination endpoint");
            break;
        case RouteLevelCodecError::InvalidRouteRelationship:
            message = QStringLiteral("0x06 requires an input-to-destination route or destination self-route");
            break;
        case RouteLevelCodecError::InvalidNormalizedValue:
            message = QStringLiteral("0x06 semantic normalized value must be finite and within 0..1");
            break;
        case RouteLevelCodecError::None:
        case RouteLevelCodecError::UnknownPayloadLayout:
            break;
        }
        return {
            .packet = std::nullopt,
            .error = *validationError,
            .message = std::move(message),
        };
    }

    return {
        .packet = std::nullopt,
        .error = RouteLevelCodecError::UnknownPayloadLayout,
        .message = QStringLiteral(
            "0x06 semantics are VERIFIED_FROM_APK, but final BLE payload layout is UNKNOWN"),
    };
}

} // namespace flow8::protocol
