#pragma once

#include <QVector>
#include <QWidget>

namespace flow8::ui {

struct EqGraphBand {
    double frequencyHz {1000.0};
    double gainDb {};
    double q {1.0};
    bool available {true};
};

class EqGraphWidget final : public QWidget {
    Q_OBJECT

public:
    explicit EqGraphWidget(QWidget* parent = nullptr);

    void setBands(QVector<EqGraphBand> bands);
    [[nodiscard]] int selectedBand() const noexcept;
    void setSelectedBand(int index);

signals:
    void selectedBandChanged(int index);

protected:
    void paintEvent(QPaintEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;

private:
    [[nodiscard]] QPointF pointForBand(const EqGraphBand& band) const;
    [[nodiscard]] double xForFrequency(double frequencyHz) const;
    [[nodiscard]] double yForGain(double gainDb) const;

    QVector<EqGraphBand> bands_;
    int selectedBand_ {};
};

} // namespace flow8::ui
