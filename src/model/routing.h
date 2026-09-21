#pragma once

#include "model/endpoint.h"
#include "model/signal_path.h"
#include "model/state_value.h"
#include "model/usb_audio.h"

#include <array>
#include <QVector>

#include <optional>

namespace flow8::model {

enum class RoutingDestination {
    Main,
    Monitor1,
    Monitor2,
    Fx1,
    Fx2,
};

// FX input sends live on ChannelState. These destinations describe only the
// return of an already-processed FX engine and must not be conflated with the
// per-input FX send levels.
enum class FxOutputDestination {
    Main,
    Monitor1,
    Monitor2,
};

struct RouteLevelState {
    EndpointId sourceEndpoint {EndpointId::Input1};
    EndpointId destinationEndpoint {EndpointId::MainLr};
    StateValue<double> confirmed;
    std::optional<double> pending;
    QString error;

    [[nodiscard]] double effectiveValue(const double fallback = 0.0) const noexcept
    {
        return pending.value_or(confirmed.value.value_or(fallback));
    }
};

[[nodiscard]] constexpr EndpointId endpointForDestination(
    RoutingDestination destination) noexcept;

struct RouteLevelMatrix {
    QVector<RouteLevelState> cells;

    [[nodiscard]] const RouteLevelState* level(
        int sourceIndex, RoutingDestination destination) const noexcept
    {
        const auto sourceEndpoint = inputEndpointForIndex(sourceIndex);
        if (!sourceEndpoint.has_value()) {
            return nullptr;
        }
        const auto destinationEndpoint = endpointForDestination(destination);
        for (const auto& candidate : cells) {
            if (candidate.sourceEndpoint == *sourceEndpoint
                && candidate.destinationEndpoint == destinationEndpoint) {
                return &candidate;
            }
        }
        return nullptr;
    }
};

[[nodiscard]] constexpr EndpointId endpointForDestination(
    const RoutingDestination destination) noexcept
{
    switch (destination) {
    case RoutingDestination::Main: return EndpointId::MainLr;
    case RoutingDestination::Monitor1: return EndpointId::Monitor1;
    case RoutingDestination::Monitor2: return EndpointId::Monitor2;
    case RoutingDestination::Fx1: return EndpointId::Fx1;
    case RoutingDestination::Fx2: return EndpointId::Fx2;
    }
    return EndpointId::MainLr;
}

[[nodiscard]] constexpr std::optional<RoutingDestination> destinationForEndpoint(
    const EndpointId endpoint) noexcept
{
    switch (endpoint) {
    case EndpointId::MainLr: return RoutingDestination::Main;
    case EndpointId::Monitor1: return RoutingDestination::Monitor1;
    case EndpointId::Monitor2: return RoutingDestination::Monitor2;
    case EndpointId::Fx1: return RoutingDestination::Fx1;
    case EndpointId::Fx2: return RoutingDestination::Fx2;
    default: return std::nullopt;
    }
}

struct HeadphoneRoutingState {
    StateValue<HeadphoneSource> source;
    StateValue<RoutingTapPoint> tapPoint;
    StateValue<bool> bluetoothUsbPhonesOnly;
};

struct FxOutputRouteState {
    int effectIndex {};
    FxOutputDestination destination {FxOutputDestination::Main};
    StateValue<bool> enabled;
};

struct RoutingState {
    RouteLevelMatrix routeLevels;
    UsbAudioRoutingState usbAudio;
    HeadphoneRoutingState headphones;
    QVector<FxOutputRouteState> fxOutputRoutes;

    [[nodiscard]] const FxOutputRouteState* fxOutputRoute(
        int effectIndex, FxOutputDestination destination) const noexcept
    {
        for (const auto& candidate : fxOutputRoutes) {
            if (candidate.effectIndex == effectIndex
                && candidate.destination == destination) {
                return &candidate;
            }
        }
        return nullptr;
    }
};

[[nodiscard]] constexpr int busIndexForDestination(
    const RoutingDestination destination) noexcept
{
    switch (destination) {
    case RoutingDestination::Main: return 0;
    case RoutingDestination::Monitor1: return 1;
    case RoutingDestination::Monitor2: return 2;
    case RoutingDestination::Fx1:
    case RoutingDestination::Fx2:
        return -1;
    }
    return -1;
}

} // namespace flow8::model
