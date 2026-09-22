#pragma once

#include "model/evidence_status.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QString>
#include <QVector>

#include <optional>

namespace flow8::protocol {

enum class PacketType : quint8 {
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
    GetSetting = 0x26,
    SnapshotNames = 0x27,
    FactoryReset = 0x29,
    FxState = 0x30,
    FxPreset = 0x31,
    SnapshotRename = 0x32,
    ChannelConnectionState = 0x33,
    ChannelSimulateConnectionState = 0x34,
    HandshakeHost = 0x35,
    HandshakeReply = 0x36,
    GetMixerState = 0x37,
    MixerState = 0x38,
    HandshakeClient = 0x39,
    FxTempo = 0x40,
    SelectOutput = 0x41,
    RequestData = 0x42,
    TransferData = 0x43,
    AckData = 0x44,
    SetMidi = 0x45,
    GetMidi = 0x46,
    SetFxPresetDescription = 0x47,
    SetFxPresetDescriptionAck = 0x48,
    ChannelReserved = 0x49,
    ChannelDelay = 0x4A,
    SysExMidiDump = 0x4B,
};

struct Packet {
    quint8 type {};
    quint8 fragmentCount {};
    // APK-native framing confirms sequence ID followed by fragment index.
    std::optional<quint8> sequenceId;
    std::optional<quint8> fragmentIndex;
    // Compatibility aliases retained for existing raw inspection tools.
    std::optional<quint8> fragmentHeaderA;
    std::optional<quint8> fragmentHeaderB;
    QByteArray payload;
    QByteArray raw;
    // Evidence applies to the command-byte meaning, not to payload fields.
    model::EvidenceStatus evidence {model::EvidenceStatus::Unknown};
    model::EvidenceStatus payloadEvidence {model::EvidenceStatus::Unknown};
};

enum class PacketError {
    TooShort,
    InvalidFragmentCount,
    InvalidFragmentIndex,
    TooManyFragments,
    MissingFragmentHeader,
    ChecksumMismatch,
};

struct PacketParseResult {
    std::optional<Packet> packet;
    std::optional<PacketError> error;
    QString message;

    [[nodiscard]] bool ok() const noexcept { return packet.has_value(); }
};

[[nodiscard]] std::optional<PacketType> knownPacketType(quint8 value) noexcept;
[[nodiscard]] QString packetTypeName(quint8 value);
[[nodiscard]] model::EvidenceStatus packetEvidence(quint8 value) noexcept;
[[nodiscard]] QByteArray frameSingleFragment(quint8 type, QByteArrayView payload = {});
// Reproduces the APK common framing. At the Android MTU-255 configuration,
// maxRawPacketSize is 251: single payload capacity 248, fragmented capacity
// 246. Returns an empty vector when limits cannot be represented safely.
[[nodiscard]] QVector<QByteArray> frameCommand(
    quint8 type, QByteArrayView payload, qsizetype maxRawPacketSize = 251,
    quint8 sequenceId = 0);
[[nodiscard]] PacketParseResult parsePacket(QByteArrayView raw);

} // namespace flow8::protocol
