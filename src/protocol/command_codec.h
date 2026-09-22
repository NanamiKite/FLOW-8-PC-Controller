#pragma once

#include "protocol/flow8_command.h"

#include <QByteArray>
#include <QString>
#include <QVector>

#include <optional>

namespace flow8::protocol {

enum class CommandCodecError {
    None,
    UnsupportedCommand,
    InvalidSemanticValue,
    InvalidPayloadLength,
    InvalidPayloadValue,
    FragmentationFailed,
};

struct CommandEncodeResult {
    QVector<QByteArray> packets;
    CommandCodecError error {CommandCodecError::None};
    QString message;

    [[nodiscard]] bool ok() const noexcept { return !packets.isEmpty(); }
};

struct CommandDecodeResult {
    std::optional<DecodedCommand> command;
    CommandCodecError error {CommandCodecError::None};
    QString message;

    [[nodiscard]] bool ok() const noexcept { return command.has_value(); }
};

[[nodiscard]] CommandEncodeResult encodeCommand(
    const Flow8Command& command, qsizetype maxRawPacketSize = 251,
    quint8 sequenceId = 0);
[[nodiscard]] CommandDecodeResult decodeCommandPayload(
    quint8 commandId, QByteArrayView payload);

} // namespace flow8::protocol
