#pragma once

#include "protocol/packet.h"

#include <QByteArray>
#include <QVector>

#include <optional>

namespace flow8::protocol {

enum class ReassemblyError {
    None,
    InvalidPacket,
    ConflictingSequence,
    ConflictingDuplicate,
    PayloadTooLarge,
    NoFreeSlot,
};

struct ReassembledCommand {
    quint8 commandId {};
    QByteArray payload;
    bool fragmented {};
    std::optional<quint8> sequenceId;
};

struct ReassemblyResult {
    std::optional<ReassembledCommand> command;
    ReassemblyError error {ReassemblyError::None};
    QString message;

    [[nodiscard]] bool complete() const noexcept { return command.has_value(); }
    [[nodiscard]] bool ok() const noexcept { return error == ReassemblyError::None; }
};

// APK-native compatible receive-side fragment accumulator. It never exposes
// partial payloads to command parsers or state code. Four concurrent slots and
// the <500-byte assembled-payload limit mirror the recovered native queue.
class FragmentReassembler final {
public:
    [[nodiscard]] ReassemblyResult accept(QByteArrayView rawPacket);
    void clear() noexcept;
    [[nodiscard]] qsizetype pendingAssemblyCount() const noexcept;

private:
    struct Slot {
        quint8 commandId {};
        quint8 sequenceId {};
        quint8 fragmentCount {};
        QVector<std::optional<QByteArray>> fragments;
    };

    QVector<Slot> slots_;
};

} // namespace flow8::protocol
