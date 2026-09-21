#pragma once

#include "model/channel.h"
#include "model/routing.h"
#include "protocol/gain_codec.h"
#include "protocol/route_level_codec.h"

#include <QString>
#include <QVector>

#include <optional>
#include <variant>

namespace flow8 {

// Transport-neutral queue used before protocol encoding. Continuous values
// coalesce only when kind and semantic target match; discrete actions never
// do. Only one command may be in flight. The queue exposes transport-ready
// bytes only after successful command-specific codec validation; real-device
// acceptance remains a separate hardware concern.
class SemanticCommandQueue final {
public:
    enum class Kind { RouteLevel, Gain, Discrete };

    using EncodeResult = std::variant<
        protocol::RouteLevelEncodeResult,
        protocol::GainEncodeResult>;

    struct Command {
        Kind kind {Kind::Discrete};
        model::EndpointId sourceEndpoint {model::EndpointId::Input1};
        model::EndpointId destinationEndpoint {model::EndpointId::MainLr};
        double normalized {};
        double gainDb {};
        QString action;

        [[nodiscard]] bool isDestinationMaster() const noexcept
        {
            return sourceEndpoint == destinationEndpoint
                && model::isDestinationEndpoint(destinationEndpoint);
        }

        // Convert transport-neutral semantics into command-specific codec
        // inputs. Neither accessor exposes command bytes or raw payloads.
        [[nodiscard]] std::optional<protocol::RouteLevelCommand>
        routeLevelCommand() const noexcept;
        [[nodiscard]] std::optional<protocol::GainCommand>
        gainCommand() const noexcept;

        // Typed queue-to-codec handoff. Discrete operations do not yet have a
        // supported codec and return nullopt; callers may write only a
        // successful result's packet to a transport.
        [[nodiscard]] std::optional<EncodeResult>
        encodePacket() const noexcept;
    };

    [[nodiscard]] bool enqueueRouteLevel(
        int sourceIndex, model::RoutingDestination destination, double normalized);
    [[nodiscard]] bool enqueueRouteLevel(
        model::EndpointId sourceEndpoint, model::EndpointId destinationEndpoint,
        double normalized);
    [[nodiscard]] bool enqueueDestinationMaster(
        model::RoutingDestination destination, double normalized);
    [[nodiscard]] bool enqueueGain(int sourceIndex, double gainDb);
    [[nodiscard]] bool enqueueGain(model::EndpointId inputEndpoint, double gainDb);
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
