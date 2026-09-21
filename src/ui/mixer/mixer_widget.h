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

class MixerWidget final : public QWidget {
    Q_OBJECT

public:
    explicit MixerWidget(Flow8Device& device, QWidget* parent = nullptr);

private:
    void rebuild();
    void refreshChannel(int index);
    void refreshAll();

    Flow8Device& device_;
    QWidget* stripsContainer_ {};
    QHBoxLayout* stripsLayout_ {};
    QVector<ChannelStrip*> channelStrips_;
    MainStrip* mainStrip_ {};
};

} // namespace flow8::ui
