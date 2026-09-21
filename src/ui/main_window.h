#pragma once

#include "ui/common/flow_layer_bar.h"

#include <QMainWindow>

class QEvent;
class QKeyEvent;
class QStackedWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ChannelEditView;
class ConnectionBar;
class LanguageManager;
class MainOutView;
class MixerWidget;
class SessionStartView;
class SetupWindow;
class StageView;

class MainWindow final : public QMainWindow {
    Q_OBJECT

public:
    MainWindow(Flow8Device& device, LanguageManager& languageManager,
               QWidget* parent = nullptr);

protected:
    void changeEvent(QEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;

private:
    void showLayer(FlowLayer layer);
    void showChannelEdit(int channelIndex);
    void showSetup(int section);
    void retranslateUi();

    Flow8Device& device_;
    LanguageManager& languageManager_;
    ConnectionBar* connectionBar_ {};
    FlowLayerBar* layerBar_ {};
    MixerWidget* mixer_ {};
    StageView* stage_ {};
    MainOutView* mainOut_ {};
    ChannelEditView* channelEdit_ {};
    SetupWindow* setup_ {};
    SessionStartView* sessionStart_ {};
    QStackedWidget* workspace_ {};
    bool pendingAssistedSetup_ {};
    bool pendingSnapshots_ {};
};

} // namespace flow8::ui
