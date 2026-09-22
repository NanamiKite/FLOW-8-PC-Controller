#include "protocol/packet.h"

#include "protocol/codec.h"
#include "protocol/apk_command_catalog.h"

namespace flow8::protocol {

std::optional<PacketType> knownPacketType(const quint8 value) noexcept
{
    switch (value) {
    case 0x00: return PacketType::Pan;
    case 0x01: return PacketType::Solo;
    case 0x02: return PacketType::Gain;
    case 0x03: return PacketType::GraphicEq;
    case 0x04: return PacketType::HighPassFilter;
    case 0x05: return PacketType::Label;
    case 0x06: return PacketType::RouteLevel;
    case 0x07: return PacketType::GetSnapshotNames;
    case 0x08: return PacketType::Mute;
    case 0x09: return PacketType::ParametricEq;
    case 0x10: return PacketType::Phase;
    case 0x11: return PacketType::FxSetup;
    case 0x12: return PacketType::SnapshotDelete;
    case 0x13: return PacketType::Compressor;
    case 0x14: return PacketType::Limiter;
    case 0x15: return PacketType::Phantom;
    case 0x16: return PacketType::GetChannelState;
    case 0x17: return PacketType::InputState;
    case 0x18: return PacketType::OutputState;
    case 0x19: return PacketType::SnapshotSave;
    case 0x20: return PacketType::SnapshotLoad;
    case 0x21: return PacketType::MeterRequest;
    case 0x22: return PacketType::MeterUpdate;
    case 0x23: return PacketType::GetChannelLabels;
    case 0x24: return PacketType::ChannelLabels;
    case 0x25: return PacketType::Setting;
    case 0x26: return PacketType::GetSetting;
    case 0x27: return PacketType::SnapshotNames;
    case 0x29: return PacketType::FactoryReset;
    case 0x30: return PacketType::FxState;
    case 0x31: return PacketType::FxPreset;
    case 0x32: return PacketType::SnapshotRename;
    case 0x33: return PacketType::ChannelConnectionState;
    case 0x34: return PacketType::ChannelSimulateConnectionState;
    case 0x35: return PacketType::HandshakeHost;
    case 0x36: return PacketType::HandshakeReply;
    case 0x37: return PacketType::GetMixerState;
    case 0x38: return PacketType::MixerState;
    case 0x39: return PacketType::HandshakeClient;
    case 0x40: return PacketType::FxTempo;
    case 0x41: return PacketType::SelectOutput;
    case 0x42: return PacketType::RequestData;
    case 0x43: return PacketType::TransferData;
    case 0x44: return PacketType::AckData;
    case 0x45: return PacketType::SetMidi;
    case 0x46: return PacketType::GetMidi;
    case 0x47: return PacketType::SetFxPresetDescription;
    case 0x48: return PacketType::SetFxPresetDescriptionAck;
    case 0x49: return PacketType::ChannelReserved;
    case 0x4A: return PacketType::ChannelDelay;
    case 0x4B: return PacketType::SysExMidiDump;
    default: return std::nullopt;
    }
}

QString packetTypeName(const quint8 value)
{
    const auto known = knownPacketType(value);
    if (!known.has_value()) {
        return QStringLiteral("unknown-0x%1").arg(value, 2, 16, QLatin1Char('0'));
    }
    switch (*known) {
    case PacketType::Pan: return QStringLiteral("pan");
    case PacketType::Solo: return QStringLiteral("solo");
    case PacketType::Gain: return QStringLiteral("gain");
    case PacketType::GraphicEq: return QStringLiteral("graphic-eq");
    case PacketType::HighPassFilter: return QStringLiteral("high-pass-filter");
    case PacketType::Label: return QStringLiteral("label");
    case PacketType::RouteLevel: return QStringLiteral("route-or-master-level");
    case PacketType::GetSnapshotNames: return QStringLiteral("get-snapshot-names");
    case PacketType::Mute: return QStringLiteral("mute");
    case PacketType::ParametricEq: return QStringLiteral("parametric-eq");
    case PacketType::Phase: return QStringLiteral("phase");
    case PacketType::FxSetup: return QStringLiteral("fx-setup");
    case PacketType::SnapshotDelete: return QStringLiteral("snapshot-delete");
    case PacketType::Compressor: return QStringLiteral("compressor");
    case PacketType::Limiter: return QStringLiteral("limiter");
    case PacketType::Phantom: return QStringLiteral("phantom");
    case PacketType::GetChannelState: return QStringLiteral("get-channel-state");
    case PacketType::InputState: return QStringLiteral("input-state");
    case PacketType::OutputState: return QStringLiteral("output-state");
    case PacketType::SnapshotSave: return QStringLiteral("snapshot-save");
    case PacketType::SnapshotLoad: return QStringLiteral("snapshot-load");
    case PacketType::MeterRequest: return QStringLiteral("meter-request");
    case PacketType::MeterUpdate: return QStringLiteral("meter-update");
    case PacketType::GetChannelLabels: return QStringLiteral("get-channel-labels");
    case PacketType::ChannelLabels: return QStringLiteral("channel-labels");
    case PacketType::Setting: return QStringLiteral("setting");
    case PacketType::GetSetting: return QStringLiteral("get-setting");
    case PacketType::SnapshotNames: return QStringLiteral("snapshot-names");
    case PacketType::FactoryReset: return QStringLiteral("factory-reset");
    case PacketType::FxState: return QStringLiteral("fx-state");
    case PacketType::FxPreset: return QStringLiteral("fx-preset");
    case PacketType::SnapshotRename: return QStringLiteral("snapshot-rename");
    case PacketType::ChannelConnectionState: return QStringLiteral("channel-connection-state");
    case PacketType::ChannelSimulateConnectionState: return QStringLiteral("channel-simulate-connection-state");
    case PacketType::HandshakeHost: return QStringLiteral("handshake-host");
    case PacketType::HandshakeReply: return QStringLiteral("handshake-reply");
    case PacketType::GetMixerState: return QStringLiteral("get-mixer-state");
    case PacketType::MixerState: return QStringLiteral("mixer-state-raw");
    case PacketType::HandshakeClient: return QStringLiteral("handshake-client");
    case PacketType::FxTempo: return QStringLiteral("fx-tempo");
    case PacketType::SelectOutput: return QStringLiteral("select-output");
    case PacketType::RequestData: return QStringLiteral("request-data");
    case PacketType::TransferData: return QStringLiteral("transfer-data");
    case PacketType::AckData: return QStringLiteral("ack-data");
    case PacketType::SetMidi: return QStringLiteral("set-midi");
    case PacketType::GetMidi: return QStringLiteral("get-midi");
    case PacketType::SetFxPresetDescription: return QStringLiteral("set-fx-preset-description");
    case PacketType::SetFxPresetDescriptionAck: return QStringLiteral("set-fx-preset-description-ack");
    case PacketType::ChannelReserved: return QStringLiteral("channel-reserved");
    case PacketType::ChannelDelay: return QStringLiteral("channel-delay");
    case PacketType::SysExMidiDump: return QStringLiteral("sysex-midi-dump");
    }
    return QStringLiteral("unknown");
}

model::EvidenceStatus packetEvidence(const quint8 value) noexcept
{
    if (apkCommandDescriptor(value).has_value()) {
        return model::EvidenceStatus::VerifiedFromApk;
    }
    return knownPacketType(value).has_value()
        ? model::EvidenceStatus::Inferred : model::EvidenceStatus::Unknown;
}

QByteArray frameSingleFragment(const quint8 type, const QByteArrayView payload)
{
    QByteArray packet;
    packet.reserve(payload.size() + 3);
    packet.append(static_cast<char>(type));
    packet.append(static_cast<char>(1));
    packet.append(payload.data(), payload.size());
    packet.append(static_cast<char>(checksum(packet)));
    return packet;
}

QVector<QByteArray> frameCommand(
    const quint8 type, const QByteArrayView payload,
    const qsizetype maxRawPacketSize, const quint8 sequenceId)
{
    if (maxRawPacketSize < 3 || payload.size() >= 500) {
        return {};
    }
    const qsizetype singleCapacity = maxRawPacketSize - 3;
    if (payload.size() <= singleCapacity) {
        return {frameSingleFragment(type, payload)};
    }
    if (maxRawPacketSize < 6) {
        return {};
    }

    const qsizetype chunkSize = maxRawPacketSize - 5;
    const qsizetype count = (payload.size() + chunkSize - 1) / chunkSize;
    if (count <= 1 || count >= 34 || count > 255) {
        return {};
    }

    QVector<QByteArray> frames;
    frames.reserve(static_cast<int>(count));
    for (qsizetype index = 0; index < count; ++index) {
        const qsizetype offset = index * chunkSize;
        const qsizetype size = qMin(chunkSize, payload.size() - offset);
        QByteArray frame;
        frame.reserve(size + 5);
        frame.append(static_cast<char>(type));
        frame.append(static_cast<char>(count));
        frame.append(static_cast<char>(sequenceId));
        frame.append(static_cast<char>(index));
        frame.append(payload.data() + offset, size);
        frame.append(static_cast<char>(checksum(frame)));
        frames.append(std::move(frame));
    }
    return frames;
}

PacketParseResult parsePacket(const QByteArrayView raw)
{
    if (raw.size() < 3) {
        return {
            .packet = std::nullopt,
            .error = PacketError::TooShort,
            .message = QStringLiteral("packet is shorter than 3 bytes"),
        };
    }
    if (!hasValidChecksum(raw)) {
        return {
            .packet = std::nullopt,
            .error = PacketError::ChecksumMismatch,
            .message = QStringLiteral("packet checksum does not match"),
        };
    }

    const quint8 fragmentCount = static_cast<quint8>(raw[1]);
    if (fragmentCount == 0) {
        return {
            .packet = std::nullopt,
            .error = PacketError::InvalidFragmentCount,
            .message = QStringLiteral("packet fragment count is zero"),
        };
    }
    if (fragmentCount >= 34) {
        return {
            .packet = std::nullopt,
            .error = PacketError::TooManyFragments,
            .message = QStringLiteral("packet fragment count exceeds APK reassembly limit"),
        };
    }
    const qsizetype headerSize = fragmentCount > 1 ? 4 : 2;
    if (raw.size() < headerSize + 1) {
        return {
            .packet = std::nullopt,
            .error = PacketError::MissingFragmentHeader,
            .message = QStringLiteral("multi-fragment packet is missing header bytes"),
        };
    }

    Packet packet;
    packet.type = static_cast<quint8>(raw[0]);
    packet.fragmentCount = fragmentCount;
    if (fragmentCount > 1) {
        packet.sequenceId = static_cast<quint8>(raw[2]);
        packet.fragmentIndex = static_cast<quint8>(raw[3]);
        if (*packet.fragmentIndex >= fragmentCount) {
            return {
                .packet = std::nullopt,
                .error = PacketError::InvalidFragmentIndex,
                .message = QStringLiteral("packet fragment index exceeds fragment count"),
            };
        }
        packet.fragmentHeaderA = packet.sequenceId;
        packet.fragmentHeaderB = packet.fragmentIndex;
    }
    packet.payload = QByteArray(raw.data() + headerSize, raw.size() - headerSize - 1);
    packet.raw = QByteArray(raw.data(), raw.size());
    packet.evidence = packetEvidence(packet.type);
    const auto descriptor = apkCommandDescriptor(packet.type);
    packet.payloadEvidence = descriptor.has_value()
        ? descriptor->payloadEvidence : model::EvidenceStatus::Unknown;
    return {
        .packet = std::move(packet),
        .error = std::nullopt,
        .message = {},
    };
}

} // namespace flow8::protocol
