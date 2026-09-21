#include "ui/widgets/meter_widget.h"

#include "ui/ui_text.h"

#include <QPainter>
#include <QSizePolicy>

#include <algorithm>
#include <array>
#include <cmath>

namespace flow8::ui {

double postFaderMeterNormalized(
    const double sourceLevel, const double faderValue,
    const double faderMinimumDb, const double faderMaximumDb) noexcept
{
    const double boundedSource = std::clamp(sourceLevel, 0.0, 1.0);
    const double boundedFader = std::clamp(faderValue, 0.0, 1.0);
    if (boundedSource <= 0.0 || boundedFader <= 0.0
        || !std::isfinite(faderMinimumDb) || !std::isfinite(faderMaximumDb)
        || faderMinimumDb >= faderMaximumDb) {
        return 0.0;
    }
    const double faderDb = faderMinimumDb
        + boundedFader * (faderMaximumDb - faderMinimumDb);
    return normalizedMeterFromDb(meterDbFromNormalized(boundedSource) + faderDb);
}

MeterWidget::MeterWidget(QWidget* parent)
    : QWidget(parent)
{
    setMinimumSize(52, 180);
    setMaximumWidth(52);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    setAccessibleName(uiText("Level"));
    setToolTip(QStringLiteral("-60…+10 dB"));
    animationTimer_.setInterval(30);
    connect(&animationTimer_, &QTimer::timeout, this, &MeterWidget::animate);
    animationTimer_.start();
}

void MeterWidget::setLevel(const double level)
{
    targetLevel_ = std::clamp(level, 0.0, 1.0);
}

void MeterWidget::setPeak(const double peak)
{
    peak_ = std::clamp(peak, 0.0, 1.0);
    update();
}

void MeterWidget::setClipping(const bool clipping)
{
    clipping_ = clipping;
    update();
}

double MeterWidget::targetLevelDb() const noexcept
{
    return meterDbFromNormalized(targetLevel_);
}

void MeterWidget::paintEvent(QPaintEvent*)
{
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const QRectF bar(4.0, 13.0, 10.0, std::max(40, height() - 50));
    painter.setPen(QPen(QColor(53, 57, 64), 1.0));
    painter.setBrush(QColor(19, 21, 25));
    painter.drawRoundedRect(bar, 3.0, 3.0);

    const double top = bar.bottom() - displayLevel_ * bar.height();
    const QRectF levelRect(
        bar.left() + 2.0, top, bar.width() - 4.0,
        std::max(0.0, bar.bottom() - top - 1.0));
    QColor color(73, 187, 126);
    if (displayLevel_ > 0.86) {
        color = QColor(239, 174, 76);
    }
    if (displayLevel_ > 0.96 || clipping_) {
        color = QColor(239, 88, 88);
    }
    painter.setPen(Qt::NoPen);
    painter.setBrush(color);
    if (levelRect.height() > 0.0) {
        painter.drawRoundedRect(levelRect, 2.0, 2.0);
    }

    const double peakY = bar.bottom() - peak_ * bar.height();
    painter.setPen(QPen(clipping_ ? QColor(255, 91, 91) : QColor(214, 219, 226), 2.0));
    painter.drawLine(QPointF(bar.left() + 1.0, peakY), QPointF(bar.right() - 1.0, peakY));

    painter.setFont(QFont(font().family(), 7));
    painter.setPen(QColor(132, 138, 147));
    constexpr std::array scaleDb {10.0, 0.0, -20.0, -40.0, -60.0};
    for (const double db : scaleDb) {
        const double normalized = (db - meterMinimumDb) / (meterMaximumDb - meterMinimumDb);
        const double y = bar.bottom() - normalized * bar.height();
        const QString label = db > 0.0
            ? QStringLiteral("+%1").arg(static_cast<int>(db))
            : QString::number(static_cast<int>(db));
        painter.drawText(QRectF(18.0, y - 7.0, width() - 19.0, 14.0),
                         Qt::AlignLeft | Qt::AlignVCenter, label);
    }

    painter.setFont(QFont(font().family(), 7, QFont::DemiBold));
    painter.setPen(clipping_ ? QColor(255, 91, 91) : QColor(189, 194, 202));
    painter.drawText(QRectF(0.0, height() - 24.0, width(), 20.0), Qt::AlignCenter,
                     decibelValueText(meterDbFromNormalized(displayLevel_)));
}

void MeterWidget::animate()
{
    if (targetLevel_ >= displayLevel_) {
        displayLevel_ += (targetLevel_ - displayLevel_) * 0.55;
    } else {
        displayLevel_ = std::max(targetLevel_, displayLevel_ - 0.025);
    }
    peak_ = std::max(displayLevel_, peak_ - 0.008);
    if (std::abs(targetLevel_ - displayLevel_) > 0.001 || peak_ > 0.001) {
        update();
    }
}

} // namespace flow8::ui
