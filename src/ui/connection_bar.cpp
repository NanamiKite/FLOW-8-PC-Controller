#include "ui/connection_bar.h"

#include "ui/ui_text.h"

#include <QComboBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>

namespace flow8::ui {
namespace {

QString stateName(const ConnectionState state)
{
    switch (state) {
    case ConnectionState::Disconnected: return uiText("Disconnected");
    case ConnectionState::Scanning: return uiText("Scanning");
    case ConnectionState::Connecting: return uiText("Connecting");
    case ConnectionState::Authenticating: return uiText("Authenticating");
    case ConnectionState::Synchronizing: return uiText("Synchronizing");
    case ConnectionState::Connected: return uiText("Connected");
    case ConnectionState::Ready: return uiText("Ready (Simulator)");
    case ConnectionState::Error: return uiText("Error");
    }
    return uiText("Unknown");
}

} // namespace

ConnectionBar::ConnectionBar(QWidget* parent)
    : QWidget(parent)
    , transportSelector_(new QComboBox(this))
    , connectButton_(new QPushButton(this))
    , settingsButton_(new QPushButton(this))
    , statusLabel_(new QLabel(this))
    , subtitle_(new QLabel(this))
{
    setObjectName(QStringLiteral("connectionBar"));
    transportSelector_->setObjectName(QStringLiteral("transportSelector"));
    connectButton_->setObjectName(QStringLiteral("connectButton"));
    settingsButton_->setObjectName(QStringLiteral("settingsButton"));
    statusLabel_->setObjectName(QStringLiteral("connectionStatus"));

    auto* brand = new QLabel(QStringLiteral("FLOW 8"), this);
    brand->setProperty("class", QStringLiteral("brandTitle"));
    subtitle_->setProperty("class", QStringLiteral("brandSubtitle"));

    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(18, 10, 18, 10);
    layout->setSpacing(10);
    layout->addWidget(brand);
    layout->addWidget(subtitle_);
    layout->addStretch();
    layout->addWidget(transportSelector_);
    layout->addWidget(connectButton_);
    layout->addWidget(statusLabel_);
    layout->addSpacing(8);
    layout->addWidget(settingsButton_);

    connect(connectButton_, &QPushButton::clicked, this, [this] {
        if (state_ == ConnectionState::Disconnected || state_ == ConnectionState::Error) {
            emit connectRequested();
        } else {
            emit disconnectRequested();
        }
    });
    connect(settingsButton_, &QPushButton::clicked, this, &ConnectionBar::settingsRequested);
    retranslateUi();
}

void ConnectionBar::setConnectionState(const ConnectionState state)
{
    state_ = state;
    statusLabel_->setText(stateName(state));
    const bool disconnected = state == ConnectionState::Disconnected || state == ConnectionState::Error;
    connectButton_->setText(disconnected ? uiText("Connect") : uiText("Disconnect"));
    const bool transition = state == ConnectionState::Connecting || state == ConnectionState::Scanning
        || state == ConnectionState::Authenticating || state == ConnectionState::Synchronizing;
    transportSelector_->setEnabled(disconnected);
    connectButton_->setEnabled(!transition);
}

void ConnectionBar::retranslateUi()
{
    subtitle_->setText(uiText("PC Controller"));
    const int selection = transportSelector_->currentIndex();
    transportSelector_->clear();
    transportSelector_->addItem(uiText("Simulator (SYNTHETIC)"));
    transportSelector_->setCurrentIndex(selection < 0 ? 0 : selection);
    transportSelector_->setToolTip(uiText(
        "The simulator is deterministic test data and is not a FLOW 8 hardware claim."));
    settingsButton_->setText(uiText("Preferences"));
    setConnectionState(state_);
}

} // namespace flow8::ui
