#pragma once

#include "model/evidence_status.h"
#include "model/endpoint.h"
#include "protocol/apk_command_catalog.h"

#include <QByteArray>
#include <QString>

#include <optional>

namespace flow8::protocol {

// Transport-neutral semantic input to the APK-confirmed 0x02 encoder. The
// value is already in dB; unlike RouteLevel it is never normalized first.
struct GainCommand {
    model::EndpointId inputEndpoint {model::EndpointId::Input1};
    double gainDb {};
    model::EvidenceStatus semanticEvidence {model::EvidenceStatus::VerifiedFromApk};
};

struct GainPayloadSchema {
    ApkCommandId command {ApkCommandId::Gain};
    model::EvidenceStatus commandEvidence {model::EvidenceStatus::VerifiedFromApk};
    model::EvidenceStatus semanticEvidence {model::EvidenceStatus::VerifiedFromApk};
    model::EvidenceStatus payloadEvidence {model::EvidenceStatus::VerifiedFromApk};
    bool endpointFieldKnown {true};
    bool gainIsDb {true};
    bool fix8Format5Known {true};
    bool nativeClampKnown {true};
    bool nativeRoundingKnown {true};
    bool singleFragmentFramingKnown {true};
    // The native serializer accepts a raw input endpoint byte. Product
    // capability filtering (notably BT/USB) belongs to the semantic layer.
    bool codecEnforcesInputCapability {};
};

enum class GainCodecError {
    None,
    InvalidInputEndpoint,
    InvalidGainValue,
};

struct GainEncodeResult {
    std::optional<QByteArray> packet;
    GainCodecError error {GainCodecError::None};
    QString message;

    [[nodiscard]] bool ok() const noexcept { return packet.has_value(); }
};

[[nodiscard]] constexpr GainPayloadSchema gainPayloadSchema() noexcept
{
    return {};
}

[[nodiscard]] std::optional<GainCodecError> validateGainCommand(
    const GainCommand& command) noexcept;

// APK FIX8 format 5: binary32 clamp to [-60,+60], add 60, multiply by
// two in binary32, convert to double, then lrint using the process rounding
// mode. Non-finite rejection is a PC safety rule rather than APK evidence.
[[nodiscard]] std::optional<quint8> encodeGainFix8Format5(double gainDb) noexcept;

// Encodes the APK-confirmed raw packet:
// 02 01 endpoint gain checksum.
[[nodiscard]] GainEncodeResult encodeGain(const GainCommand& command) noexcept;

} // namespace flow8::protocol
