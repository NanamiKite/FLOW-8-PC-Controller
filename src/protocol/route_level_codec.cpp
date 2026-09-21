#include "protocol/route_level_codec.h"

#include "protocol/packet.h"

#include <array>
#include <cmath>
#include <utility>

namespace flow8::protocol {
namespace {

// Mathematical structure recovered from linearScaleValueTodBValue(), with
// every operation intentionally kept in binary32 like the APK x86_64 code.
constexpr float normalizedToDbBinary32(const float normalized) noexcept
{
    if (normalized >= 1.0F) {
        return 10.0F;
    }
    if (normalized >= 0.5F) {
        return 40.0F * normalized - 30.0F;
    }
    if (normalized >= 0.25F) {
        return 80.0F * normalized - 50.0F;
    }
    if (normalized >= 0.0625F) {
        return 160.0F * normalized - 70.0F;
    }
    if (normalized >= (1.0F / 1024.0F)) {
        return 480.0F * normalized - 90.0F;
    }
    return -144.0F;
}

constexpr std::array<float, 256> makeRouteLevelFix8DbTable() noexcept
{
    std::array<float, 256> result {};
    for (std::size_t index = 0; index < result.size(); ++index) {
        const float normalized = static_cast<float>(index) / 255.0F;
        result[index] = normalizedToDbBinary32(normalized);
    }
    // The embedded APK table explicitly stores unity gain at code 191. This
    // authoritative anchor is why the encoder must search the table instead
    // of reducing the operation to floor(normalized * 255).
    result[191] = 0.0F;
    return result;
}

constexpr auto routeLevelDbTable = makeRouteLevelFix8DbTable();

static_assert(routeLevelDbTable.size() == 256);
static_assert(routeLevelDbTable.front() == -144.0F);
static_assert(routeLevelDbTable[191] == 0.0F);
static_assert(routeLevelDbTable.back() == 10.0F);

} // namespace

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
            break;
        }
        return {
            .packet = std::nullopt,
            .error = *validationError,
            .message = std::move(message),
        };
    }

    const auto decibels = routeLevelNormalizedToDb(command.normalizedValue);
    const auto wireLevel = decibels.has_value()
        ? encodeRouteLevelFix8(*decibels) : std::nullopt;
    if (!wireLevel.has_value()) {
        return {
            .packet = std::nullopt,
            .error = RouteLevelCodecError::InvalidNormalizedValue,
            .message = QStringLiteral("0x06 level conversion rejected a non-finite value"),
        };
    }

    const char payload[] = {
        static_cast<char>(command.sourceEndpoint),
        static_cast<char>(command.destinationEndpoint),
        static_cast<char>(*wireLevel),
    };
    return {
        .packet = frameSingleFragment(
            static_cast<quint8>(ApkCommandId::RouteLevel),
            QByteArrayView(payload, 3)),
        .error = RouteLevelCodecError::None,
        .message = {},
    };
}

std::optional<float> routeLevelNormalizedToDb(const double normalized) noexcept
{
    if (!std::isfinite(normalized) || normalized < 0.0 || normalized > 1.0) {
        return std::nullopt;
    }
    return normalizedToDbBinary32(static_cast<float>(normalized));
}

const std::array<float, 256>& routeLevelFix8DbTable() noexcept
{
    return routeLevelDbTable;
}

std::optional<quint8> encodeRouteLevelFix8(const float decibels) noexcept
{
    if (!std::isfinite(decibels)) {
        return std::nullopt;
    }
    for (std::size_t index = 1; index < routeLevelDbTable.size(); ++index) {
        if (routeLevelDbTable[index] > decibels) {
            return static_cast<quint8>(index - 1);
        }
    }
    return static_cast<quint8>(routeLevelDbTable.size() - 1);
}

} // namespace flow8::protocol
