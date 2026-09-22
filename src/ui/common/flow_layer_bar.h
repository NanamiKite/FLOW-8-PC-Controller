#pragma once

#include <QWidget>

#include <QVector>

class QButtonGroup;
class QToolButton;

namespace flow8::ui {

enum class FlowLayer {
    Mixer,
    Stage,
    Fx1,
    Fx2,
    Monitor1,
    Monitor2,
    Main,
    MainOut,
};

class FlowLayerBar final : public QWidget {
    Q_OBJECT

public:
    explicit FlowLayerBar(QWidget* parent = nullptr);

    [[nodiscard]] FlowLayer currentLayer() const noexcept;
    void setCurrentLayer(FlowLayer layer);
    void retranslateUi();

signals:
    void layerSelected(flow8::ui::FlowLayer layer);

private:
    QButtonGroup* group_ {};
    QVector<QToolButton*> buttons_;
    FlowLayer currentLayer_ {FlowLayer::Mixer};
};

} // namespace flow8::ui

Q_DECLARE_METATYPE(flow8::ui::FlowLayer)
