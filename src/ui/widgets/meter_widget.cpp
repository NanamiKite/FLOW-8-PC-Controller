#include "ui/widgets/meter_widget.h"

#include <QPainter>

#include <algorithm>
#include <cmath>

namespace flow8::ui {

MeterWidget::MeterWidget(QWidget* parent)
    : QWidget(parent)
{
    setMinimumSize(14, 180);
    setMaximumWidth(20);
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

void MeterWidget::paintEvent(QPaintEvent*)
{
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const QRectF bar(3.0, 4.0, width() - 6.0, height() - 8.0);
    painter.setPen(QPen(QColor(53, 57, 64), 1.0));
    painter.setBrush(QColor(19, 21, 25));
    painter.drawRoundedRect(bar, 3.0, 3.0);

    const double top = bar.bottom() - displayLevel_ * bar.height();
    const QRectF levelRect(bar.left() + 2.0, top, bar.width() - 4.0, bar.bottom() - top - 1.0);
    QColor color(73, 187, 126);
    if (displayLevel_ > 0.86) {
        color = QColor(239, 174, 76);
    }
    if (displayLevel_ > 0.96 || clipping_) {
        color = QColor(239, 88, 88);
    }
    painter.setPen(Qt::NoPen);
    painter.setBrush(color);
    painter.drawRoundedRect(levelRect, 2.0, 2.0);

    const double peakY = bar.bottom() - peak_ * bar.height();
    painter.setPen(QPen(clipping_ ? QColor(255, 91, 91) : QColor(214, 219, 226), 2.0));
    painter.drawLine(QPointF(bar.left() + 1.0, peakY), QPointF(bar.right() - 1.0, peakY));
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
