#pragma once

#include <QWidget>

class QCheckBox;
class QSlider;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class MainStrip final : public QWidget {
    Q_OBJECT

public:
    explicit MainStrip(Flow8Device& device, QWidget* parent = nullptr);
    void refresh();

private:
    Flow8Device& device_;
    QSlider* fader_ {};
    QCheckBox* mute_ {};
};

} // namespace flow8::ui
