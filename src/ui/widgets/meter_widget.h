#pragma once

#include <QTimer>
#include <QWidget>

#include <algorithm>

namespace flow8::ui {

inline constexpr double meterMinimumDb = -60.0;
inline constexpr double meterMaximumDb = 10.0;

[[nodiscard]] constexpr double meterDbFromNormalized(const double normalized) noexcept
{
    const double bounded = std::clamp(normalized, 0.0, 1.0);
    return meterMinimumDb + bounded * (meterMaximumDb - meterMinimumDb);
}

[[nodiscard]] constexpr double normalizedMeterFromDb(const double db) noexcept
{
    return std::clamp(
        (db - meterMinimumDb) / (meterMaximumDb - meterMinimumDb), 0.0, 1.0);
}

// Mixer meters beside a route fader show the signal after that route level.
// The raw, pre-fader observation remains in Flow8State::MeterState; this helper
// derives only the presentation value and does not mutate control state.
[[nodiscard]] double postFaderMeterNormalized(
    double sourceLevel, double faderValue,
    double faderMinimumDb = -70.0, double faderMaximumDb = 10.0) noexcept;

class MeterWidget final : public QWidget {
    Q_OBJECT

public:
    explicit MeterWidget(QWidget* parent = nullptr);

    void setLevel(double level);
    void setPeak(double peak);
    void setClipping(bool clipping);
    [[nodiscard]] double targetLevelDb() const noexcept;

protected:
    void paintEvent(QPaintEvent* event) override;

private:
    void animate();

    QTimer animationTimer_;
    double targetLevel_ {};
    double displayLevel_ {};
    double peak_ {};
    bool clipping_ {};
};

} // namespace flow8::ui
