#pragma once

#include <QWidget>

#include <QVector>

class QCheckBox;
class QLabel;
class QToolButton;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class DetailPanel;
class FaderWidget;
class MeterWidget;
class MixSendStrip;

class FxView final : public QWidget {
    Q_OBJECT

public:
    FxView(Flow8Device& device, int engineIndex, QWidget* parent = nullptr);
    void refresh();
    void retranslateUi();

private:
    Flow8Device& device_;
    int engineIndex_ {};
    QLabel* title_ {};
    QLabel* note_ {};
    QLabel* masterLabel_ {};
    FaderWidget* master_ {};
    QVector<MixSendStrip*> sends_;
    QVector<QCheckBox*> outputRoutes_;
    DetailPanel* details_ {};
};

class MonitorView final : public QWidget {
    Q_OBJECT

public:
    MonitorView(Flow8Device& device, int monitorIndex, QWidget* parent = nullptr);
    void refresh();
    void retranslateUi();

private:
    Flow8Device& device_;
    int monitorIndex_ {};
    QLabel* title_ {};
    QLabel* note_ {};
    QCheckBox* stereoLink_ {};
    QVector<MixSendStrip*> sends_;
    DetailPanel* details_ {};
};

class MainView final : public QWidget {
    Q_OBJECT

public:
    explicit MainView(Flow8Device& device, QWidget* parent = nullptr);
    void refresh();
    void retranslateUi();

private:
    Flow8Device& device_;
    QLabel* title_ {};
    QLabel* note_ {};
    QVector<MixSendStrip*> sends_;
    DetailPanel* details_ {};
};

class MainOutView final : public QWidget {
    Q_OBJECT

public:
    explicit MainOutView(Flow8Device& device, QWidget* parent = nullptr);
    void refresh();
    void retranslateUi();

private:
    Flow8Device& device_;
    QLabel* title_ {};
    QLabel* subtitle_ {};
    QLabel* outputState_ {};
    QLabel* delayState_ {};
    QLabel* outputLevelMode_ {};
    FaderWidget* fader_ {};
    MeterWidget* meter_ {};
    QToolButton* mute_ {};
};

} // namespace flow8::ui
