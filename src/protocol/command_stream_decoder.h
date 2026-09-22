#pragma once

#include "protocol/command_codec.h"
#include "protocol/fragment_reassembler.h"

namespace flow8::protocol {

struct StreamDecodeResult {
    std::optional<DecodedCommand> command;
    bool awaitingFragments {};
    QString message;

    [[nodiscard]] bool ok() const noexcept { return command.has_value(); }
};

class CommandStreamDecoder final {
public:
    [[nodiscard]] StreamDecodeResult accept(QByteArrayView rawPacket);
    void reset() noexcept;
    [[nodiscard]] qsizetype pendingAssemblyCount() const noexcept;

private:
    FragmentReassembler reassembler_;
};

} // namespace flow8::protocol
