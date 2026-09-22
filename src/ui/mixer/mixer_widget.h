#pragma once

#include "model/routing.h"

#include <QWidget>

#include <QVector>

class QHBoxLayout;
class QButtonGroup;
class QLabel;
class QPushButton;
class QStackedWidget;
class QToolButton;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ChannelStrip;
class DetailPanel;
class InspectorWidget;

// The mixer page is intentionally limited to the console surface. Channel
// editing and other layers are separate workspace pages owned by MainWindow.
class MixerWidget final : public QWidget {
    Q_OBJECT

public:
    explicit MixerWidget(Flow8Device& device, QWidget* parent = nullptr);

    void retranslateUi();
    void refreshAll();
    void setDestination(model::RoutingDestination destination);
    [[nodiscard]] model::RoutingDestination destination() const noexcept;
    [[nodiscard]] int selectedChannel() const noexcept;

signals:
    void channelEditRequested(int channelIndex);
    void mainOutRequested();
    void destinationChanged(flow8::model::RoutingDestination destination);

private:
    void rebuild();
    void selectChannel(int index);
    void showMaster();
    void refreshChannel(int index);

    Flow8Device& device_;
    QWidget* stripsContainer_ {};
    QHBoxLayout* stripsLayout_ {};
    QVector<ChannelStrip*> channelStrips_;
    QButtonGroup* destinationGroup_ {};
    QVector<QToolButton*> destinationButtons_;
    QLabel* routeTitle_ {};
    QPushButton* masterButton_ {};
    QStackedWidget* inspectorStack_ {};
    DetailPanel* masterPanel_ {};
    InspectorWidget* inputInspector_ {};
    model::RoutingDestination destination_ {model::RoutingDestination::Main};
    int selectedChannel_ {-1};
};

} // namespace flow8::ui
