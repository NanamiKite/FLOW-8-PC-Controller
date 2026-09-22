#pragma once

#include "model/evidence_status.h"

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
    // Identifies the origin of received bytes. Generic/test transports must
    // opt in explicitly; only an actual hardware backend may claim device
    // evidence.
    [[nodiscard]] virtual model::EvidenceStatus observationEvidence() const noexcept
    {
        return model::EvidenceStatus::Unknown;
    }
    [[nodiscard]] virtual QString observationSource() const
    {
        return QStringLiteral("unclassified transport RX");
    }

    virtual void connectTransport() = 0;
    virtual void disconnectTransport() = 0;
    virtual bool send(const QByteArray& payload) = 0;
    // Called only after a complete protocol state response established the
    // application session. Generic transports may ignore it.
    virtual void protocolSessionReady() {}

signals:
    void stateChanged(flow8::Flow8Transport::State state);
    void bytesReceived(const QByteArray& payload);
    void bytesWritten(const QByteArray& payload);
    void errorOccurred(const QString& message);
};

} // namespace flow8

Q_DECLARE_METATYPE(flow8::Flow8Transport::State)
