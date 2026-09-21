#pragma once

#include <QtGlobal>

#include <optional>

namespace flow8::model {

// Channel/route endpoint identifiers recovered from the official FLOW Mix
// APK/native library. They describe APK semantics only; they are not a claim
// about a complete hardware endpoint catalogue or a final BLE payload field.
enum class EndpointId : quint8 {
    Input1 = 0,
    Input2 = 1,
    Input3 = 2,
    Input4 = 3,
    Input56 = 4,
    Input78 = 5,
    BluetoothUsb = 6,
    Monitor1 = 10,
    Monitor2 = 11,
    Fx1 = 12,
    Fx2 = 13,
    MainLr = 15,
};

[[nodiscard]] constexpr bool isInputEndpoint(const EndpointId endpoint) noexcept
{
    return static_cast<quint8>(endpoint) <= static_cast<quint8>(EndpointId::BluetoothUsb);
}

[[nodiscard]] constexpr bool isDestinationEndpoint(const EndpointId endpoint) noexcept
{
    switch (endpoint) {
    case EndpointId::Monitor1:
    case EndpointId::Monitor2:
    case EndpointId::Fx1:
    case EndpointId::Fx2:
    case EndpointId::MainLr:
        return true;
    default:
        return false;
    }
}

[[nodiscard]] constexpr std::optional<EndpointId> inputEndpointForIndex(
    const int inputIndex) noexcept
{
    if (inputIndex < 0 || inputIndex > 6) {
        return std::nullopt;
    }
    return static_cast<EndpointId>(inputIndex);
}

[[nodiscard]] constexpr std::optional<int> inputIndexForEndpoint(
    const EndpointId endpoint) noexcept
{
    if (!isInputEndpoint(endpoint)) {
        return std::nullopt;
    }
    return static_cast<int>(endpoint);
}

} // namespace flow8::model
