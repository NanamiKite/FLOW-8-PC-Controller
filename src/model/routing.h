#pragma once

#include "model/state_value.h"

#include <QVector>

namespace flow8::model {

enum class RoutingDestination {
    Main,
    Monitor1,
    Monitor2,
    Fx1,
    Fx2,
};

enum class UsbMode {
    Streaming,
    Recording,
};

enum class UsbRouteDestination {
    Input1,
    Input2,
    Input3,
    Input4,
    Input56,
    Input78,
    UsbBluetooth,
    Monitor1,
    Monitor2,
};

enum class HeadphoneSource {
    Main,
    Monitor1,
    Monitor2,
};

// FX input sends live on ChannelState. These destinations describe only the
// return of an already-processed FX engine and must not be conflated with the
// per-input FX send levels.
enum class FxOutputDestination {
    Main,
    Monitor1,
    Monitor2,
};

struct RouteState {
    int inputIndex {};
    RoutingDestination destination {RoutingDestination::Main};
    StateValue<bool> enabled;
};

struct UsbRouteState {
    UsbRouteDestination destination {UsbRouteDestination::Input1};
    StateValue<bool> enabled;
};

struct FxOutputRouteState {
    int effectIndex {};
    FxOutputDestination destination {FxOutputDestination::Main};
    StateValue<bool> enabled;
};

struct MonitorLinkState {
    StateValue<bool> stereoLinked;
};

struct RoutingState {
    QVector<RouteState> routes;
    StateValue<UsbMode> usbMode;
    QVector<UsbRouteState> usbRoutes;
    QVector<FxOutputRouteState> fxOutputRoutes;
    StateValue<HeadphoneSource> headphoneSource;
    MonitorLinkState monitorLink;

    [[nodiscard]] const RouteState* route(int inputIndex,
                                          RoutingDestination destination) const noexcept
    {
        for (const auto& candidate : routes) {
            if (candidate.inputIndex == inputIndex && candidate.destination == destination) {
                return &candidate;
            }
        }
        return nullptr;
    }

    [[nodiscard]] const UsbRouteState* usbRoute(
        UsbRouteDestination destination) const noexcept
    {
        for (const auto& candidate : usbRoutes) {
            if (candidate.destination == destination) {
                return &candidate;
            }
        }
        return nullptr;
    }

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

} // namespace flow8::model
