#pragma once

#include <QByteArray>
#include <QByteArrayView>

#include <optional>

namespace flow8::protocol {

[[nodiscard]] quint8 checksum(QByteArrayView bytes) noexcept;
[[nodiscard]] bool hasValidChecksum(QByteArrayView packet) noexcept;
// 8-bit unit-interval conversion retained only for the isolated legacy
// reference candidate. It is not the APK 0x06 normalized-value wire encoding.
[[nodiscard]] std::optional<quint8> encodeLegacyUnitInterval8(double value) noexcept;
[[nodiscard]] double decodeLegacyUnitInterval8(quint8 value) noexcept;
[[nodiscard]] QByteArray toHexBytes(QByteArrayView bytes);

} // namespace flow8::protocol
