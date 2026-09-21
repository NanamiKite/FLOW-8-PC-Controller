#include "protocol/flow8_protocol.h"

#include "protocol/codec.h"

namespace flow8::protocol {

QByteArray encodeLegacyReferenceParameterChange(
    const quint8 channel, const quint8 parameter, const quint8 value)
{
    const char payload[] = {
        static_cast<char>(channel),
        static_cast<char>(parameter),
        static_cast<char>(value),
    };
    return frameSingleFragment(static_cast<quint8>(PacketType::RouteLevel),
                               QByteArrayView(payload, 3));
}

std::optional<LegacyReferenceParameterChange>
decodeLegacyReferenceParameterChange(const Packet& packet) noexcept
{
    if (packet.type != static_cast<quint8>(PacketType::RouteLevel)
        || packet.fragmentCount != 1 || packet.payload.size() != 3) {
        return std::nullopt;
    }
    return LegacyReferenceParameterChange {
        .channel = static_cast<quint8>(packet.payload[0]),
        .parameter = static_cast<quint8>(packet.payload[1]),
        .value = static_cast<quint8>(packet.payload[2]),
        .evidence = model::EvidenceStatus::Inferred,
    };
}

std::optional<QByteArray> encodeLegacyReferenceFaderLevel(
    const quint8 oneBasedChannel, const double normalized) noexcept
{
    const auto encoded = encodeLegacyUnitInterval8(normalized);
    if (!encoded.has_value() || oneBasedChannel == 0) {
        return std::nullopt;
    }
    return encodeLegacyReferenceParameterChange(
        oneBasedChannel, legacyReferenceFaderLevelParameter, *encoded);
}

QByteArray referenceAuthenticationPacket()
{
    return QByteArray::fromHex("3901fd062b0639f17fe7b7278b8f355a495c2a");
}

QByteArray referenceSessionStartPacket()
{
    return frameSingleFragment(static_cast<quint8>(PacketType::GetMixerState));
}

QByteArray referenceConfigRequestPacket()
{
    return frameSingleFragment(static_cast<quint8>(PacketType::GetSnapshotNames));
}

QByteArray referenceDumpTriggerPacket()
{
    return frameSingleFragment(static_cast<quint8>(PacketType::DumpTrigger));
}

} // namespace flow8::protocol
