#include "simulator/fake_transport.h"

#include "protocol/flow8_protocol.h"

#include <QTimer>

namespace flow8::simulator {

FakeTransport::FakeTransport(QObject* parent)
    : Flow8Transport(parent)
{
    remoteChangeTimer_.setInterval(3500);
    connect(&remoteChangeTimer_, &QTimer::timeout, this, &FakeTransport::emitSyntheticRemoteChange);
}

QString FakeTransport::displayName() const
{
    return QStringLiteral("Simulator (SYNTHETIC)");
}

Flow8Transport::State FakeTransport::state() const noexcept
{
    return state_;
}

bool FakeTransport::isSimulator() const noexcept
{
    return true;
}

void FakeTransport::connectTransport()
{
    if (state_ != State::Disconnected && state_ != State::Error) {
        return;
    }
    setState(State::Connecting);
    QTimer::singleShot(25, this, [this] {
        setState(State::Connected);
        if (remoteChangesEnabled_) {
            remoteChangeTimer_.start();
        }
    });
}

void FakeTransport::disconnectTransport()
{
    remoteChangeTimer_.stop();
    setState(State::Disconnected);
}

bool FakeTransport::send(const QByteArray& payload)
{
    if (state_ != State::Connected || payload.isEmpty()) {
        return false;
    }
    sentPackets_.append(payload);
    emit bytesWritten(payload);

    // SYNTHETIC deterministic echo; this is not a hardware-behavior claim.
    QTimer::singleShot(10, this, [this, payload] { emit bytesReceived(payload); });
    return true;
}

const QVector<QByteArray>& FakeTransport::sentPackets() const noexcept
{
    return sentPackets_;
}

void FakeTransport::simulateIncoming(const QByteArray& payload)
{
    if (state_ == State::Connected) {
        emit bytesReceived(payload);
    }
}

void FakeTransport::setRemoteChangesEnabled(const bool enabled)
{
    remoteChangesEnabled_ = enabled;
    if (enabled && state_ == State::Connected) {
        remoteChangeTimer_.start();
    } else {
        remoteChangeTimer_.stop();
    }
}

void FakeTransport::setState(const State state)
{
    if (state_ == state) {
        return;
    }
    state_ = state;
    emit stateChanged(state_);
}

void FakeTransport::emitSyntheticRemoteChange()
{
    constexpr int channelCount = 7;
    const int channel = remoteStep_ % channelCount;
    const double value = static_cast<double>((remoteStep_ * 37) % 256) / 255.0;
    ++remoteStep_;
    if (const auto packet = protocol::encodeFaderLevel(static_cast<quint8>(channel + 1), value);
        packet.has_value()) {
        emit bytesReceived(*packet);
    }
}

} // namespace flow8::simulator
