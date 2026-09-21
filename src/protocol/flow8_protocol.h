#pragma once

#include "protocol/packet.h"

#include <QByteArray>

#include <optional>

namespace flow8::protocol {

inline constexpr quint8 faderLevelParameter = 0x0F;

struct ParameterChange {
    quint8 channel {};
    quint8 parameter {};
    quint8 value {};
    model::EvidenceStatus evidence {model::EvidenceStatus::Inferred};
};

[[nodiscard]] QByteArray encodeParameterChange(quint8 channel, quint8 parameter, quint8 value);
[[nodiscard]] std::optional<ParameterChange> decodeParameterChange(const Packet& packet) noexcept;
[[nodiscard]] std::optional<QByteArray> encodeFaderLevel(quint8 oneBasedChannel,
                                                        double normalized) noexcept;

// Copied from retained reference evidence. INFERRED until verified on project hardware.
[[nodiscard]] QByteArray referenceAuthenticationPacket();
[[nodiscard]] QByteArray sessionStartPacket();
[[nodiscard]] QByteArray configRequestPacket();
[[nodiscard]] QByteArray dumpTriggerPacket();

} // namespace flow8::protocol
