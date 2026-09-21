#pragma once

#include <QByteArray>
#include <QObject>
#include <QString>

namespace flow8 {

class Flow8Transport : public QObject {
    Q_OBJECT

public:
    enum class State {
        Disconnected,
        Scanning,
        Connecting,
        // Confirmed APK lifecycle phases. Qt does not expose identical MTU
        // control on every backend, so presence in this enum is not proof the
        // current transport performed the Android operation.
        RequestingMtu,
        DiscoveringServices,
        WaitingForHandshake,
        WaitingForHandshakeReply,
        Connected,
        Reconnecting,
        Error,
    };
    Q_ENUM(State)

    explicit Flow8Transport(QObject* parent = nullptr);
    ~Flow8Transport() override;

    [[nodiscard]] virtual QString displayName() const = 0;
    [[nodiscard]] virtual State state() const noexcept = 0;
    [[nodiscard]] virtual bool isSimulator() const noexcept { return false; }

    virtual void connectTransport() = 0;
    virtual void disconnectTransport() = 0;
    virtual bool send(const QByteArray& payload) = 0;

signals:
    void stateChanged(flow8::Flow8Transport::State state);
    void bytesReceived(const QByteArray& payload);
    void bytesWritten(const QByteArray& payload);
    void errorOccurred(const QString& message);
};

} // namespace flow8

Q_DECLARE_METATYPE(flow8::Flow8Transport::State)
