#include "protocol/packet.h"

#include "protocol/codec.h"

namespace flow8::protocol {

std::optional<PacketType> knownPacketType(const quint8 value) noexcept
{
    switch (value) {
    case 0x06: return PacketType::ParameterChange;
    case 0x07: return PacketType::ConfigRequest;
    case 0x21: return PacketType::ContextSubscribe;
    case 0x22: return PacketType::Metering;
    case 0x25: return PacketType::ParameterResponse;
    case 0x26: return PacketType::ParameterQuery;
    case 0x27: return PacketType::SnapshotNames;
    case 0x35: return PacketType::Identity;
    case 0x36: return PacketType::AuthenticationAck;
    case 0x37: return PacketType::SessionStart;
    case 0x38: return PacketType::StateData;
    case 0x39: return PacketType::Authentication;
    case 0x4B: return PacketType::DumpTrigger;
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
    case PacketType::ParameterChange: return QStringLiteral("parameter-change");
    case PacketType::ConfigRequest: return QStringLiteral("config-request");
    case PacketType::ContextSubscribe: return QStringLiteral("context-subscribe");
    case PacketType::Metering: return QStringLiteral("metering");
    case PacketType::ParameterResponse: return QStringLiteral("parameter-response");
    case PacketType::ParameterQuery: return QStringLiteral("parameter-query");
    case PacketType::SnapshotNames: return QStringLiteral("snapshot-names");
    case PacketType::Identity: return QStringLiteral("identity");
    case PacketType::AuthenticationAck: return QStringLiteral("authentication-ack");
    case PacketType::SessionStart: return QStringLiteral("session-start");
    case PacketType::StateData: return QStringLiteral("state-data-raw");
    case PacketType::Authentication: return QStringLiteral("authentication");
    case PacketType::DumpTrigger: return QStringLiteral("dump-trigger");
    }
    return QStringLiteral("unknown");
}

model::EvidenceStatus packetEvidence(const quint8 value) noexcept
{
    return knownPacketType(value).has_value() ? model::EvidenceStatus::Inferred
                                              : model::EvidenceStatus::Unknown;
}

QByteArray framePacket(const quint8 type, const quint8 discriminator, const QByteArrayView payload)
{
    QByteArray packet;
    packet.reserve(payload.size() + 3);
    packet.append(static_cast<char>(type));
    packet.append(static_cast<char>(discriminator));
    packet.append(payload.data(), payload.size());
    packet.append(static_cast<char>(checksum(packet)));
    return packet;
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

    Packet packet;
    packet.type = static_cast<quint8>(raw[0]);
    packet.discriminator = static_cast<quint8>(raw[1]);
    packet.payload = QByteArray(raw.data() + 2, raw.size() - 3);
    packet.raw = QByteArray(raw.data(), raw.size());
    packet.evidence = packetEvidence(packet.type);
    return {
        .packet = std::move(packet),
        .error = std::nullopt,
        .message = {},
    };
}

} // namespace flow8::protocol
