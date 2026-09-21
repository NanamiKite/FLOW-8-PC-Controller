#include "ui/main_window.h"

#include "core/flow8_device.h"
#include "ui/connection_bar.h"
#include "ui/mixer/mixer_widget.h"

#include <QLabel>
#include <QStatusBar>
#include <QTabWidget>
#include <QVBoxLayout>

namespace flow8::ui {
namespace {

QWidget* placeholder(const QString& title, QWidget* parent)
{
    auto* page = new QWidget(parent);
    auto* layout = new QVBoxLayout(page);
    auto* label = new QLabel(
        QStringLiteral("%1\nProtocol fields remain UNKNOWN or INFERRED; controls are not exposed yet.")
            .arg(title),
        page);
    label->setAlignment(Qt::AlignCenter);
    layout->addWidget(label);
    return page;
}

} // namespace

MainWindow::MainWindow(Flow8Device& device, QWidget* parent)
    : QMainWindow(parent)
    , device_(device)
    , connectionBar_(new ConnectionBar(this))
{
    setObjectName(QStringLiteral("mainWindow"));
    setWindowTitle(QStringLiteral("FLOW 8 PC Controller — Simulator Development"));
    resize(1120, 720);

    auto* central = new QWidget(this);
    auto* layout = new QVBoxLayout(central);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(connectionBar_);
    auto* tabs = new QTabWidget(central);
    tabs->setObjectName(QStringLiteral("mainTabs"));
    tabs->addTab(new MixerWidget(device_, tabs), QStringLiteral("Mixer"));
    for (const auto& page : {QStringLiteral("EQ"), QStringLiteral("Compressor"),
                             QStringLiteral("Sends"), QStringLiteral("FX"),
                             QStringLiteral("Snapshots"), QStringLiteral("Settings")}) {
        tabs->addTab(placeholder(page, tabs), page);
    }
    layout->addWidget(tabs);
    setCentralWidget(central);
    statusBar()->showMessage(QStringLiteral(
        "Simulator data is SYNTHETIC. Real FLOW 8 verification: BLOCKED: NEED_HARDWARE"));

    connect(connectionBar_, &ConnectionBar::connectRequested, this,
            [this] { device_.connectDevice(); });
    connect(connectionBar_, &ConnectionBar::disconnectRequested, this,
            [this] { device_.disconnectDevice(); });
    connect(&device_.state(), &Flow8State::connectionStateChanged, connectionBar_,
            &ConnectionBar::setConnectionState);
    connect(&device_, &Flow8Device::controlRejected, this,
            [this](Flow8Device::Control, const QString& reason) {
                statusBar()->showMessage(reason, 4000);
            });
    connectionBar_->setConnectionState(device_.state().connectionState());
}

} // namespace flow8::ui
