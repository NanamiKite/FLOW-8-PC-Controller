#include "ui/connection_bar.h"

#include <QComboBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>

namespace flow8::ui {
namespace {

QString stateName(const ConnectionState state)
{
    switch (state) {
    case ConnectionState::Disconnected: return QStringLiteral("Disconnected");
    case ConnectionState::Scanning: return QStringLiteral("Scanning");
    case ConnectionState::Connecting: return QStringLiteral("Connecting");
    case ConnectionState::Authenticating: return QStringLiteral("Authenticating");
    case ConnectionState::Synchronizing: return QStringLiteral("Synchronizing");
    case ConnectionState::Connected: return QStringLiteral("Connected");
    case ConnectionState::Ready: return QStringLiteral("Ready (Simulator)");
    case ConnectionState::Error: return QStringLiteral("Error");
    }
    return QStringLiteral("Unknown");
}

} // namespace

ConnectionBar::ConnectionBar(QWidget* parent)
    : QWidget(parent)
    , transportSelector_(new QComboBox(this))
    , connectButton_(new QPushButton(QStringLiteral("Connect"), this))
    , statusLabel_(new QLabel(QStringLiteral("Disconnected"), this))
{
    setObjectName(QStringLiteral("connectionBar"));
    transportSelector_->setObjectName(QStringLiteral("transportSelector"));
    transportSelector_->addItem(QStringLiteral("Simulator (SYNTHETIC)"));
    transportSelector_->setToolTip(QStringLiteral(
        "The simulator is deterministic test data and is not a FLOW 8 hardware claim."));
    connectButton_->setObjectName(QStringLiteral("connectButton"));
    statusLabel_->setObjectName(QStringLiteral("connectionStatus"));

    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(8, 6, 8, 6);
    layout->addWidget(new QLabel(QStringLiteral("Connection:"), this));
    layout->addWidget(transportSelector_);
    layout->addWidget(connectButton_);
    layout->addStretch();
    layout->addWidget(statusLabel_);

    connect(connectButton_, &QPushButton::clicked, this, [this] {
        if (state_ == ConnectionState::Disconnected || state_ == ConnectionState::Error) {
            emit connectRequested();
        } else {
            emit disconnectRequested();
        }
    });
}

void ConnectionBar::setConnectionState(const ConnectionState state)
{
    state_ = state;
    statusLabel_->setText(stateName(state));
    const bool disconnected = state == ConnectionState::Disconnected || state == ConnectionState::Error;
    connectButton_->setText(disconnected ? QStringLiteral("Connect") : QStringLiteral("Disconnect"));
    const bool transition = state == ConnectionState::Connecting || state == ConnectionState::Scanning
        || state == ConnectionState::Authenticating || state == ConnectionState::Synchronizing;
    transportSelector_->setEnabled(disconnected);
    connectButton_->setEnabled(!transition);
}

} // namespace flow8::ui
