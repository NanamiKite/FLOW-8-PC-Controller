#pragma once

#include <QWidget>

class QCheckBox;
class QLabel;
class QPushButton;
class QSlider;
class QToolButton;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class FaderWidget;
class InspectorWidget;

class ChannelEditView final : public QWidget {
    Q_OBJECT

public:
    explicit ChannelEditView(Flow8Device& device, QWidget* parent = nullptr);

    void setChannel(int index);
    [[nodiscard]] int channel() const noexcept;
    void refresh();
    void retranslateUi();

signals:
    void backRequested();

private:
    Flow8Device& device_;
    int channelIndex_ {};
    QPushButton* back_ {};
    QLabel* title_ {};
    QLabel* identity_ {};
    QLabel* preampTitle_ {};
    QLabel* mixTitle_ {};
    QLabel* gainLabel_ {};
    QLabel* panLabel_ {};
    QSlider* gain_ {};
    QCheckBox* phantom_ {};
    FaderWidget* fader_ {};
    QSlider* pan_ {};
    QToolButton* mute_ {};
    QToolButton* solo_ {};
    InspectorWidget* inspector_ {};
};

} // namespace flow8::ui
