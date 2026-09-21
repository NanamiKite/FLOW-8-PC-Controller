#include "ui/widgets/fader_widget.h"

#include "ui/ui_text.h"

#include <QKeyEvent>
#include <QEnterEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QLocale>
#include <QWheelEvent>

#include <algorithm>
#include <array>
#include <cmath>

namespace flow8::ui {
namespace {

QString decibelText(const double value)
{
    if (value <= 0.0001) {
        return QStringLiteral("−∞");
    }
    const double db = -70.0 + value * 80.0;
    return QLocale().toString(db, 'f', 1);
}

} // namespace

FaderWidget::FaderWidget(QWidget* parent)
    : QWidget(parent)
{
    setFocusPolicy(Qt::StrongFocus);
    setMinimumSize(62, 230);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    retranslateUi();
}

void FaderWidget::retranslateUi()
{
    setAccessibleName(uiText("Channel fader"));
    setToolTip(uiText("Fader; double-click to reset to 0 dB"));
}

double FaderWidget::value() const noexcept
{
    return value_;
}

void FaderWidget::setValue(const double value)
{
    const double bounded = std::clamp(value, 0.0, 1.0);
    if (qFuzzyCompare(value_, bounded)) {
        return;
    }
    value_ = bounded;
    update();
}

void FaderWidget::paintEvent(QPaintEvent*)
{
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    const QRectF track = trackRect();
    painter.setPen(Qt::NoPen);
    painter.setBrush(QColor(31, 34, 39));
    painter.drawRoundedRect(track.adjusted(-4.0, -2.0, 4.0, 2.0), 5.0, 5.0);
    painter.setBrush(QColor(84, 90, 99));
    painter.drawRoundedRect(track, 2.0, 2.0);

    const double thumbY = track.bottom() - value_ * track.height();
    const QRectF fill(track.left(), thumbY, track.width(), track.bottom() - thumbY);
    painter.setBrush(underMouse() ? QColor(101, 166, 255) : QColor(80, 151, 255));
    painter.drawRoundedRect(fill, 2.0, 2.0);

    painter.setPen(QColor(105, 111, 121));
    const std::array<double, 6> marks {0.0, 0.125, 0.5, 0.75, 0.875, 1.0};
    for (const double mark : marks) {
        const double y = track.bottom() - mark * track.height();
        painter.drawLine(QPointF(track.right() + 7.0, y), QPointF(track.right() + 12.0, y));
    }

    QRectF thumb(track.center().x() - 22.0, thumbY - 7.0, 44.0, 14.0);
    painter.setPen(QPen(hasFocus() ? QColor(132, 184, 255) : QColor(135, 140, 149), 1.0));
    painter.setBrush(QColor(202, 207, 214));
    painter.drawRoundedRect(thumb, 4.0, 4.0);
    painter.setPen(QPen(QColor(75, 80, 88), 1.0));
    painter.drawLine(QPointF(thumb.left() + 8.0, thumb.center().y()),
                     QPointF(thumb.right() - 8.0, thumb.center().y()));

    painter.setPen(QColor(189, 194, 202));
    painter.setFont(QFont(font().family(), 9, QFont::DemiBold));
    painter.drawText(QRectF(0.0, height() - 24.0, width(), 20.0), Qt::AlignCenter,
                     decibelText(value_) + QStringLiteral(" dB"));
}

void FaderWidget::enterEvent(QEnterEvent* event)
{
    update();
    QWidget::enterEvent(event);
}

void FaderWidget::leaveEvent(QEvent* event)
{
    update();
    QWidget::leaveEvent(event);
}

void FaderWidget::mousePressEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        dragging_ = true;
        dragAnchorY_ = event->position().y();
        dragStartValue_ = value_;
        setFocus(Qt::MouseFocusReason);
        if (!event->modifiers().testFlag(Qt::ShiftModifier)) {
            setValueFromPosition(event->position());
        }
        event->accept();
        return;
    }
    QWidget::mousePressEvent(event);
}

void FaderWidget::mouseMoveEvent(QMouseEvent* event)
{
    if (dragging_) {
        if (event->modifiers().testFlag(Qt::ShiftModifier)) {
            const double delta = (dragAnchorY_ - event->position().y())
                / (trackRect().height() * 5.0);
            setUserValue(dragStartValue_ + delta);
        } else {
            setValueFromPosition(event->position());
        }
        event->accept();
        return;
    }
    QWidget::mouseMoveEvent(event);
}

void FaderWidget::mouseReleaseEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton && dragging_) {
        dragging_ = false;
        event->accept();
        return;
    }
    QWidget::mouseReleaseEvent(event);
}

void FaderWidget::mouseDoubleClickEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        setUserValue(0.875); // 0 dB on the project's -70..+10 normalized scale.
        event->accept();
        return;
    }
    QWidget::mouseDoubleClickEvent(event);
}

void FaderWidget::wheelEvent(QWheelEvent* event)
{
    const double step = event->modifiers().testFlag(Qt::ShiftModifier) ? 0.002 : 0.01;
    const int direction = event->angleDelta().y() >= 0 ? 1 : -1;
    setUserValue(value_ + step * direction);
    event->accept();
}

void FaderWidget::keyPressEvent(QKeyEvent* event)
{
    const double step = event->modifiers().testFlag(Qt::ShiftModifier) ? 0.002 : 0.01;
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
        setUserValue(1.0);
        event->accept();
        return;
    }
    if (event->key() == Qt::Key_End) {
        setUserValue(0.0);
        event->accept();
        return;
    }
    QWidget::keyPressEvent(event);
}

QRectF FaderWidget::trackRect() const
{
    return QRectF(width() / 2.0 - 3.0, 13.0, 6.0, std::max(40, height() - 50));
}

void FaderWidget::setValueFromPosition(const QPointF& position)
{
    const QRectF track = trackRect();
    setUserValue((track.bottom() - position.y()) / track.height());
}

void FaderWidget::setUserValue(const double value)
{
    const double bounded = std::clamp(value, 0.0, 1.0);
    if (qFuzzyCompare(value_, bounded)) {
        return;
    }
    value_ = bounded;
    update();
    emit valueChanged(value_);
}

} // namespace flow8::ui
