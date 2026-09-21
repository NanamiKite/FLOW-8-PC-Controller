#pragma once

#include <QWidget>

class QEnterEvent;
class QEvent;

namespace flow8::ui {

class FaderWidget final : public QWidget {
    Q_OBJECT
    Q_PROPERTY(double value READ value WRITE setValue NOTIFY valueChanged)

public:
    explicit FaderWidget(QWidget* parent = nullptr);

    [[nodiscard]] double value() const noexcept;
    void setValue(double value);
    void retranslateUi();

signals:
    void valueChanged(double value);

protected:
    void paintEvent(QPaintEvent* event) override;
    void enterEvent(QEnterEvent* event) override;
    void leaveEvent(QEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void mouseDoubleClickEvent(QMouseEvent* event) override;
    void wheelEvent(QWheelEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;

private:
    [[nodiscard]] QRectF trackRect() const;
    void setValueFromPosition(const QPointF& position);
    void setUserValue(double value);

    double value_ {0.75};
    bool dragging_ {};
    double dragAnchorY_ {};
    double dragStartValue_ {};
};

} // namespace flow8::ui
