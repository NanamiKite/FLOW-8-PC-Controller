#pragma once

#include "core/flow8_transport.h"

#include <QTimer>
#include <QVector>

namespace flow8::simulator {

class FakeTransport final : public Flow8Transport {
    Q_OBJECT

public:
    explicit FakeTransport(QObject* parent = nullptr);

    [[nodiscard]] QString displayName() const override;
    [[nodiscard]] State state() const noexcept override;
    [[nodiscard]] bool isSimulator() const noexcept override;

    void connectTransport() override;
    void disconnectTransport() override;
    bool send(const QByteArray& payload) override;

    [[nodiscard]] const QVector<QByteArray>& sentPackets() const noexcept;
    void simulateIncoming(const QByteArray& payload);
    void setRemoteChangesEnabled(bool enabled);

private:
    void setState(State state);
    void emitSyntheticRemoteChange();

    State state_ {State::Disconnected};
    QVector<QByteArray> sentPackets_;
    QTimer remoteChangeTimer_;
    int remoteStep_ {};
    bool remoteChangesEnabled_ {true};
};

} // namespace flow8::simulator
