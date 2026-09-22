#include "protocol/apk_command_catalog.h"

#include <array>

namespace flow8::protocol {
namespace {

constexpr ApkCommandDescriptor known(
    const ApkCommandId id, const char* semanticName) noexcept
{
    return {
        id,
        semanticName,
        model::EvidenceStatus::VerifiedFromApk,
        true,
        true,
        model::EvidenceStatus::VerifiedFromApk,
    };
}

constexpr std::array descriptors {
    known(ApkCommandId::Pan, "Pan"),
    known(ApkCommandId::Solo, "Solo"),
    known(ApkCommandId::Gain, "Gain"),
    known(ApkCommandId::GraphicEq, "Output GEQ"),
    known(ApkCommandId::HighPassFilter, "HPF"),
    known(ApkCommandId::Label, "Label"),
    known(ApkCommandId::RouteLevel, "Master/route level"),
    known(ApkCommandId::GetSnapshotNames, "Get snapshot names"),
    known(ApkCommandId::Mute, "Mute"),
    known(ApkCommandId::ParametricEq, "Input PEQ"),
    known(ApkCommandId::Phase, "Phase"),
    known(ApkCommandId::FxSetup, "FX setup"),
    known(ApkCommandId::SnapshotDelete, "Snapshot delete"),
    known(ApkCommandId::Compressor, "Squeezer/compressor amount"),
    known(ApkCommandId::Limiter, "Limiter"),
    known(ApkCommandId::Phantom, "Phantom"),
    known(ApkCommandId::GetChannelState, "Get channel state"),
    known(ApkCommandId::InputState, "Input state"),
    known(ApkCommandId::OutputState, "Output state"),
    known(ApkCommandId::SnapshotSave, "Snapshot save"),
    known(ApkCommandId::SnapshotLoad, "Snapshot load"),
    known(ApkCommandId::MeterRequest, "Meter request"),
    known(ApkCommandId::MeterUpdate, "Meter update"),
    known(ApkCommandId::GetChannelLabels, "Get channel labels"),
    known(ApkCommandId::ChannelLabels, "Channel labels"),
    known(ApkCommandId::Setting, "Setting"),
    known(ApkCommandId::GetSetting, "Get setting"),
    known(ApkCommandId::SnapshotNames, "Snapshot names"),
    known(ApkCommandId::FactoryReset, "Factory reset"),
    known(ApkCommandId::FxState, "FX state"),
    known(ApkCommandId::FxPreset, "FX preset"),
    known(ApkCommandId::SnapshotRename, "Snapshot rename/update"),
    known(ApkCommandId::ChannelConnectionState, "Physical channel connection state"),
    known(ApkCommandId::ChannelSimulateConnectionState, "Simulate channel connection state"),
    known(ApkCommandId::HandshakeHost, "Handshake host"),
    known(ApkCommandId::HandshakeReply, "Handshake reply"),
    known(ApkCommandId::GetMixerState, "Get mixer state"),
    known(ApkCommandId::MixerState, "Mixer state"),
    known(ApkCommandId::HandshakeClient, "Handshake client"),
    known(ApkCommandId::FxTempo, "Global FX tempo"),
    known(ApkCommandId::SelectOutput, "Device linked output selection"),
    known(ApkCommandId::RequestData, "Request data"),
    known(ApkCommandId::TransferData, "Transfer data"),
    known(ApkCommandId::AckData, "Acknowledge data"),
    known(ApkCommandId::SetMidi, "Set MIDI"),
    known(ApkCommandId::GetMidi, "Get MIDI"),
    known(ApkCommandId::SetFxPresetDescription, "Set FX preset description"),
    known(ApkCommandId::SetFxPresetDescriptionAck, "Set FX preset description acknowledge"),
    known(ApkCommandId::ChannelReserved, "Channel reserved"),
    known(ApkCommandId::ChannelDelay, "Channel delay"),
    known(ApkCommandId::SysExMidiDump, "SysEx MIDI dump"),
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
