#pragma once

#include "protocol/packet.h"

#include <QByteArray>

#include <optional>

namespace flow8::protocol {

inline constexpr quint8 legacyReferenceFaderLevelParameter = 0x0F;

// Candidate payload model retained from reference/flow-8-midi. The current
// APK/native evidence identifies command 0x06 as route/master level but does
// not confirm this byte layout, so it must never drive production state.
struct LegacyReferenceParameterChange {
    quint8 channel {};
    quint8 parameter {};
    quint8 value {};
    model::EvidenceStatus evidence {model::EvidenceStatus::Inferred};
};

[[nodiscard]] QByteArray encodeLegacyReferenceParameterChange(
    quint8 channel, quint8 parameter, quint8 value);
[[nodiscard]] std::optional<LegacyReferenceParameterChange>
decodeLegacyReferenceParameterChange(const Packet& packet) noexcept;
[[nodiscard]] std::optional<QByteArray> encodeLegacyReferenceFaderLevel(
    quint8 oneBasedChannel, double normalized) noexcept;

// Copied from retained reference evidence. INFERRED until verified on project hardware.
[[nodiscard]] QByteArray referenceAuthenticationPacket();
[[nodiscard]] QByteArray referenceSessionStartPacket();
[[nodiscard]] QByteArray referenceConfigRequestPacket();
[[nodiscard]] QByteArray referenceDumpTriggerPacket();

} // namespace flow8::protocol
