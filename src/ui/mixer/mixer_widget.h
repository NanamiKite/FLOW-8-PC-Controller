#pragma once

#include <QWidget>

#include <QVector>

class QHBoxLayout;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ChannelStrip;
class MainStrip;

// The mixer page is intentionally limited to the console surface. Channel
// editing and other layers are separate workspace pages owned by MainWindow.
class MixerWidget final : public QWidget {
    Q_OBJECT

public:
    explicit MixerWidget(Flow8Device& device, QWidget* parent = nullptr);

    void retranslateUi();
    void refreshAll();

signals:
    void channelEditRequested(int channelIndex);
    void mainOutRequested();

private:
    void rebuild();
    void selectChannel(int index);
    void refreshChannel(int index);

    Flow8Device& device_;
    QWidget* stripsContainer_ {};
    QHBoxLayout* stripsLayout_ {};
    QVector<ChannelStrip*> channelStrips_;
    MainStrip* mainStrip_ {};
};

} // namespace flow8::ui
