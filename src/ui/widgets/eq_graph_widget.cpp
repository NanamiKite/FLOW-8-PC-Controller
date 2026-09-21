#include "ui/widgets/eq_graph_widget.h"

#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QLineF>

#include <algorithm>
#include <cmath>

namespace flow8::ui {
namespace {

constexpr double minimumFrequency = 20.0;
constexpr double maximumFrequency = 20000.0;
constexpr double minimumGain = -15.0;
constexpr double maximumGain = 15.0;
constexpr double plotInset = 8.0;
constexpr double plotBottomInset = 20.0;
constexpr double handleHitRadius = 18.0;

} // namespace

EqGraphWidget::EqGraphWidget(QWidget* parent)
    : QWidget(parent)
{
    setMinimumHeight(180);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    setMouseTracking(true);
}

void EqGraphWidget::setBands(QVector<EqGraphBand> bands)
{
    bands_ = std::move(bands);
    if (selectedBand_ >= bands_.size()) {
        selectedBand_ = std::max(0, static_cast<int>(bands_.size()) - 1);
    }
    update();
}

int EqGraphWidget::selectedBand() const noexcept
{
    return selectedBand_;
}

void EqGraphWidget::setSelectedBand(const int index)
{
    if (index < 0 || index >= bands_.size() || selectedBand_ == index) {
        return;
    }
    selectedBand_ = index;
    update();
    emit selectedBandChanged(index);
}

void EqGraphWidget::paintEvent(QPaintEvent*)
{
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.fillRect(rect(), QColor(21, 23, 28));

    painter.setPen(QPen(QColor(50, 54, 62), 1.0));
    for (const double frequency : {20.0, 50.0, 100.0, 200.0, 500.0, 1000.0,
                                    2000.0, 5000.0, 10000.0, 20000.0}) {
        const double x = xForFrequency(frequency);
        painter.drawLine(QPointF(x, 8.0), QPointF(x, height() - 20.0));
    }
    for (const double gain : {-15.0, -10.0, -5.0, 0.0, 5.0, 10.0, 15.0}) {
        const double y = yForGain(gain);
        painter.setPen(QPen(gain == 0.0 ? QColor(78, 84, 94) : QColor(44, 48, 55), 1.0));
        painter.drawLine(QPointF(8.0, y), QPointF(width() - 8.0, y));
    }

    if (!bands_.isEmpty()) {
        QPainterPath curve;
        constexpr int samples = 180;
        for (int sample = 0; sample <= samples; ++sample) {
            const double proportion = static_cast<double>(sample) / samples;
            const double logFrequency = std::log10(minimumFrequency)
                + proportion * (std::log10(maximumFrequency) - std::log10(minimumFrequency));
            const double frequency = std::pow(10.0, logFrequency);
            double gain = 0.0;
            for (const auto& band : bands_) {
                if (!band.available) {
                    continue;
                }
                const double distance = std::log2(frequency / std::max(20.0, band.frequencyHz));
                const double width = std::max(0.18, 1.25 / std::max(0.2, band.q));
                gain += band.gainDb * std::exp(-(distance * distance) / (2.0 * width * width));
            }
            const QPointF point(8.0 + proportion * (width() - 16.0),
                                yForGain(std::clamp(gain, minimumGain, maximumGain)));
            if (sample == 0) {
                curve.moveTo(point);
            } else {
                curve.lineTo(point);
            }
        }
        painter.setPen(QPen(QColor(80, 151, 255), 2.3));
        painter.drawPath(curve);
    }

    for (int index = 0; index < bands_.size(); ++index) {
        const auto point = pointForBand(bands_[index]);
        painter.setPen(QPen(index == selectedBand_ ? QColor(222, 235, 255)
                                                   : QColor(125, 177, 252), 1.5));
        painter.setBrush(index == selectedBand_ ? QColor(80, 151, 255)
                                                 : QColor(48, 93, 158));
        painter.drawEllipse(point, index == selectedBand_ ? 7.0 : 5.0,
                            index == selectedBand_ ? 7.0 : 5.0);
        painter.setPen(QColor(231, 235, 241));
        painter.drawText(QRectF(point.x() - 10.0, point.y() - 9.0, 20.0, 18.0),
                         Qt::AlignCenter, QString::number(index + 1));
    }

    painter.setPen(QColor(126, 132, 143));
    painter.setFont(QFont(font().family(), 8));
    painter.drawText(QRectF(8.0, height() - 18.0, width() - 16.0, 15.0),
                     Qt::AlignLeft, QStringLiteral("20 Hz"));
    painter.drawText(QRectF(8.0, height() - 18.0, width() - 16.0, 15.0),
                     Qt::AlignCenter, QStringLiteral("1 kHz"));
    painter.drawText(QRectF(8.0, height() - 18.0, width() - 16.0, 15.0),
                     Qt::AlignRight, QStringLiteral("20 kHz"));
}

void EqGraphWidget::mousePressEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        const int closest = bandAt(event->position());
        if (closest >= 0) {
            setSelectedBand(closest);
            draggedBand_ = closest;
            setCursor(Qt::ClosedHandCursor);
            editBandAt(closest, event->position());
            event->accept();
            return;
        }
    }
    QWidget::mousePressEvent(event);
}

