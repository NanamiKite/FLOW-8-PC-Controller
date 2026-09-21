#include "simulator/fake_transport.h"

#include <QTimer>

namespace flow8::simulator {

FakeTransport::FakeTransport(QObject* parent)
    : Flow8Transport(parent)
{
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
    });
}

void FakeTransport::disconnectTransport()
{
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

void FakeTransport::setState(const State state)
{
    if (state_ == state) {
        return;
    }
    state_ = state;
    emit stateChanged(state_);
}

} // namespace flow8::simulator
