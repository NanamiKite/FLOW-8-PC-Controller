#include "core/flow8_device.h"

#include "protocol/codec.h"
#include "protocol/flow8_protocol.h"
#include "protocol/packet.h"

#include <QStringList>

namespace flow8 {
namespace {

constexpr auto simulatorSource = "SYNTHETIC simulator state; not hardware evidence";

model::StateValue<double> simulatorDouble(const double value)
{
    return model::StateValue<double>::known(value, model::EvidenceStatus::Unknown,
                                            QString::fromLatin1(simulatorSource));
}

model::StateValue<bool> simulatorBool(const bool value)
{
    return model::StateValue<bool>::known(value, model::EvidenceStatus::Unknown,
                                          QString::fromLatin1(simulatorSource));
}

} // namespace

Flow8Device::Flow8Device(QObject* parent)
    : QObject(parent)
    , state_()
{
}

Flow8Device::~Flow8Device() = default;

Flow8State& Flow8Device::state() noexcept
{
    return state_;
}

const Flow8State& Flow8Device::state() const noexcept
{
    return state_;
}

Flow8Transport* Flow8Device::transport() const noexcept
{
    return transport_.get();
}

void Flow8Device::setTransport(std::unique_ptr<Flow8Transport> transport)
{
    if (transport_) {
        transport_->disconnectTransport();
        transport_->disconnect(this);
    }
    transport_ = std::move(transport);
    state_.setConnectionState(ConnectionState::Disconnected);

    if (transport_) {
        connect(transport_.get(), &Flow8Transport::stateChanged, this,
                &Flow8Device::handleTransportState);
        connect(transport_.get(), &Flow8Transport::bytesReceived, this,
                &Flow8Device::handleBytesReceived);
        connect(transport_.get(), &Flow8Transport::errorOccurred, this, [this](const QString&) {
            state_.setConnectionState(ConnectionState::Error);
        });
    }
    emit transportChanged();
}

void Flow8Device::connectDevice()
{
    if (transport_) {
        transport_->connectTransport();
    }
}

void Flow8Device::disconnectDevice()
{
    if (transport_) {
        transport_->disconnectTransport();
    }
}

bool Flow8Device::isControlAvailable(const Control control) const noexcept
{
    if (!simulatorReady()) {
        // Real hardware controls remain disabled until mappings are project-verified.
        return false;
    }

    switch (control) {
    case Control::ChannelFader:
    case Control::ChannelGain:
    case Control::ChannelMute:
    case Control::ChannelSolo:
    case Control::ChannelPan:
    case Control::MainFader:
    case Control::MainMute:
        return true;
    }
    return false;
}

bool Flow8Device::setChannelFader(const int index, const double normalized)
{
    if (!isControlAvailable(Control::ChannelFader) || index < 0 || index >= state_.channels().size()) {
        reject(Control::ChannelFader, QStringLiteral("channel fader is unavailable"));
        return false;
    }
    const auto packet = protocol::encodeFaderLevel(static_cast<quint8>(index + 1), normalized);
    if (!packet.has_value() || !transport_->send(*packet)) {
        reject(Control::ChannelFader, QStringLiteral("failed to encode or send fader value"));
        return false;
    }
    return true;
}

bool Flow8Device::setChannelGain(const int index, const double normalized)
{
    if (!isControlAvailable(Control::ChannelGain)) {
        reject(Control::ChannelGain, QStringLiteral("gain mapping is not hardware-verified"));
        return false;
    }
    return state_.setChannelGain(index, normalized, model::EvidenceStatus::Unknown,
                                 QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelMuted(const int index, const bool muted)
{
    if (!isControlAvailable(Control::ChannelMute)) {
        reject(Control::ChannelMute, QStringLiteral("mute mapping is not hardware-verified"));
        return false;
    }
    return state_.setChannelMuted(index, muted, model::EvidenceStatus::Unknown,
                                  QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelSoloed(const int index, const bool soloed)
{
    if (!isControlAvailable(Control::ChannelSolo)) {
        reject(Control::ChannelSolo, QStringLiteral("solo mapping is not hardware-verified"));
        return false;
    }
    return state_.setChannelSoloed(index, soloed, model::EvidenceStatus::Unknown,
                                   QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelPan(const int index, const double pan)
{
    if (!isControlAvailable(Control::ChannelPan)) {
        reject(Control::ChannelPan, QStringLiteral("pan mapping is not hardware-verified"));
        return false;
    }
    return state_.setChannelPan(index, pan, model::EvidenceStatus::Unknown,
                                QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setMainFader(const double normalized)
{
    if (!isControlAvailable(Control::MainFader)) {
        reject(Control::MainFader, QStringLiteral("main fader BLE address is UNKNOWN"));
        return false;
    }
    return state_.setMainFader(normalized, model::EvidenceStatus::Unknown,
                               QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setMainMuted(const bool muted)
{
    if (!isControlAvailable(Control::MainMute)) {
        reject(Control::MainMute, QStringLiteral("main mute mapping is UNKNOWN"));
        return false;
    }
    return state_.setMainMuted(muted, model::EvidenceStatus::Unknown,
                               QString::fromLatin1(simulatorSource));
}

void Flow8Device::handleTransportState(const Flow8Transport::State state)
{
    switch (state) {
    case Flow8Transport::State::Disconnected:
        state_.setConnectionState(ConnectionState::Disconnected);
        break;
    case Flow8Transport::State::Scanning:
        state_.setConnectionState(ConnectionState::Scanning);
        break;
    case Flow8Transport::State::Connecting:
    case Flow8Transport::State::Reconnecting:
        state_.setConnectionState(ConnectionState::Connecting);
        break;
    case Flow8Transport::State::DiscoveringServices:
        state_.setConnectionState(ConnectionState::Synchronizing);
        break;
    case Flow8Transport::State::Connected:
        state_.setConnectionState(ConnectionState::Connected);
        if (transport_ && transport_->isSimulator()) {
            initializeSimulatorProfile();
            state_.setConnectionState(ConnectionState::Ready);
        }
        break;
    case Flow8Transport::State::Error:
        state_.setConnectionState(ConnectionState::Error);
        break;
    }
}

void Flow8Device::handleBytesReceived(const QByteArray& payload)
{
    const auto result = protocol::parsePacket(payload);
    if (!result.ok()) {
        return;
    }
    const auto& packet = *result.packet;
    emit protocolPacketObserved(packet.type, packet.raw);

    const auto change = protocol::decodeParameterChange(packet);
    if (!change.has_value() || change->parameter != protocol::faderLevelParameter
        || change->channel == 0) {
        return;
    }
    const int index = static_cast<int>(change->channel) - 1;
    (void)state_.setChannelFader(index, protocol::decodeUnitInterval(change->value),
                                 model::EvidenceStatus::Inferred,
                                 QStringLiteral("reference 0x06/0x0f mapping; NEED_HARDWARE"));
}

void Flow8Device::initializeSimulatorProfile()
{
    const QStringList names {
        QStringLiteral("Channel 1"), QStringLiteral("Channel 2"),
        QStringLiteral("Channel 3"), QStringLiteral("Channel 4"),
        QStringLiteral("Channel 5/6"), QStringLiteral("Channel 7/8"),
        QStringLiteral("USB/BT"),
    };

    QVector<model::ChannelState> channels;
    channels.reserve(names.size());
    for (int index = 0; index < names.size(); ++index) {
        model::ChannelState channel;
        channel.index = index;
        channel.name = model::StateValue<QString>::known(
            names.at(index), model::EvidenceStatus::Unknown, QString::fromLatin1(simulatorSource));
        channel.gain = simulatorDouble(0.5);
        channel.fader = simulatorDouble(0.65);
        channel.muted = simulatorBool(false);
        channel.soloed = simulatorBool(false);
        channel.pan = simulatorDouble(0.0);
        channels.append(std::move(channel));
    }
    state_.replaceChannels(std::move(channels));
    (void)state_.setMainFader(0.75, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
    (void)state_.setMainMuted(false, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::simulatorReady() const noexcept
{
    return transport_ && transport_->isSimulator()
        && transport_->state() == Flow8Transport::State::Connected
        && state_.connectionState() == ConnectionState::Ready;
}

void Flow8Device::reject(const Control control, const QString& reason)
{
    emit controlRejected(control, reason);
}

} // namespace flow8
