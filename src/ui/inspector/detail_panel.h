#pragma once

#include <QWidget>

#include <QVector>

class QCheckBox;
class QComboBox;
class QLabel;
class QListWidget;
class QLineEdit;
class QPushButton;
class QSlider;
class QStackedWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class EqGraphWidget;
class MeterWidget;

class DetailPanel final : public QWidget {
    Q_OBJECT

public:
    explicit DetailPanel(Flow8Device& device, QWidget* parent = nullptr);

    void showBus(int busIndex);
    void showFx(int effectIndex);
    void showSnapshots();
    void showRouting();
    void refresh();
    void retranslateUi();

private:
    void refreshBus();
    void refreshBusMeter();
    void refreshFx();
    void refreshFxMeter();
    void refreshSnapshots();
    void refreshRouting();

    Flow8Device& device_;
    QStackedWidget* pages_ {};

    int selectedBus_ {};
    QLabel* busTitle_ {};
    QLabel* busCapability_ {};
    MeterWidget* busMeter_ {};
    QSlider* busLevel_ {};
    QCheckBox* busMute_ {};
    QSlider* busBalance_ {};
    QSlider* busLimiter_ {};
    EqGraphWidget* busEqGraph_ {};
    QVector<QSlider*> busEqSliders_;
    QLabel* busLevelLabel_ {};
    QLabel* busBalanceLabel_ {};
    QLabel* busLimiterLabel_ {};
    QLabel* busOutputDelay_ {};

    int selectedFx_ {};
    QLabel* fxTitle_ {};
    MeterWidget* fxMeter_ {};
    QSlider* fxMaster_ {};
    QComboBox* fxPreset_ {};
    QLabel* fxType_ {};
    QCheckBox* fxMute_ {};
    QLabel* fxTempo_ {};
    QPushButton* fxTap_ {};
    QVector<QSlider*> fxParameters_;
    QVector<QCheckBox*> fxReturnChecks_;
    QVector<QLabel*> fxFormLabels_;
    QLabel* fxInfo_ {};

    QListWidget* hardwareSnapshots_ {};
    QLabel* appLibrary_ {};
    QListWidget* appSnapshots_ {};
    QLineEdit* appSnapshotName_ {};
    QComboBox* appSnapshotScope_ {};
    QPushButton* storeSnapshot_ {};
    QPushButton* loadAppSnapshot_ {};
    QPushButton* renameAppSnapshot_ {};
    QPushButton* deleteAppSnapshot_ {};
    QPushButton* shareSnapshot_ {};
    QLabel* hardwareTitle_ {};
    QLabel* libraryTitle_ {};
    QPushButton* recallButton_ {};
    QPushButton* hardwareStore_ {};
    QPushButton* hardwareRename_ {};
    QPushButton* hardwareDelete_ {};
    QPushButton* hardwareReset_ {};

    QWidget* routingGrid_ {};
    QLabel* routingTitle_ {};
    QVector<QLabel*> routingDestinationLabels_;
    QVector<QLabel*> routingInputLabels_;
    QVector<QCheckBox*> routingChecks_;
    QLabel* advancedRoutingTitle_ {};
    QComboBox* usbMode_ {};
    QVector<QCheckBox*> usbRouteChecks_;
    QVector<QCheckBox*> fxOutputChecks_;
    QComboBox* headphoneSource_ {};
    QCheckBox* monitorStereoLink_ {};
    QLabel* routingEvidence_ {};
};

} // namespace flow8::ui
