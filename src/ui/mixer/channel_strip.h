#pragma once

#include <QWidget>

class QLabel;
class QSlider;
class QToolButton;
class QMouseEvent;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class FaderWidget;
class MeterWidget;

class ChannelStrip final : public QWidget {
    Q_OBJECT

public:
    ChannelStrip(Flow8Device& device, int channelIndex, QWidget* parent = nullptr);

    [[nodiscard]] int channelIndex() const noexcept;
    void setSelected(bool selected);
    void refresh();
    void retranslateUi();

signals:
    void selected(int channelIndex);

protected:
    void mousePressEvent(QMouseEvent* event) override;

private:
    void requestSelection();

    Flow8Device& device_;
    int channelIndex_ {};
    QLabel* nameLabel_ {};
    QLabel* iconLabel_ {};
    QLabel* typeLabel_ {};
    QLabel* eqIndicator_ {};
    QLabel* compressorIndicator_ {};
    QLabel* sendIndicator_ {};
    QLabel* gainLabel_ {};
    QLabel* panLabel_ {};
    QSlider* gainSlider_ {};
    FaderWidget* fader_ {};
    MeterWidget* meter_ {};
    QToolButton* muteButton_ {};
    QToolButton* soloButton_ {};
    QSlider* panSlider_ {};
};

} // namespace flow8::ui
