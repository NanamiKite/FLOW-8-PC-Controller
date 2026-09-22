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

// APK channel capability bit 0x04 is present for conventional analog inputs
// 0..5 and absent for the BT/USB strip (6). This is a product/semantic guard;
// the native wire serializer itself can mechanically emit endpoint 6.
[[nodiscard]] constexpr bool isGainCapableInputEndpoint(
    const EndpointId endpoint) noexcept
{
    switch (endpoint) {
    case EndpointId::Input1:
    case EndpointId::Input2:
    case EndpointId::Input3:
    case EndpointId::Input4:
    case EndpointId::Input56:
    case EndpointId::Input78:
        return true;
    case EndpointId::BluetoothUsb:
    case EndpointId::Monitor1:
    case EndpointId::Monitor2:
    case EndpointId::Fx1:
    case EndpointId::Fx2:
    case EndpointId::MainLr:
        return false;
    }
    return false;
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
