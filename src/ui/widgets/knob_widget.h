#pragma once

#include <QWidget>

class QEnterEvent;
class QEvent;

namespace flow8::ui {

class KnobWidget final : public QWidget {
    Q_OBJECT
    Q_PROPERTY(double value READ value WRITE setValue NOTIFY valueChanged)

public:
    enum class DisplayMode {
        Number,
        Decibels,
        PanBalance,
    };
    Q_ENUM(DisplayMode)

    explicit KnobWidget(QWidget* parent = nullptr);

    void setRange(double minimum, double maximum);
    void setSingleStep(double step);
    void setDefaultValue(double value);
    void setDisplayMode(DisplayMode mode);
    [[nodiscard]] double minimum() const noexcept;
    [[nodiscard]] double maximum() const noexcept;
    [[nodiscard]] double value() const noexcept;
    [[nodiscard]] DisplayMode displayMode() const noexcept;
    [[nodiscard]] QString valueText() const;
    void setValue(double value);

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
    void setUserValue(double value);
    [[nodiscard]] double normalizedValue() const noexcept;

    double minimum_ {};
    double maximum_ {1.0};
    double value_ {};
    double defaultValue_ {};
    double singleStep_ {0.01};
    double dragAnchorY_ {};
    double dragStartValue_ {};
    DisplayMode displayMode_ {DisplayMode::Number};
    bool dragging_ {};
};

} // namespace flow8::ui
