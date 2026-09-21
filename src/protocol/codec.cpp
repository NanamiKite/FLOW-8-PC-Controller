#include "protocol/codec.h"

#include <cmath>

namespace flow8::protocol {

quint8 checksum(const QByteArrayView bytes) noexcept
{
    quint8 result = 0;
    for (const char byte : bytes) {
        result = static_cast<quint8>(result + static_cast<quint8>(byte));
    }
    return result;
}

bool hasValidChecksum(const QByteArrayView packet) noexcept
{
    return packet.size() >= 2
        && checksum(packet.first(packet.size() - 1)) == static_cast<quint8>(packet.back());
}

std::optional<quint8> encodeUnitInterval(const double value) noexcept
{
    if (!std::isfinite(value) || value < 0.0 || value > 1.0) {
        return std::nullopt;
    }
    return static_cast<quint8>(std::lround(value * 255.0));
}

double decodeUnitInterval(const quint8 value) noexcept
{
    return static_cast<double>(value) / 255.0;
}

QByteArray toHexBytes(const QByteArrayView bytes)
{
    return QByteArray(bytes.data(), bytes.size()).toHex(' ');
}

} // namespace flow8::protocol
