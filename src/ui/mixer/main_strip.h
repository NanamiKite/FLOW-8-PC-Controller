#pragma once

#include <QWidget>

class QToolButton;
class QLabel;
class QMouseEvent;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class FaderWidget;
class MeterWidget;

class MainStrip final : public QWidget {
    Q_OBJECT

public:
    explicit MainStrip(Flow8Device& device, QWidget* parent = nullptr);
    void refresh();
    void retranslateUi();

signals:
    void selected();
    void editRequested();

protected:
    void mouseDoubleClickEvent(QMouseEvent* event) override;

private:
    Flow8Device& device_;
    FaderWidget* fader_ {};
    MeterWidget* meter_ {};
    QToolButton* mute_ {};
    QLabel* title_ {};
    QLabel* badge_ {};
    QLabel* processors_ {};
};

} // namespace flow8::ui
