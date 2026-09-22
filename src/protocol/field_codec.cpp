#include "protocol/field_codec.h"

#include "protocol/gain_codec.h"
#include "protocol/route_level_codec.h"

#include <algorithm>
#include <cmath>
#include <utility>

namespace flow8::protocol {

std::optional<quint8> encodeFix8(const Fix8Format format, const double value) noexcept
{
    if (!std::isfinite(value)) {
        return std::nullopt;
    }

    const float nativeValue = static_cast<float>(value);
    switch (format) {
    case Fix8Format::Pan: {
        const float clamped = std::clamp(nativeValue, -1.0F, 1.0F);
        return static_cast<quint8>(std::trunc(clamped * 127.0F + 127.0F));
    }
    case Fix8Format::UnitInterval: {
        const float clamped = std::clamp(nativeValue, 0.0F, 1.0F);
        return static_cast<quint8>(std::trunc(clamped * 255.0F));
    }
    case Fix8Format::FaderDb:
        return encodeRouteLevelFix8(nativeValue);
    case Fix8Format::EqGainDb: {
        const float clamped = std::clamp(nativeValue, -15.0F, 15.0F);
        const float scaled = clamped / 15.0F * 127.0F + 127.0F;
        return static_cast<quint8>(std::lrint(static_cast<double>(scaled)));
    }
    case Fix8Format::GainDb:
        return encodeGainFix8Format5(value);
    case Fix8Format::Q: {
        if (nativeValue <= 0.0F) {
            return quint8(0);
        }
        if (nativeValue <= 10.7F) {
            const long code = std::lrint(
                static_cast<double>((nativeValue - 0.3F) * 10.0F)) + 1L;
            return static_cast<quint8>(std::clamp(code, 1L, 101L));
        }
        const long code = std::lrint(static_cast<double>(nativeValue - 11.0F)) + 102L;
        return static_cast<quint8>(std::clamp(code, 102L, 255L));
    }
    }
    return std::nullopt;
}

double decodeFix8(const Fix8Format format, const quint8 value) noexcept
{
    switch (format) {
    case Fix8Format::Pan:
        return (static_cast<double>(value) - 127.0) / 127.0;
    case Fix8Format::UnitInterval:
        return static_cast<double>(value) / 255.0;
    case Fix8Format::FaderDb:
        return static_cast<double>(routeLevelFix8DbTable()[value]);
    case Fix8Format::EqGainDb:
        return (static_cast<double>(value) - 127.0) / 127.0 * 15.0;
    case Fix8Format::GainDb:
        return static_cast<double>(value) / 2.0 - 60.0;
    case Fix8Format::Q:
        if (value == 0) {
            return 0.0;
        }
        if (value <= 101) {
            return 0.3 + static_cast<double>(value - 1) / 10.0;
        }
        return static_cast<double>(value) - 91.0;
    }
    return 0.0;
}

void FieldWriter::u8(const quint8 value)
{
    bytes_.append(static_cast<char>(value));
}

void FieldWriter::u16be(const quint16 value)
{
    u8(static_cast<quint8>(value >> 8U));
    u8(static_cast<quint8>(value));
}

void FieldWriter::u32be(const quint32 value)
{
    u8(static_cast<quint8>(value >> 24U));
    u8(static_cast<quint8>(value >> 16U));
    u8(static_cast<quint8>(value >> 8U));
    u8(static_cast<quint8>(value));
}

void FieldWriter::raw(const QByteArrayView value)
{
    bytes_.append(value.data(), value.size());
}

bool FieldWriter::fix8(const Fix8Format format, const double value)
{
    const auto encoded = encodeFix8(format, value);
    if (!encoded.has_value()) {
        return false;
    }
    u8(*encoded);
    return true;
}

bool FieldWriter::lengthPrefixed(
    const QByteArrayView value, const qsizetype maximumBytes)
{
    if (value.size() < 0 || value.size() > maximumBytes || value.size() > 255) {
        return false;
    }
    u8(static_cast<quint8>(value.size()));
    raw(value);
    return true;
}

const QByteArray& FieldWriter::bytes() const noexcept
{
    return bytes_;
}

QByteArray FieldWriter::takeBytes()
{
    return std::move(bytes_);
}

FieldReader::FieldReader(const QByteArrayView bytes) noexcept
    : bytes_(bytes)
{
}

std::optional<quint8> FieldReader::u8() noexcept
{
    if (remaining() < 1) {
        return std::nullopt;
    }
    return static_cast<quint8>(bytes_[offset_++]);
}

std::optional<quint16> FieldReader::u16be() noexcept
{
    const auto high = u8();
    const auto low = u8();
    if (!high.has_value() || !low.has_value()) {
        return std::nullopt;
    }
    return static_cast<quint16>((static_cast<quint16>(*high) << 8U) | *low);
}

std::optional<quint32> FieldReader::u32be() noexcept
{
    const auto a = u8();
    const auto b = u8();
    const auto c = u8();
    const auto d = u8();
    if (!a.has_value() || !b.has_value() || !c.has_value() || !d.has_value()) {
        return std::nullopt;
    }
    return (static_cast<quint32>(*a) << 24U)
        | (static_cast<quint32>(*b) << 16U)
        | (static_cast<quint32>(*c) << 8U)
        | static_cast<quint32>(*d);
}

std::optional<double> FieldReader::fix8(const Fix8Format format) noexcept
{
    const auto encoded = u8();
    return encoded.has_value()
        ? std::optional<double>(decodeFix8(format, *encoded)) : std::nullopt;
}

std::optional<QByteArray> FieldReader::raw(const qsizetype size) noexcept
{
    if (size < 0 || remaining() < size) {
        return std::nullopt;
    }
    QByteArray result(bytes_.data() + offset_, size);
    offset_ += size;
    return result;
}

std::optional<QByteArray> FieldReader::lengthPrefixed(
    const qsizetype maximumBytes) noexcept
{
    const auto size = u8();
    if (!size.has_value() || *size > maximumBytes) {
        return std::nullopt;
    }
    return raw(*size);
}

qsizetype FieldReader::remaining() const noexcept
{
    return bytes_.size() - offset_;
}

bool FieldReader::atEnd() const noexcept
{
    return remaining() == 0;
}

} // namespace flow8::protocol
