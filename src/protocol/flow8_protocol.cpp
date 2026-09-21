#include "protocol/flow8_protocol.h"

#include "protocol/codec.h"

namespace flow8::protocol {

QByteArray encodeParameterChange(const quint8 channel, const quint8 parameter, const quint8 value)
{
    const char payload[] = {
        static_cast<char>(channel),
        static_cast<char>(parameter),
        static_cast<char>(value),
    };
    return framePacket(static_cast<quint8>(PacketType::ParameterChange), 0x01,
                       QByteArrayView(payload, 3));
}

std::optional<ParameterChange> decodeParameterChange(const Packet& packet) noexcept
{
    if (packet.type != static_cast<quint8>(PacketType::ParameterChange)
        || packet.discriminator != 0x01 || packet.payload.size() != 3) {
        return std::nullopt;
    }
    return ParameterChange {
        .channel = static_cast<quint8>(packet.payload[0]),
        .parameter = static_cast<quint8>(packet.payload[1]),
        .value = static_cast<quint8>(packet.payload[2]),
        .evidence = model::EvidenceStatus::Inferred,
    };
}

std::optional<QByteArray> encodeFaderLevel(const quint8 oneBasedChannel,
                                           const double normalized) noexcept
{
    const auto encoded = encodeUnitInterval(normalized);
    if (!encoded.has_value() || oneBasedChannel == 0) {
        return std::nullopt;
    }
    return encodeParameterChange(oneBasedChannel, faderLevelParameter, *encoded);
}

QByteArray referenceAuthenticationPacket()
{
    return QByteArray::fromHex("3901fd062b0639f17fe7b7278b8f355a495c2a");
}

QByteArray sessionStartPacket()
{
    return framePacket(static_cast<quint8>(PacketType::SessionStart), 0x01);
}

QByteArray configRequestPacket()
{
    return framePacket(static_cast<quint8>(PacketType::ConfigRequest), 0x01);
}

QByteArray dumpTriggerPacket()
{
    return framePacket(static_cast<quint8>(PacketType::DumpTrigger), 0x01);
}

} // namespace flow8::protocol
