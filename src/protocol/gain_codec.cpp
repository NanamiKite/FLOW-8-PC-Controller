#include "protocol/gain_codec.h"

#include "protocol/packet.h"

#include <algorithm>
#include <cmath>
#include <utility>

namespace flow8::protocol {

std::optional<GainCodecError> validateGainCommand(
    const GainCommand& command) noexcept
{
    if (!model::isInputEndpoint(command.inputEndpoint)) {
        return GainCodecError::InvalidInputEndpoint;
    }
    if (!std::isfinite(command.gainDb)) {
        return GainCodecError::InvalidGainValue;
    }
    return std::nullopt;
}

std::optional<quint8> encodeGainFix8Format5(const double gainDb) noexcept
{
    if (!std::isfinite(gainDb)) {
        return std::nullopt;
    }

    // JNI receives a Java float, and the native helper performs the clamp,
    // addition and multiplication in binary32 before widening for lrint().
    const float nativeGainDb = static_cast<float>(gainDb);
    const float clampedDb = std::clamp(nativeGainDb, -60.0F, 60.0F);
    const float scaled = (clampedDb + 60.0F) * 2.0F;
    const long rounded = std::lrint(static_cast<double>(scaled));
    return static_cast<quint8>(rounded);
}

GainEncodeResult encodeGain(const GainCommand& command) noexcept
{
    if (const auto validationError = validateGainCommand(command);
        validationError.has_value()) {
        QString message;
        switch (*validationError) {
        case GainCodecError::InvalidInputEndpoint:
            message = QStringLiteral("0x02 requires an APK input endpoint");
            break;
        case GainCodecError::InvalidGainValue:
            message = QStringLiteral("0x02 gain must be finite");
            break;
        case GainCodecError::None:
            break;
        }
        return {
            .packet = std::nullopt,
            .error = *validationError,
            .message = std::move(message),
        };
    }

    const auto wireGain = encodeGainFix8Format5(command.gainDb);
    if (!wireGain.has_value()) {
        return {
            .packet = std::nullopt,
            .error = GainCodecError::InvalidGainValue,
            .message = QStringLiteral("0x02 FIX8 format-5 conversion rejected the value"),
        };
    }

    const char payload[] = {
        static_cast<char>(command.inputEndpoint),
        static_cast<char>(*wireGain),
    };
    return {
        .packet = frameSingleFragment(
            static_cast<quint8>(ApkCommandId::Gain),
            QByteArrayView(payload, 2)),
        .error = GainCodecError::None,
        .message = {},
    };
}

} // namespace flow8::protocol
