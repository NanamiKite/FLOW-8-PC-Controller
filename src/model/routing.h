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

struct RouteState {
    int inputIndex {};
    RoutingDestination destination {RoutingDestination::Main};
    StateValue<bool> enabled;
};

struct UsbRouteState {
    UsbRouteDestination destination {UsbRouteDestination::Input1};
    StateValue<bool> enabled;
};

struct FxMonitorRouteState {
    int effectIndex {};
    int monitorIndex {};
    StateValue<bool> enabled;
};

struct RoutingState {
    QVector<RouteState> routes;
    StateValue<UsbMode> usbMode;
    QVector<UsbRouteState> usbRoutes;
    QVector<FxMonitorRouteState> fxMonitorRoutes;
    StateValue<HeadphoneSource> headphoneSource;
    StateValue<bool> monitorStereoLink;

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

    [[nodiscard]] const FxMonitorRouteState* fxMonitorRoute(
        int effectIndex, int monitorIndex) const noexcept
    {
        for (const auto& candidate : fxMonitorRoutes) {
            if (candidate.effectIndex == effectIndex
                && candidate.monitorIndex == monitorIndex) {
                return &candidate;
            }
        }
        return nullptr;
    }
};

} // namespace flow8::model
