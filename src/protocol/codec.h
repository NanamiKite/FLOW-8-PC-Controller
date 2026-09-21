#pragma once

#include <QByteArray>
#include <QByteArrayView>

#include <optional>

namespace flow8::protocol {

[[nodiscard]] quint8 checksum(QByteArrayView bytes) noexcept;
[[nodiscard]] bool hasValidChecksum(QByteArrayView packet) noexcept;
[[nodiscard]] std::optional<quint8> encodeUnitInterval(double value) noexcept;
[[nodiscard]] double decodeUnitInterval(quint8 value) noexcept;
[[nodiscard]] QByteArray toHexBytes(QByteArrayView bytes);

} // namespace flow8::protocol
