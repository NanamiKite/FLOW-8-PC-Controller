#include "ui/widgets/knob_widget.h"

#include "ui/ui_text.h"

#include <QEnterEvent>
#include <QKeyEvent>
#include <QLocale>
#include <QMouseEvent>
#include <QPainter>
#include <QSizePolicy>
#include <QWheelEvent>

#include <algorithm>
#include <cmath>
#include <numbers>

namespace flow8::ui {

KnobWidget::KnobWidget(QWidget* parent)
    : QWidget(parent)
{
    setFocusPolicy(Qt::StrongFocus);
    setMinimumSize(104, 126);
    setMaximumWidth(124);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Fixed);
}

void KnobWidget::setRange(const double minimum, const double maximum)
{
    if (!std::isfinite(minimum) || !std::isfinite(maximum) || minimum >= maximum) {
        return;
    }
    minimum_ = minimum;
    maximum_ = maximum;
    value_ = std::clamp(value_, minimum_, maximum_);
    defaultValue_ = std::clamp(defaultValue_, minimum_, maximum_);
    update();
}

void KnobWidget::setSingleStep(const double step)
{
    if (std::isfinite(step) && step > 0.0) {
        singleStep_ = step;
    }
}

void KnobWidget::setDefaultValue(const double value)
{
    defaultValue_ = std::clamp(value, minimum_, maximum_);
}

void KnobWidget::setDisplayMode(const DisplayMode mode)
{
    displayMode_ = mode;
    update();
}

double KnobWidget::minimum() const noexcept
{
    return minimum_;
}

double KnobWidget::maximum() const noexcept
{
    return maximum_;
}

double KnobWidget::value() const noexcept
{
    return value_;
}

KnobWidget::DisplayMode KnobWidget::displayMode() const noexcept
{
    return displayMode_;
}

QString KnobWidget::valueText() const
{
    switch (displayMode_) {
    case DisplayMode::Decibels:
        return decibelValueText(value_);
    case DisplayMode::PanBalance:
        return panBalanceValueText(static_cast<int>(std::lround(value_ * 100.0)));
    case DisplayMode::Number:
        return QLocale().toString(value_, 'f', 2);
    }
    return {};
}

void KnobWidget::setValue(const double value)
{
    const double bounded = std::clamp(value, minimum_, maximum_);
    if (qFuzzyCompare(value_ + 1.0, bounded + 1.0)) {
        return;
    }
    value_ = bounded;
    update();
}

void KnobWidget::paintEvent(QPaintEvent*)
{
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    const double diameter = std::min(width() - 24.0, height() - 44.0);
    const QRectF dial((width() - diameter) / 2.0, 8.0, diameter, diameter);
    const QRectF arc = dial.adjusted(7.0, 7.0, -7.0, -7.0);

    painter.setPen(QPen(QColor(54, 59, 67), 5.0, Qt::SolidLine, Qt::RoundCap));
    painter.drawArc(arc, 225 * 16, -270 * 16);
    painter.setPen(QPen(underMouse() ? QColor(111, 174, 255) : QColor(80, 151, 255),
                        5.0, Qt::SolidLine, Qt::RoundCap));
    painter.drawArc(arc, 225 * 16,
                    static_cast<int>(-270.0 * normalizedValue() * 16.0));

    const QPointF center = dial.center();
    const double angle = (225.0 - 270.0 * normalizedValue())
        * std::numbers::pi / 180.0;
    const double radius = diameter * 0.29;
    const QPointF tip(center.x() + std::cos(angle) * radius,
                      center.y() - std::sin(angle) * radius);
    painter.setPen(QPen(hasFocus() ? QColor(145, 193, 255) : QColor(213, 218, 225),
                        3.0, Qt::SolidLine, Qt::RoundCap));
    painter.drawLine(center, tip);
    painter.setPen(Qt::NoPen);
    painter.setBrush(QColor(40, 44, 51));
    painter.drawEllipse(center, 5.0, 5.0);

    painter.setPen(QColor(208, 213, 220));
    painter.setFont(QFont(font().family(), 9, QFont::DemiBold));
    painter.drawText(QRectF(0.0, height() - 28.0, width(), 22.0),
                     Qt::AlignCenter, valueText());
}

void KnobWidget::enterEvent(QEnterEvent* event)
{
    update();
    QWidget::enterEvent(event);
}

void KnobWidget::leaveEvent(QEvent* event)
{
    update();
    QWidget::leaveEvent(event);
}

void KnobWidget::mousePressEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        dragging_ = true;
        dragAnchorY_ = event->position().y();
        dragStartValue_ = value_;
        setFocus(Qt::MouseFocusReason);
        event->accept();
        return;
    }
    QWidget::mousePressEvent(event);
}

void KnobWidget::mouseMoveEvent(QMouseEvent* event)
{
    if (dragging_) {
        const double precision = event->modifiers().testFlag(Qt::ShiftModifier) ? 5.0 : 1.0;
        const double delta = (dragAnchorY_ - event->position().y()) / (160.0 * precision);
        setUserValue(dragStartValue_ + delta * (maximum_ - minimum_));
        event->accept();
        return;
    }
    QWidget::mouseMoveEvent(event);
}

void KnobWidget::mouseReleaseEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton && dragging_) {
        dragging_ = false;
        event->accept();
        return;
    }
    QWidget::mouseReleaseEvent(event);
}

void KnobWidget::mouseDoubleClickEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        setUserValue(defaultValue_);
        event->accept();
        return;
    }
    QWidget::mouseDoubleClickEvent(event);
}

void KnobWidget::wheelEvent(QWheelEvent* event)
{
    const double step = event->modifiers().testFlag(Qt::ShiftModifier)
        ? singleStep_ / 5.0 : singleStep_;
    setUserValue(value_ + (event->angleDelta().y() >= 0 ? step : -step));
    event->accept();
}

void KnobWidget::keyPressEvent(QKeyEvent* event)
{
    const double step = event->modifiers().testFlag(Qt::ShiftModifier)
        ? singleStep_ / 5.0 : singleStep_;
    if (event->key() == Qt::Key_Up || event->key() == Qt::Key_Right) {
        setUserValue(value_ + step);
        event->accept();
        return;
    }
    if (event->key() == Qt::Key_Down || event->key() == Qt::Key_Left) {
        setUserValue(value_ - step);
        event->accept();
        return;
    }
    if (event->key() == Qt::Key_Home) {
        setUserValue(maximum_);
        event->accept();
        return;
    }
    if (event->key() == Qt::Key_End) {
        setUserValue(minimum_);
        event->accept();
        return;
    }
    QWidget::keyPressEvent(event);
}

void KnobWidget::setUserValue(const double value)
{
    const double bounded = std::clamp(value, minimum_, maximum_);
    if (qFuzzyCompare(value_ + 1.0, bounded + 1.0)) {
        return;
    }
    value_ = bounded;
    update();
    emit valueChanged(value_);
}

double KnobWidget::normalizedValue() const noexcept
{
    return (value_ - minimum_) / (maximum_ - minimum_);
}

} // namespace flow8::ui
