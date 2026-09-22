#include "protocol/fragment_reassembler.h"

#include <utility>

namespace flow8::protocol {
namespace {

constexpr qsizetype maximumAssemblySlots = 4;
constexpr qsizetype maximumPayloadBytes = 499;

ReassemblyResult errorResult(const ReassemblyError error, QString message)
{
    return {
        .command = std::nullopt,
        .error = error,
        .message = std::move(message),
    };
}

} // namespace

ReassemblyResult FragmentReassembler::accept(const QByteArrayView rawPacket)
{
    const PacketParseResult parsed = parsePacket(rawPacket);
    if (!parsed.ok()) {
        return errorResult(ReassemblyError::InvalidPacket, parsed.message);
    }

    const Packet& packet = *parsed.packet;
    if (packet.fragmentCount == 1) {
        return {
            .command = ReassembledCommand {
                .commandId = packet.type,
                .payload = packet.payload,
                .fragmented = false,
                .sequenceId = std::nullopt,
            },
        };
    }

    if (!packet.sequenceId.has_value() || !packet.fragmentIndex.has_value()) {
        return errorResult(
            ReassemblyError::InvalidPacket,
            QStringLiteral("fragmented packet is missing sequence metadata"));
    }

    Slot* slot = nullptr;
    for (auto& candidate : slots_) {
        if (candidate.commandId == packet.type
            && candidate.sequenceId == *packet.sequenceId) {
            slot = &candidate;
            break;
        }
    }

    if (slot != nullptr && slot->fragmentCount != packet.fragmentCount) {
        return errorResult(
            ReassemblyError::ConflictingSequence,
            QStringLiteral("sequence was reused with a different fragment count"));
    }

    if (slot == nullptr) {
        if (slots_.size() >= maximumAssemblySlots) {
            return errorResult(
                ReassemblyError::NoFreeSlot,
                QStringLiteral("all four APK-compatible reassembly slots are occupied"));
        }
        Slot fresh;
        fresh.commandId = packet.type;
        fresh.sequenceId = *packet.sequenceId;
        fresh.fragmentCount = packet.fragmentCount;
        fresh.fragments.resize(packet.fragmentCount);
        slots_.append(std::move(fresh));
        slot = &slots_.last();
    }

    auto& destination = slot->fragments[*packet.fragmentIndex];
    if (destination.has_value()) {
        if (*destination != packet.payload) {
            return errorResult(
                ReassemblyError::ConflictingDuplicate,
                QStringLiteral("duplicate fragment carries different payload bytes"));
        }
    } else {
        destination = packet.payload;
    }

    qsizetype assembledSize = 0;
    bool complete = true;
    for (const auto& fragment : slot->fragments) {
        if (!fragment.has_value()) {
            complete = false;
            continue;
        }
        assembledSize += fragment->size();
    }
    if (assembledSize > maximumPayloadBytes) {
        for (qsizetype index = 0; index < slots_.size(); ++index) {
            if (&slots_[index] == slot) {
                slots_.removeAt(index);
                break;
            }
        }
        return errorResult(
            ReassemblyError::PayloadTooLarge,
            QStringLiteral("assembled command exceeds the APK 499-byte payload limit"));
    }
    if (!complete) {
        return {};
    }

    QByteArray payload;
    payload.reserve(assembledSize);
    for (const auto& fragment : slot->fragments) {
        payload.append(*fragment);
    }
    const ReassembledCommand command {
        .commandId = slot->commandId,
        .payload = std::move(payload),
        .fragmented = true,
        .sequenceId = slot->sequenceId,
    };
    for (qsizetype index = 0; index < slots_.size(); ++index) {
        if (&slots_[index] == slot) {
            slots_.removeAt(index);
            break;
        }
    }
    return {.command = command};
}

void FragmentReassembler::clear() noexcept
{
    slots_.clear();
}

qsizetype FragmentReassembler::pendingAssemblyCount() const noexcept
{
    return slots_.size();
}

} // namespace flow8::protocol
