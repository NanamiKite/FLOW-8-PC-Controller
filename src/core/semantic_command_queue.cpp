#include "core/semantic_command_queue.h"

#include <cmath>
#include <utility>

namespace flow8 {

bool SemanticCommandQueue::enqueueRouteLevel(
    const int sourceIndex, const model::RoutingDestination destination,
    const double normalized)
{
    const auto sourceEndpoint = model::inputEndpointForIndex(sourceIndex);
    if (!sourceEndpoint.has_value()) {
        return false;
    }
    return enqueueRouteLevel(
        *sourceEndpoint, model::endpointForDestination(destination), normalized);
}

bool SemanticCommandQueue::enqueueRouteLevel(
    const model::EndpointId sourceEndpoint,
    const model::EndpointId destinationEndpoint,
    const double normalized)
{
    const bool inputRoute = model::isInputEndpoint(sourceEndpoint)
        && model::isDestinationEndpoint(destinationEndpoint);
    const bool destinationMaster = sourceEndpoint == destinationEndpoint
        && model::isDestinationEndpoint(destinationEndpoint);
    if ((!inputRoute && !destinationMaster) || !std::isfinite(normalized)
        || normalized < 0.0 || normalized > 1.0) {
        return false;
    }
    for (auto& command : pending_) {
        if (command.kind == Kind::RouteLevel
            && command.sourceEndpoint == sourceEndpoint
            && command.destinationEndpoint == destinationEndpoint) {
            command.normalized = normalized;
            return true;
        }
    }
    pending_.append(Command {
        .kind = Kind::RouteLevel,
        .sourceEndpoint = sourceEndpoint,
        .destinationEndpoint = destinationEndpoint,
        .normalized = normalized,
        .action = {},
    });
    return true;
}

bool SemanticCommandQueue::enqueueDestinationMaster(
    const model::RoutingDestination destination, const double normalized)
{
    const auto endpoint = model::endpointForDestination(destination);
    return enqueueRouteLevel(endpoint, endpoint, normalized);
}

bool SemanticCommandQueue::enqueueDiscrete(QString action)
{
    action = action.trimmed();
    if (action.isEmpty()) {
        return false;
    }
    pending_.append(Command {
        .kind = Kind::Discrete,
        .action = std::move(action),
    });
    return true;
}

std::optional<SemanticCommandQueue::Command> SemanticCommandQueue::beginNext()
{
    if (inFlight_.has_value() || pending_.isEmpty()) {
        return std::nullopt;
    }
    inFlight_ = pending_.takeFirst();
    return inFlight_;
}

bool SemanticCommandQueue::completeInFlight()
{
    if (!inFlight_.has_value()) {
        return false;
    }
    inFlight_.reset();
    return true;
}

const std::optional<SemanticCommandQueue::Command>&
SemanticCommandQueue::inFlight() const noexcept
{
    return inFlight_;
}

qsizetype SemanticCommandQueue::pendingCount() const noexcept
{
    return pending_.size();
}

void SemanticCommandQueue::clearPending()
{
    pending_.clear();
}

} // namespace flow8
