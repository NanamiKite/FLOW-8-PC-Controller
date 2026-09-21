#include "ui/main_window.h"

#include "core/flow8_device.h"
#include "ui/channel/channel_edit_view.h"
#include "ui/connection_bar.h"
#include "ui/language_manager.h"
#include "ui/layers/layer_views.h"
#include "ui/mixer/mixer_widget.h"
#include "ui/session/session_start_view.h"
#include "ui/setup/assisted_setup_wizard.h"
#include "ui/setup/setup_window.h"
#include "ui/stage/stage_view.h"
#include "ui/ui_text.h"

#include <QEvent>
#include <QKeyEvent>
#include <QList>
#include <QStatusBar>
#include <QStackedWidget>
#include <QTimer>
#include <QVBoxLayout>

namespace flow8::ui {
namespace {

QString applicationStyle()
{
    return QStringLiteral(R"STYLE(
        QMainWindow, QWidget {
            background: #111317;
            color: #e6e9ee;
            font-family: "Segoe UI", "Microsoft YaHei UI", "Noto Sans CJK SC", "Noto Sans", sans-serif;
            font-size: 12px;
        }
        #connectionBar { background: #181b20; border-bottom: 1px solid #2b2f36; }
        #flowLayerBar { background: #14171b; border-bottom: 1px solid #2a2e34; }
        QLabel[class="brandTitle"] { font-size: 22px; font-weight: 700; letter-spacing: 1px; }
        QLabel[class="brandSubtitle"] { color: #7f8793; font-size: 10px; letter-spacing: 2px; }
        QLabel[class="connectionState"] { color: #6aa8ff; font-weight: 650; }
        QLabel[class="channelName"], QLabel[class="inspectorTitle"] { font-size: 14px; font-weight: 700; }
        QLabel[class="stageName"] { font-size: 17px; font-weight: 700; }
        QLabel[class="sessionTitle"] { font-size: 25px; font-weight: 720; }
        QLabel[class="sectionLabel"] { color: #bfc5cf; font-size: 11px; font-weight: 700; letter-spacing: 1.5px; }
        QLabel[class="secondaryText"] { color: #8e96a2; }
        QLabel[class="parameterValue"] {
            color: #eef1f5; background: #22262c; border: 1px solid #343943;
            border-radius: 5px; padding: 4px 7px; font-weight: 650;
        }
        QLabel[class="inputBadge"], QLabel[class="syntheticBadge"] {
            color: #9bc3ff; background: #1b2940; border: 1px solid #29496f;
            border-radius: 5px; padding: 3px 7px; font-size: 9px; font-weight: 650;
        }
        QLabel[class="stripIndicator"] {
            color: #a8afb9; background: #252930; border-radius: 4px; padding: 3px 4px; font-size: 8px;
        }
        QLabel[class="phantomIndicator"] {
            color: #8d5b60; background: #25191b; border: 1px solid #583037;
            border-radius: 4px; padding: 3px 5px; font-size: 8px; font-weight: 750;
        }
        QLabel[class="phantomIndicator"][active="true"] {
            color: #fff7f7; background: #d74752; border-color: #ff7d86;
        }
        QToolButton[class="layerButton"] {
            border: 0; border-bottom: 2px solid transparent; border-radius: 5px;
            min-width: 68px; padding: 8px 12px; color: #8f97a2; font-weight: 650;
        }
        QToolButton[class="layerButton"]:hover { background: #22262c; color: #eef1f5; }
        QToolButton[class="layerButton"][layerRole="mix"]:checked { color: #f2c94c; border-bottom-color: #f2c94c; background: #292519; }
        QToolButton[class="layerButton"][layerRole="fx"]:checked { color: #c9a3ff; border-bottom-color: #9b6ad6; background: #241d2d; }
        QToolButton[class="layerButton"][layerRole="monitor"]:checked { color: #83d9a0; border-bottom-color: #55b979; background: #19271f; }
        QToolButton[class="layerButton"][layerRole="main"]:checked { color: #f1f3f6; border-bottom-color: #d7dce3; background: #25282d; }
        QToolButton[class="destinationButton"] {
            border: 1px solid #30353d; border-radius: 6px; min-width: 64px;
            padding: 7px 12px; color: #9aa2ad; background: #1a1d22; font-weight: 650;
        }
        QToolButton[class="destinationButton"]:hover { border-color: #59616d; color: #eef1f5; }
        QToolButton[class="destinationButton"]:checked {
            color: #18191b; background: #e1b941; border-color: #f2cd5c;
        }
        #stripsContainer { background: #101216; }
        QWidget[class="channelStrip"], QWidget[class="mainStrip"] {
            background: #181b20; border: 1px solid #2a2e34; border-radius: 3px;
        }
        QWidget[class="mixSendStrip"] {
            background: #181b20; border: 1px solid #2a2e34; border-radius: 3px;
        }
        QWidget[class="mixSendStrip"]:hover { border-color: #454b55; background: #1b1e23; }
        QWidget[class="channelStrip"][selected="true"] { border-color: #e1b941; background: #1d1d1a; }
        QWidget[class="detailCard"], QWidget[class="stageCard"], QWidget[class="inputCard"] {
            background: #1a1d22; border: 1px solid #2c3037; border-radius: 9px;
        }
        QWidget[class="inspector"] { background: #171a1f; border: 1px solid #2b2f36; border-radius: 9px; }
        QWidget[class="inspectorPage"] { background: #171a1f; }
        QPushButton[class="sessionAction"] {
            text-align: left; font-size: 16px; font-weight: 650;
            padding: 18px; border-radius: 10px; background: #1b1f25;
        }
        QPushButton, QComboBox, QToolButton, QLineEdit {
            background: #252930; color: #dfe3e9; border: 1px solid #353a43;
            border-radius: 6px; padding: 6px 10px;
        }
        QPushButton:hover, QComboBox:hover, QToolButton:hover, QLineEdit:hover { border-color: #626c7a; }
        QPushButton:pressed, QToolButton:checked { background: #345f96; border-color: #5a96df; }
        QToolButton[class="muteButton"]:checked { background: #a93f45; border-color: #e2676e; }
        QToolButton[class="soloButton"]:checked { background: #8c6b1d; border-color: #d7aa35; }
        QCheckBox { color: #e6e9ee; spacing: 8px; }
        QCheckBox::indicator {
            width: 17px; height: 17px; border: 2px solid #7b8594;
            border-radius: 4px; background: #0d0f12;
        }
        QCheckBox::indicator:hover { border-color: #c5ccd6; background: #181c22; }
        QCheckBox::indicator:checked { background: #4f92e8; border-color: #9ac7ff; }
        QCheckBox::indicator:checked:hover { background: #63a2f2; border-color: #c0dcff; }
        QCheckBox::indicator:disabled { background: #17191d; border-color: #3b414a; }
        QCheckBox[class="phantomControl"]::indicator:checked {
            background: #d74752; border-color: #ff8b93;
        }
        QSlider::groove:horizontal { height: 4px; background: #343943; border-radius: 2px; }
        QSlider::sub-page:horizontal { background: #e1b941; border-radius: 2px; }
        QSlider::handle:horizontal { width: 14px; margin: -5px 0; background: #d7dce3; border-radius: 7px; }
        QSlider::groove:vertical { width: 4px; background: #343943; border-radius: 2px; }
        QSlider::handle:vertical { height: 13px; margin: 0 -5px; background: #d7dce3; border-radius: 6px; }
        QTabWidget::pane { border: 0; background: #171a1f; }
        QTabBar::tab { border: 0; color: #848d99; padding: 8px 13px; }
        QTabBar::tab:selected { color: #e6eaf0; border-bottom: 2px solid #e1b941; }
        QScrollArea, QListWidget { border: 0; background: transparent; }
        QListWidget::item { padding: 9px; border-radius: 5px; }
        QListWidget::item:selected { background: #2b3038; color: #f1f3f6; }
        QSplitter::handle { background: #292d34; width: 2px; height: 2px; }
        QStatusBar { background: #14171b; color: #7f8793; border-top: 1px solid #292d34; }
        *:disabled { color: #59606a; }
    )STYLE");
}

} // namespace

MainWindow::MainWindow(Flow8Device& device, LanguageManager& languageManager, QWidget* parent)
    : QMainWindow(parent)
    , device_(device)
    , languageManager_(languageManager)
    , connectionBar_(new ConnectionBar(this))
    , layerBar_(new FlowLayerBar(this))
    , mixer_(new MixerWidget(device_, this))
    , stage_(new StageView(device_, this))
    , mainOut_(new MainOutView(device_, this))
    , channelEdit_(new ChannelEditView(device_, this))
    , setup_(new SetupWindow(device_, languageManager_, this))
    , sessionStart_(new SessionStartView(this))
    , workspace_(new QStackedWidget(this))
{
    setObjectName(QStringLiteral("mainWindow"));
    resize(1440, 920);
    setMinimumSize(1040, 700);
    setStyleSheet(applicationStyle());

    auto* central = new QWidget(this);
    auto* layout = new QVBoxLayout(central);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(connectionBar_);
    layout->addWidget(layerBar_);
    const QList<QWidget*> pages {
        sessionStart_, mixer_, stage_, mainOut_, channelEdit_, setup_,
    };
    for (auto* page : pages) {
        workspace_->addWidget(page);
    }
    workspace_->setCurrentWidget(sessionStart_);
    layout->addWidget(workspace_, 1);
    setCentralWidget(central);

    layerBar_->setEnabled(false);
    connect(layerBar_, &FlowLayerBar::layerSelected, this, &MainWindow::showLayer);
    connect(connectionBar_, &ConnectionBar::connectRequested,
            this, [this] { device_.connectDevice(); });
    connect(connectionBar_, &ConnectionBar::disconnectRequested,
            this, [this] { device_.disconnectDevice(); });
    connect(connectionBar_, &ConnectionBar::setupRequested,
            this, [this] { showSetup(static_cast<int>(SetupSection::ConfigureInputs)); });
    connect(connectionBar_, &ConnectionBar::preferencesRequested,
            this, [this] { showSetup(static_cast<int>(SetupSection::Preferences)); });
    connect(mixer_, &MixerWidget::channelEditRequested,
            this, &MainWindow::showChannelEdit);
    connect(mixer_, &MixerWidget::mainOutRequested,
            this, [this] { showLayer(FlowLayer::MainOut); });
    connect(mixer_, &MixerWidget::destinationChanged,
            stage_, &StageView::setDestination);
    connect(channelEdit_, &ChannelEditView::backRequested,
            this, [this] { showLayer(FlowLayer::Mixer); });
    connect(setup_, &SetupWindow::backRequested,
            this, [this] { showLayer(layerBar_->currentLayer()); });
    connect(setup_, &SetupWindow::inputEditRequested,
            this, &MainWindow::showChannelEdit);
    connect(&device_.state(), &Flow8State::connectionStateChanged,
            connectionBar_, &ConnectionBar::setConnectionState);
    connect(&device_.state(), &Flow8State::connectionStateChanged, this,
            [this](const ConnectionState state) {
                const bool ready = state == ConnectionState::Ready;
                layerBar_->setEnabled(ready);
                if (!ready) {
                    return;
                }
                if (pendingSnapshots_) {
                    pendingSnapshots_ = false;
                    showSetup(static_cast<int>(SetupSection::SnapshotLibrary));
                } else {
                    showLayer(FlowLayer::Mixer);
                }
                if (pendingAssistedSetup_) {
                    pendingAssistedSetup_ = false;
                    QTimer::singleShot(0, this, [this] {
                        AssistedSetupWizard wizard(device_, this);
                        wizard.exec();
                    });
                }
            });

    const auto connectThen = [this](const FlowLayer layer) {
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            showLayer(layer);
        }
    };
    connect(sessionStart_, &SessionStartView::startNewRequested,
            this, [connectThen] { connectThen(FlowLayer::Mixer); });
    connect(sessionStart_, &SessionStartView::continueSessionRequested,
            this, [connectThen] { connectThen(FlowLayer::Mixer); });
    connect(sessionStart_, &SessionStartView::loadSnapshotRequested, this, [this] {
        pendingSnapshots_ = true;
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            pendingSnapshots_ = false;
            showSetup(static_cast<int>(SetupSection::SnapshotLibrary));
        }
    });
    connect(sessionStart_, &SessionStartView::assistedSetupRequested, this, [this] {
        pendingAssistedSetup_ = true;
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            pendingAssistedSetup_ = false;
            showLayer(FlowLayer::Mixer);
            QTimer::singleShot(0, this, [this] {
                AssistedSetupWizard wizard(device_, this);
                wizard.exec();
            });
        }
    });
    connect(&device_, &Flow8Device::controlRejected, this,
            [this](Flow8Device::Control, const QString& reason) {
                statusBar()->showMessage(reason, 4000);
            });
    connectionBar_->setConnectionState(device_.state().connectionState());
    retranslateUi();
}

void MainWindow::showLayer(const FlowLayer layer)
{
    if (device_.state().connectionState() != ConnectionState::Ready) {
        return;
    }
    layerBar_->setCurrentLayer(layer);
    switch (layer) {
    case FlowLayer::Mixer:
        mixer_->setDestination(model::RoutingDestination::Main);
        mixer_->refreshAll();
        workspace_->setCurrentWidget(mixer_);
        break;
    case FlowLayer::Stage:
        stage_->refresh();
        workspace_->setCurrentWidget(stage_);
        break;
    case FlowLayer::Fx1:
        mixer_->setDestination(model::RoutingDestination::Fx1);
        mixer_->refreshAll();
        workspace_->setCurrentWidget(mixer_);
        break;
    case FlowLayer::Fx2:
        mixer_->setDestination(model::RoutingDestination::Fx2);
        mixer_->refreshAll();
        workspace_->setCurrentWidget(mixer_);
        break;
    case FlowLayer::Monitor1:
        mixer_->setDestination(model::RoutingDestination::Monitor1);
        mixer_->refreshAll();
        workspace_->setCurrentWidget(mixer_);
        break;
    case FlowLayer::Monitor2:
        mixer_->setDestination(model::RoutingDestination::Monitor2);
        mixer_->refreshAll();
        workspace_->setCurrentWidget(mixer_);
        break;
    case FlowLayer::Main:
        mixer_->setDestination(model::RoutingDestination::Main);
        mixer_->refreshAll();
        workspace_->setCurrentWidget(mixer_);
        break;
    case FlowLayer::MainOut:
        mainOut_->refresh();
        workspace_->setCurrentWidget(mainOut_);
        break;
    }
}

void MainWindow::showChannelEdit(const int channelIndex)
{
    if (device_.state().connectionState() != ConnectionState::Ready) {
        return;
    }
    channelEdit_->setChannel(channelIndex);
    workspace_->setCurrentWidget(channelEdit_);
}

void MainWindow::showSetup(const int section)
{
    if (device_.state().connectionState() != ConnectionState::Ready) {
        statusBar()->showMessage(uiText("Connect to Simulator before opening Setup."), 3000);
        return;
    }
    setup_->selectSection(static_cast<SetupSection>(section));
    setup_->refresh();
    workspace_->setCurrentWidget(setup_);
}

void MainWindow::changeEvent(QEvent* event)
{
    QMainWindow::changeEvent(event);
    if (event->type() == QEvent::LanguageChange) {
        retranslateUi();
    }
}

void MainWindow::keyPressEvent(QKeyEvent* event)
{
    if (event->key() == Qt::Key_Escape) {
        if (workspace_->currentWidget() == channelEdit_) {
            showLayer(FlowLayer::Mixer);
            event->accept();
            return;
        }
        if (workspace_->currentWidget() == setup_) {
            showLayer(layerBar_->currentLayer());
            event->accept();
            return;
        }
    }
    QMainWindow::keyPressEvent(event);
}

void MainWindow::retranslateUi()
{
    setWindowTitle(uiText("FLOW 8 PC Controller — Simulator"));
    statusBar()->showMessage(uiText(
        "Simulator mode · Synthetic mixer data · No FLOW 8 hardware connected"));
    connectionBar_->retranslateUi();
    layerBar_->retranslateUi();
    mixer_->retranslateUi();
    stage_->retranslateUi();
    mainOut_->retranslateUi();
    channelEdit_->retranslateUi();
    setup_->retranslateUi();
    sessionStart_->retranslateUi();
}

} // namespace flow8::ui
