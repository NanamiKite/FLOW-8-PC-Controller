#include "protocol/apk_command_catalog.h"

#include <array>

namespace flow8::protocol {
namespace {

constexpr std::array descriptors {
    ApkCommandDescriptor {ApkCommandId::Pan, "Pan"},
    ApkCommandDescriptor {ApkCommandId::Solo, "Solo"},
    ApkCommandDescriptor {
        ApkCommandId::Gain,
        "Gain",
        model::EvidenceStatus::VerifiedFromApk,
        true,
        true,
        model::EvidenceStatus::VerifiedFromApk,
    },
    ApkCommandDescriptor {ApkCommandId::GraphicEq, "Graphic EQ"},
    ApkCommandDescriptor {ApkCommandId::HighPassFilter, "HPF"},
    ApkCommandDescriptor {ApkCommandId::Label, "Label"},
    ApkCommandDescriptor {
        ApkCommandId::RouteLevel,
        "Master/route level",
        model::EvidenceStatus::VerifiedFromApk,
        true,
        true,
        model::EvidenceStatus::VerifiedFromApk,
    },
    ApkCommandDescriptor {ApkCommandId::GetSnapshotNames, "Get snapshot names"},
    ApkCommandDescriptor {ApkCommandId::Mute, "Mute"},
    ApkCommandDescriptor {ApkCommandId::ParametricEq, "PEQ"},
    ApkCommandDescriptor {ApkCommandId::Phase, "Phase"},
    ApkCommandDescriptor {ApkCommandId::FxSetup, "FX setup"},
    ApkCommandDescriptor {ApkCommandId::SnapshotDelete, "Snapshot delete"},
    ApkCommandDescriptor {ApkCommandId::Compressor, "Compressor"},
    ApkCommandDescriptor {ApkCommandId::Limiter, "Limiter"},
    ApkCommandDescriptor {ApkCommandId::Phantom, "Phantom"},
    ApkCommandDescriptor {ApkCommandId::GetChannelState, "Get channel state"},
    ApkCommandDescriptor {ApkCommandId::InputState, "Input state"},
    ApkCommandDescriptor {ApkCommandId::OutputState, "Output state"},
    ApkCommandDescriptor {ApkCommandId::SnapshotSave, "Snapshot save"},
    ApkCommandDescriptor {ApkCommandId::SnapshotLoad, "Snapshot load"},
    ApkCommandDescriptor {ApkCommandId::MeterRequest, "Meter request"},
    ApkCommandDescriptor {ApkCommandId::MeterUpdate, "Meter update"},
    ApkCommandDescriptor {ApkCommandId::GetChannelLabels, "Get channel labels"},
    ApkCommandDescriptor {ApkCommandId::ChannelLabels, "Channel labels"},
    ApkCommandDescriptor {ApkCommandId::Setting, "Setting"},
    ApkCommandDescriptor {ApkCommandId::SnapshotNames, "Snapshot names"},
    ApkCommandDescriptor {ApkCommandId::FactoryReset, "Factory reset"},
    ApkCommandDescriptor {ApkCommandId::FxState, "FX state"},
    ApkCommandDescriptor {ApkCommandId::FxPreset, "FX preset"},
    ApkCommandDescriptor {ApkCommandId::SnapshotRename, "Snapshot rename/update"},
    ApkCommandDescriptor {ApkCommandId::ConnectionState, "Connection state"},
    ApkCommandDescriptor {ApkCommandId::GetMixerState, "Get mixer state"},
    ApkCommandDescriptor {ApkCommandId::MixerState, "Mixer state"},
    ApkCommandDescriptor {ApkCommandId::FxTempo, "FX tempo"},
    ApkCommandDescriptor {ApkCommandId::SelectOutput, "Select output"},
    ApkCommandDescriptor {ApkCommandId::ChannelDelay, "Channel delay"},
};

} // namespace

std::optional<ApkCommandDescriptor> apkCommandDescriptor(const quint8 id) noexcept
{
    for (const auto& descriptor : descriptors) {
        if (static_cast<quint8>(descriptor.id) == id) {
            return descriptor;
        }
    }
    return std::nullopt;
}

std::optional<ApkRouteSourceId> apkRouteSourceId(const int sourceIndex) noexcept
{
    return model::inputEndpointForIndex(sourceIndex);
}

std::optional<ApkRouteDestinationId> apkRouteDestinationId(
    const model::RoutingDestination destination) noexcept
{
    return model::endpointForDestination(destination);
}

std::optional<ApkEndpointId> apkEndpointId(const quint8 value) noexcept
{
    const auto endpoint = static_cast<ApkEndpointId>(value);
    if (!model::isInputEndpoint(endpoint) && !model::isDestinationEndpoint(endpoint)) {
        return std::nullopt;
    }
    return endpoint;
}

} // namespace flow8::protocol
