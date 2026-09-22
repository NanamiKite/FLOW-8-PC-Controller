#include "protocol/command_stream_decoder.h"

namespace flow8::protocol {

StreamDecodeResult CommandStreamDecoder::accept(const QByteArrayView rawPacket)
{
    const ReassemblyResult reassembled = reassembler_.accept(rawPacket);
    if (!reassembled.ok()) {
        return {.message = reassembled.message};
    }
    if (!reassembled.complete()) {
        return {.awaitingFragments = true};
    }
    const auto decoded = decodeCommandPayload(
        reassembled.command->commandId, reassembled.command->payload);
    if (!decoded.ok()) {
        return {.message = decoded.message};
    }
    return {.command = decoded.command};
}

void CommandStreamDecoder::reset() noexcept
{
    reassembler_.clear();
}

qsizetype CommandStreamDecoder::pendingAssemblyCount() const noexcept
{
    return reassembler_.pendingAssemblyCount();
}

} // namespace flow8::protocol
