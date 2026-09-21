#pragma once

#include "model/evidence_status.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QString>

#include <optional>

namespace flow8::protocol {

enum class PacketType : quint8 {
    ParameterChange = 0x06,
    ConfigRequest = 0x07,
    ContextSubscribe = 0x21,
    Metering = 0x22,
    ParameterResponse = 0x25,
    ParameterQuery = 0x26,
    SnapshotNames = 0x27,
    Identity = 0x35,
    AuthenticationAck = 0x36,
    SessionStart = 0x37,
    StateData = 0x38,
    Authentication = 0x39,
    DumpTrigger = 0x4B,
};

struct Packet {
    quint8 type {};
    quint8 discriminator {};
    QByteArray payload;
    QByteArray raw;
    model::EvidenceStatus evidence {model::EvidenceStatus::Unknown};
};

enum class PacketError {
    TooShort,
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
[[nodiscard]] QByteArray framePacket(quint8 type, quint8 discriminator, QByteArrayView payload = {});
[[nodiscard]] PacketParseResult parsePacket(QByteArrayView raw);

} // namespace flow8::protocol
