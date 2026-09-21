#include "core/semantic_command_queue.h"

#include <cmath>
#include <utility>

namespace flow8 {

std::optional<protocol::RouteLevelCommand>
SemanticCommandQueue::Command::routeLevelCommand() const noexcept
{
    if (kind != Kind::RouteLevel) {
        return std::nullopt;
    }
    return protocol::RouteLevelCommand {
        .sourceEndpoint = sourceEndpoint,
        .destinationEndpoint = destinationEndpoint,
        .normalizedValue = normalized,
        .semanticEvidence = model::EvidenceStatus::VerifiedFromApk,
    };
}

std::optional<protocol::GainCommand>
SemanticCommandQueue::Command::gainCommand() const noexcept
{
    if (kind != Kind::Gain) {
        return std::nullopt;
    }
    return protocol::GainCommand {
        .inputEndpoint = sourceEndpoint,
        .gainDb = gainDb,
        .semanticEvidence = model::EvidenceStatus::VerifiedFromApk,
    };
}

std::optional<SemanticCommandQueue::EncodeResult>
SemanticCommandQueue::Command::encodePacket() const noexcept
{
    if (const auto routeCommand = routeLevelCommand(); routeCommand.has_value()) {
        return EncodeResult {protocol::encodeRouteLevel(*routeCommand)};
    }
    if (const auto inputGainCommand = gainCommand(); inputGainCommand.has_value()) {
        return EncodeResult {protocol::encodeGain(*inputGainCommand)};
    }
    return std::nullopt;
}

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

bool SemanticCommandQueue::enqueueGain(const int sourceIndex, const double gainDb)
{
    const auto sourceEndpoint = model::inputEndpointForIndex(sourceIndex);
    if (!sourceEndpoint.has_value()) {
        return false;
    }
    return enqueueGain(*sourceEndpoint, gainDb);
}

bool SemanticCommandQueue::enqueueGain(
    const model::EndpointId inputEndpoint, const double gainDb)
{
    if (!model::isGainCapableInputEndpoint(inputEndpoint)
        || !std::isfinite(gainDb)
        || gainDb < model::inputGainMinimumDb
        || gainDb > model::inputGainMaximumDb) {
        return false;
    }
    for (auto& command : pending_) {
        if (command.kind == Kind::Gain
            && command.sourceEndpoint == inputEndpoint) {
            command.gainDb = gainDb;
            return true;
        }
    }
    pending_.append(Command {
        .kind = Kind::Gain,
        .sourceEndpoint = inputEndpoint,
        .gainDb = gainDb,
        .action = {},
    });
    return true;
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
