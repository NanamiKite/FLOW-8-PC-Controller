#pragma once

#include <QWidget>

#include <QVector>

class QLabel;
class QPushButton;
class QToolButton;
class QHBoxLayout;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class FaderWidget;
class MeterWidget;

// A performance-oriented view with deliberately larger, reduced controls. It
// shares Flow8Device/Flow8State with Mixer View and never talks to a transport.
class StageView final : public QWidget {
    Q_OBJECT

public:
    explicit StageView(Flow8Device& device, QWidget* parent = nullptr);

    void refresh();
    void retranslateUi();

private:
    struct Card {
        QWidget* root {};
        QLabel* icon {};
        QLabel* name {};
        QLabel* type {};
        MeterWidget* meter {};
        FaderWidget* fader {};
        QToolButton* mute {};
        QToolButton* solo {};
    };

    void rebuild();

    Flow8Device& device_;
    QWidget* cardsContainer_ {};
    QHBoxLayout* cardsLayout_ {};
    QLabel* title_ {};
    QLabel* description_ {};
    QLabel* tempo_ {};
    QPushButton* tapTempo_ {};
    QVector<Card> cards_;
};

} // namespace flow8::ui
