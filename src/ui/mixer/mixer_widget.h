#pragma once

#include <QWidget>

#include <QVector>

class QHBoxLayout;
class QLabel;
class QScrollArea;
class QSplitter;
class QStackedWidget;
class QToolButton;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ChannelStrip;
class DetailPanel;
class InspectorWidget;
class MainStrip;
class StageView;

class MixerWidget final : public QWidget {
    Q_OBJECT

public:
    explicit MixerWidget(Flow8Device& device, QWidget* parent = nullptr);
    void retranslateUi();
    void showMixer();
    void showSnapshots();

private:
    void rebuild();
    void selectChannel(int index);
    void refreshChannel(int index);
    void refreshAll();

    Flow8Device& device_;
    QWidget* stripsContainer_ {};
    QHBoxLayout* stripsLayout_ {};
    QVector<ChannelStrip*> channelStrips_;
    MainStrip* mainStrip_ {};
    InspectorWidget* inspector_ {};
    DetailPanel* detailPanel_ {};
    QStackedWidget* detailStack_ {};
    QStackedWidget* workspaceStack_ {};
    QScrollArea* mixerScroll_ {};
    QSplitter* mixerSplitter_ {};
    StageView* stageView_ {};
    QLabel* sectionTitle_ {};
    QLabel* syntheticBadge_ {};
    QVector<QToolButton*> navigationButtons_;
};

} // namespace flow8::ui
