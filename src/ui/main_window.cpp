#include "ui/main_window.h"

#include "core/flow8_device.h"
#include "ui/connection_bar.h"
#include "ui/language_manager.h"
#include "ui/mixer/mixer_widget.h"
#include "ui/session/session_start_view.h"
#include "ui/settings/settings_dialog.h"
#include "ui/setup/assisted_setup_wizard.h"
#include "ui/ui_text.h"

#include <QEvent>
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
            color: #e5e8ed;
            font-family: "Inter", "Segoe UI", "Microsoft YaHei UI", "Noto Sans CJK SC", "Noto Sans", sans-serif;
            font-size: 12px;
        }
        #connectionBar {
            background: #171a1f;
            border-bottom: 1px solid #2b2f36;
        }
        QLabel[class="brandTitle"] { font-size: 22px; font-weight: 700; letter-spacing: 1px; }
        QLabel[class="brandSubtitle"] { color: #7f8793; font-size: 10px; letter-spacing: 2px; }
        QLabel[class="channelName"], QLabel[class="inspectorTitle"] {
            font-size: 14px; font-weight: 700; letter-spacing: .4px;
        }
        QLabel[class="stageName"] { font-size: 17px; font-weight: 700; }
        QLabel[class="sessionTitle"] { font-size: 30px; font-weight: 750; }
        QLabel[class="secondaryText"] { color: #89919d; }
        QLabel[class="inputBadge"], QLabel[class="syntheticBadge"] {
            color: #9bc3ff; background: #1d2d45; border: 1px solid #294e7e;
            border-radius: 6px; padding: 3px 7px; font-size: 9px; font-weight: 700;
        }
        QLabel[class="stripIndicator"] {
            color: #9ba3af; background: #252930; border-radius: 5px;
            padding: 3px 4px; font-size: 8px;
        }
        #mixerNavigation { background: #14171b; border-bottom: 1px solid #282c33; }
        QLabel[class="navigationTitle"] { font-size: 12px; font-weight: 700; color: #aeb5c0; }
        QToolButton[class="navigationButton"] {
            border: 0; border-radius: 7px; padding: 7px 12px; color: #959da8;
        }
        QToolButton[class="navigationButton"]:hover { background: #22262c; color: #e7eaf0; }
        QToolButton[class="navigationButton"]:checked { background: #2c5f9f; color: white; }
        #stripsContainer { background: #101216; }
        QWidget[class="channelStrip"], QWidget[class="mainStrip"], QWidget[class="detailCard"],
        QWidget[class="stageCard"] {
            background: #1a1d22; border: 1px solid #2a2e35; border-radius: 11px;
        }
        QPushButton[class="sessionAction"] {
            text-align: left; font-size: 16px; font-weight: 650;
            padding: 18px; border-radius: 12px; background: #1b1f25;
        }
        QWidget[class="channelStrip"][selected="true"] {
            border: 1px solid #5097ff; background: #1b2028;
        }
        QWidget[class="inspector"] { background: #171a1f; border-top: 1px solid #2b2f36; }
        QWidget[class="inspectorPage"] { background: #171a1f; }
        QPushButton, QComboBox, QToolButton {
            background: #252930; color: #dfe3e9; border: 1px solid #343943;
            border-radius: 7px; padding: 6px 11px;
        }
        QPushButton:hover, QComboBox:hover, QToolButton:hover { border-color: #56606e; }
        QPushButton:pressed, QToolButton:checked { background: #315f99; border-color: #5294e8; }
        QToolButton[class="muteButton"]:checked { background: #ad3f45; border-color: #e2676e; }
        QToolButton[class="soloButton"]:checked { background: #94721e; border-color: #d7aa35; }
        QSlider::groove:horizontal { height: 4px; background: #343943; border-radius: 2px; }
        QSlider::sub-page:horizontal { background: #5097ff; border-radius: 2px; }
        QSlider::handle:horizontal { width: 14px; margin: -5px 0; background: #d7dce3; border-radius: 7px; }
        QSlider::groove:vertical { width: 4px; background: #343943; border-radius: 2px; }
        QSlider::handle:vertical { height: 13px; margin: 0 -5px; background: #d7dce3; border-radius: 6px; }
        QTabWidget::pane { border: 0; background: #171a1f; }
        QTabBar::tab { border: 0; color: #848d99; padding: 7px 13px; }
        QTabBar::tab:selected { color: #e6eaf0; border-bottom: 2px solid #5097ff; }
        QScrollArea, QListWidget { border: 0; background: transparent; }
        QListWidget::item { padding: 6px; border-radius: 5px; }
        QListWidget::item:selected { background: #2c5f9f; }
        QSplitter::handle { background: #292d34; height: 2px; }
        QStatusBar { background: #14171b; color: #7f8793; border-top: 1px solid #292d34; }
        *:disabled { color: #555c66; }
    )STYLE");
}

} // namespace

MainWindow::MainWindow(Flow8Device& device, LanguageManager& languageManager, QWidget* parent)
    : QMainWindow(parent)
    , device_(device)
    , languageManager_(languageManager)
    , connectionBar_(new ConnectionBar(this))
    , mixer_(new MixerWidget(device_, this))
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
    workspace_->addWidget(sessionStart_);
    workspace_->addWidget(mixer_);
    workspace_->setCurrentWidget(sessionStart_);
    layout->addWidget(workspace_, 1);
    setCentralWidget(central);
    retranslateUi();

    connect(connectionBar_, &ConnectionBar::connectRequested, this,
            [this] { device_.connectDevice(); });
    connect(connectionBar_, &ConnectionBar::disconnectRequested, this,
            [this] { device_.disconnectDevice(); });
    connect(connectionBar_, &ConnectionBar::settingsRequested, this,
            [this] {
                SettingsDialog dialog(device_, languageManager_, this);
                dialog.exec();
            });
    connect(&device_.state(), &Flow8State::connectionStateChanged, connectionBar_,
            &ConnectionBar::setConnectionState);
    connect(&device_.state(), &Flow8State::connectionStateChanged, this,
            [this](const ConnectionState state) {
                if (state != ConnectionState::Ready) {
                    return;
                }
                workspace_->setCurrentWidget(mixer_);
                if (pendingSnapshots_) {
                    pendingSnapshots_ = false;
                    mixer_->showSnapshots();
                } else {
                    mixer_->showMixer();
                }
                if (pendingAssistedSetup_) {
                    pendingAssistedSetup_ = false;
                    QTimer::singleShot(0, this, [this] {
                        AssistedSetupWizard wizard(device_, this);
                        wizard.exec();
                    });
                }
            });
    connect(sessionStart_, &SessionStartView::startNewRequested, this, [this] {
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            workspace_->setCurrentWidget(mixer_);
            mixer_->showMixer();
        }
    });
    connect(sessionStart_, &SessionStartView::continueSessionRequested, this, [this] {
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            workspace_->setCurrentWidget(mixer_);
            mixer_->showMixer();
        }
    });
    connect(sessionStart_, &SessionStartView::loadSnapshotRequested, this, [this] {
        pendingSnapshots_ = true;
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            pendingSnapshots_ = false;
            workspace_->setCurrentWidget(mixer_);
            mixer_->showSnapshots();
        }
    });
    connect(sessionStart_, &SessionStartView::assistedSetupRequested, this, [this] {
        pendingAssistedSetup_ = true;
        device_.connectDevice();
        if (device_.state().connectionState() == ConnectionState::Ready) {
            pendingAssistedSetup_ = false;
            workspace_->setCurrentWidget(mixer_);
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
}

void MainWindow::changeEvent(QEvent* event)
{
    QMainWindow::changeEvent(event);
    if (event->type() == QEvent::LanguageChange) {
        retranslateUi();
    }
}

void MainWindow::retranslateUi()
{
    setWindowTitle(uiText("FLOW 8 PC Controller — Simulator"));
    statusBar()->showMessage(uiText(
        "Simulator mode · Synthetic mixer data · No FLOW 8 hardware connected"));
    connectionBar_->retranslateUi();
    mixer_->retranslateUi();
    sessionStart_->retranslateUi();
}

} // namespace flow8::ui
