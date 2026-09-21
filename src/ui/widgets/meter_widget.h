#pragma once

#include <QTimer>
#include <QWidget>

namespace flow8::ui {

class MeterWidget final : public QWidget {
    Q_OBJECT

public:
    explicit MeterWidget(QWidget* parent = nullptr);

    void setLevel(double level);
    void setPeak(double peak);
    void setClipping(bool clipping);

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