void EqGraphWidget::mouseMoveEvent(QMouseEvent* event)
{
    if (draggedBand_ >= 0) {
        editBandAt(draggedBand_, event->position());
        event->accept();
        return;
    }

    setCursor(bandAt(event->position()) >= 0
        ? Qt::OpenHandCursor : Qt::ArrowCursor);
    QWidget::mouseMoveEvent(event);
}

void EqGraphWidget::mouseReleaseEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton && draggedBand_ >= 0) {
        editBandAt(draggedBand_, event->position());
        draggedBand_ = -1;
        setCursor(bandAt(event->position()) >= 0
            ? Qt::OpenHandCursor : Qt::ArrowCursor);
        event->accept();
        return;
    }
    QWidget::mouseReleaseEvent(event);
}

int EqGraphWidget::bandAt(const QPointF& position) const
{
    int closest = -1;
    double closestDistance = handleHitRadius;
    for (int index = 0; index < bands_.size(); ++index) {
        if (!bands_[index].available) {
            continue;
        }
        const double candidate = QLineF(position, pointForBand(bands_[index])).length();
        if (candidate < closestDistance) {
            closestDistance = candidate;
            closest = index;
        }
    }
    return closest;
}

void EqGraphWidget::editBandAt(const int index, const QPointF& position)
{
    if (index < 0 || index >= bands_.size() || !bands_[index].available) {
        return;
    }

    auto& band = bands_[index];
    const double gain = gainForY(position.y());
    if (qFuzzyCompare(gain + 1.0, band.gainDb + 1.0)) {
        return;
    }
    band.gainDb = gain;
    update();
    emit bandGainEdited(index, gain);
}

QPointF EqGraphWidget::pointForBand(const EqGraphBand& band) const
{
    return {xForFrequency(band.frequencyHz), yForGain(band.gainDb)};
}

double EqGraphWidget::xForFrequency(const double frequencyHz) const
{
    const double bounded = std::clamp(frequencyHz, minimumFrequency, maximumFrequency);
    const double normalized = (std::log10(bounded) - std::log10(minimumFrequency))
        / (std::log10(maximumFrequency) - std::log10(minimumFrequency));
    return plotInset + normalized * (width() - 2.0 * plotInset);
}

double EqGraphWidget::yForGain(const double gainDb) const
{
    const double normalized = (std::clamp(gainDb, minimumGain, maximumGain) - minimumGain)
        / (maximumGain - minimumGain);
    return (height() - plotBottomInset)
        - normalized * (height() - plotBottomInset - plotInset);
}

double EqGraphWidget::gainForY(const double y) const
{
    const double plotHeight = std::max(1.0, height() - plotBottomInset - plotInset);
    const double normalized = std::clamp(
        ((height() - plotBottomInset) - y) / plotHeight, 0.0, 1.0);
    return minimumGain + normalized * (maximumGain - minimumGain);
}

} // namespace flow8::ui
