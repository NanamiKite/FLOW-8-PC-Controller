#pragma once

#include "model/routing.h"
#include "protocol/route_level_codec.h"

#include <QString>
#include <QVector>

#include <optional>

namespace flow8 {

// Transport-neutral queue used before protocol encoding. Continuous route
// fader changes coalesce by (source endpoint, destination endpoint); discrete
// actions never do. A destination master is represented by equal endpoints,
// matching the APK/native semantic model without implying a BLE byte layout.
// Only one command may be in flight. The BLE backend can adopt this queue once
// the APK-derived command semantics have a verified payload layout.
class SemanticCommandQueue final {
public:
    enum class Kind { RouteLevel, Discrete };

    struct Command {
        Kind kind {Kind::Discrete};
        model::EndpointId sourceEndpoint {model::EndpointId::Input1};
        model::EndpointId destinationEndpoint {model::EndpointId::MainLr};
        double normalized {};
        QString action;

        [[nodiscard]] bool isDestinationMaster() const noexcept
        {
            return sourceEndpoint == destinationEndpoint
                && model::isDestinationEndpoint(destinationEndpoint);
        }

        // Converts only the transport-neutral route semantic into the input
        // type accepted by the protocol codec. It exposes no command byte or
        // raw payload to the caller.
        [[nodiscard]] std::optional<protocol::RouteLevelCommand>
        routeLevelCommand() const noexcept;
    };

    [[nodiscard]] bool enqueueRouteLevel(
        int sourceIndex, model::RoutingDestination destination, double normalized);
    [[nodiscard]] bool enqueueRouteLevel(
        model::EndpointId sourceEndpoint, model::EndpointId destinationEndpoint,
        double normalized);
    [[nodiscard]] bool enqueueDestinationMaster(
        model::RoutingDestination destination, double normalized);
    [[nodiscard]] bool enqueueDiscrete(QString action);
    [[nodiscard]] std::optional<Command> beginNext();
    [[nodiscard]] bool completeInFlight();
    [[nodiscard]] const std::optional<Command>& inFlight() const noexcept;
    [[nodiscard]] qsizetype pendingCount() const noexcept;
    void clearPending();

private:
    QVector<Command> pending_;
    std::optional<Command> inFlight_;
};

} // namespace flow8
