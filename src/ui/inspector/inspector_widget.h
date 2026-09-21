#pragma once

#include "model/routing.h"

#include <QWidget>

#include <QVector>

class QCheckBox;
class QComboBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QSlider;
class QTabWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class EqGraphWidget;

class InspectorWidget final : public QWidget {
    Q_OBJECT

public:
    explicit InspectorWidget(Flow8Device& device, QWidget* parent = nullptr);

    void setSelectedChannel(int index);
    void setSelectedDestination(model::RoutingDestination destination);
    [[nodiscard]] int selectedChannel() const noexcept;
    [[nodiscard]] model::RoutingDestination selectedDestination() const noexcept;
    void refresh();
    void retranslateUi();

private:
    Flow8Device& device_;
    int selectedChannel_ {};
    model::RoutingDestination selectedDestination_ {model::RoutingDestination::Main};
    QLabel* title_ {};
    QLabel* metadata_ {};
    QLabel* inputCapabilities_ {};
    QLabel* evidenceNote_ {};
    QLabel* syntheticBadge_ {};
    QLabel* currentRouteLabel_ {};
    QSlider* currentRoute_ {};
    QLineEdit* channelName_ {};
    QComboBox* channelIcon_ {};
    QCheckBox* channelVisible_ {};
    QSlider* gain_ {};
    QCheckBox* phantom_ {};
    QCheckBox* phase_ {};
    QCheckBox* lowCutEnabled_ {};
    QSlider* lowCutFrequency_ {};
    QSlider* pan_ {};
    QCheckBox* mute_ {};
    QCheckBox* solo_ {};
    QVector<QLabel*> channelFormLabels_;
    QPushButton* ezGainSelected_ {};
    QPushButton* ezGainAll_ {};
    QPushButton* ezGainCancel_ {};
    QLabel* ezGainStatus_ {};
    QTabWidget* tabs_ {};
    EqGraphWidget* eqGraph_ {};
    QVector<QSlider*> eqGainSliders_;
    QVector<QLabel*> eqBandLabels_;
    QSlider* compressorAmount_ {};
    QLabel* gainReduction_ {};
    QVector<QLabel*> compressorLabels_;
    QVector<QLabel*> compressorUnavailable_;
    QVector<QSlider*> sendSliders_;
    QVector<QLabel*> sendLabels_;
    QVector<QComboBox*> monitorSendModes_;
};

} // namespace flow8::ui
