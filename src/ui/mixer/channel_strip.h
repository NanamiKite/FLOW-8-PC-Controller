#pragma once

#include <QWidget>

class QCheckBox;
class QDial;
class QLabel;
class QSlider;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ChannelStrip final : public QWidget {
    Q_OBJECT

public:
    ChannelStrip(Flow8Device& device, int channelIndex, QWidget* parent = nullptr);

    [[nodiscard]] int channelIndex() const noexcept;
    void refresh();

private:
    Flow8Device& device_;
    int channelIndex_ {};
    QLabel* nameLabel_ {};
    QSlider* gainSlider_ {};
    QSlider* faderSlider_ {};
    QCheckBox* muteButton_ {};
    QCheckBox* soloButton_ {};
    QDial* panDial_ {};
};

} // namespace flow8::ui
