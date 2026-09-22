#pragma once

#include <QByteArray>
#include <QByteArrayView>
#include <QString>

#include <optional>

namespace flow8::protocol {

enum class Fix8Format : quint8 {
    Pan = 1,
    UnitInterval = 2,
    FaderDb = 3,
    EqGainDb = 4,
    GainDb = 5,
    Q = 6,
};

[[nodiscard]] std::optional<quint8> encodeFix8(Fix8Format format, double value) noexcept;
[[nodiscard]] double decodeFix8(Fix8Format format, quint8 value) noexcept;

class FieldWriter final {
public:
    void u8(quint8 value);
    void u16be(quint16 value);
    void u32be(quint32 value);
    void raw(QByteArrayView value);
    [[nodiscard]] bool fix8(Fix8Format format, double value);
    [[nodiscard]] bool lengthPrefixed(QByteArrayView value, qsizetype maximumBytes);

    [[nodiscard]] const QByteArray& bytes() const noexcept;
    [[nodiscard]] QByteArray takeBytes();

private:
    QByteArray bytes_;
};

class FieldReader final {
public:
    explicit FieldReader(QByteArrayView bytes) noexcept;

    [[nodiscard]] std::optional<quint8> u8() noexcept;
    [[nodiscard]] std::optional<quint16> u16be() noexcept;
    [[nodiscard]] std::optional<quint32> u32be() noexcept;
    [[nodiscard]] std::optional<double> fix8(Fix8Format format) noexcept;
    [[nodiscard]] std::optional<QByteArray> raw(qsizetype size) noexcept;
    [[nodiscard]] std::optional<QByteArray> lengthPrefixed(qsizetype maximumBytes) noexcept;
    [[nodiscard]] qsizetype remaining() const noexcept;
    [[nodiscard]] bool atEnd() const noexcept;

private:
    QByteArrayView bytes_;
    qsizetype offset_ {};
};

} // namespace flow8::protocol
