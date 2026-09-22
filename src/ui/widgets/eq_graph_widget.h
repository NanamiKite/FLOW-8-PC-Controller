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
    void bandGainEdited(int index, double gainDb);

protected:
    void paintEvent(QPaintEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;

private:
    [[nodiscard]] int bandAt(const QPointF& position) const;
    void editBandAt(int index, const QPointF& position);
    [[nodiscard]] QPointF pointForBand(const EqGraphBand& band) const;
    [[nodiscard]] double xForFrequency(double frequencyHz) const;
    [[nodiscard]] double yForGain(double gainDb) const;
    [[nodiscard]] double gainForY(double y) const;

    QVector<EqGraphBand> bands_;
    int selectedBand_ {};
    int draggedBand_ {-1};
};

} // namespace flow8::ui
