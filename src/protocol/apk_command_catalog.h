#pragma once

#include "model/evidence_status.h"
#include "model/routing.h"

#include <QtGlobal>

#include <optional>

namespace flow8::protocol {

// Semantic command identifiers recovered from the official FLOW Mix APK.
// They intentionally describe neither BLE framing nor payload fields.
enum class ApkCommandId : quint8 {
    Pan = 0x00,
    Solo = 0x01,
    Gain = 0x02,
    GraphicEq = 0x03,
    HighPassFilter = 0x04,
    Label = 0x05,
    RouteLevel = 0x06,
    GetSnapshotNames = 0x07,
    Mute = 0x08,
    ParametricEq = 0x09,
    Phase = 0x10,
    FxSetup = 0x11,
    SnapshotDelete = 0x12,
    Compressor = 0x13,
    Limiter = 0x14,
    Phantom = 0x15,
    GetChannelState = 0x16,
    InputState = 0x17,
    OutputState = 0x18,
    SnapshotSave = 0x19,
    SnapshotLoad = 0x20,
    MeterRequest = 0x21,
    MeterUpdate = 0x22,
    GetChannelLabels = 0x23,
    ChannelLabels = 0x24,
    Setting = 0x25,
    SnapshotNames = 0x27,
    FactoryReset = 0x29,
    FxState = 0x30,
    FxPreset = 0x31,
    SnapshotRename = 0x32,
    ConnectionState = 0x33,
    GetMixerState = 0x37,
    MixerState = 0x38,
    FxTempo = 0x40,
    SelectOutput = 0x41,
    ChannelDelay = 0x4a,
};

using ApkEndpointId = model::EndpointId;
using ApkRouteSourceId = ApkEndpointId;
using ApkRouteDestinationId = ApkEndpointId;

struct ApkCommandDescriptor {
    ApkCommandId id;
    const char* semanticName;
    model::EvidenceStatus evidence {model::EvidenceStatus::VerifiedFromApk};
    // Native packet construction confirms that the command ID is the first
    // byte of the raw packet envelope. This does not reveal payload fields.
    bool commandByteConfirmed {true};
    // Always false until a complete payload is documented independently.
    bool payloadLayoutKnown {};
    model::EvidenceStatus payloadEvidence {model::EvidenceStatus::Unknown};
};

[[nodiscard]] std::optional<ApkCommandDescriptor> apkCommandDescriptor(quint8 id) noexcept;
[[nodiscard]] std::optional<ApkRouteSourceId> apkRouteSourceId(int sourceIndex) noexcept;
[[nodiscard]] std::optional<ApkRouteDestinationId> apkRouteDestinationId(
    model::RoutingDestination destination) noexcept;
[[nodiscard]] std::optional<ApkEndpointId> apkEndpointId(quint8 value) noexcept;

} // namespace flow8::protocol
