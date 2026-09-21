#pragma once

#include "core/flow8_state.h"
#include "core/flow8_transport.h"

#include <QObject>

#include <memory>

namespace flow8 {

class Flow8Device final : public QObject {
    Q_OBJECT

public:
    enum class Control {
        ChannelFader,
        ChannelGain,
        ChannelMute,
        ChannelSolo,
        ChannelPan,
        MainFader,
        MainMute,
    };
    Q_ENUM(Control)

    explicit Flow8Device(QObject* parent = nullptr);
    ~Flow8Device() override;

    [[nodiscard]] Flow8State& state() noexcept;
    [[nodiscard]] const Flow8State& state() const noexcept;
    [[nodiscard]] Flow8Transport* transport() const noexcept;

    void setTransport(std::unique_ptr<Flow8Transport> transport);
    void connectDevice();
    void disconnectDevice();

    [[nodiscard]] bool isControlAvailable(Control control) const noexcept;
    [[nodiscard]] bool setChannelFader(int index, double normalized);
    [[nodiscard]] bool setChannelGain(int index, double normalized);
    [[nodiscard]] bool setChannelMuted(int index, bool muted);
    [[nodiscard]] bool setChannelSoloed(int index, bool soloed);
    [[nodiscard]] bool setChannelPan(int index, double pan);
    [[nodiscard]] bool setMainFader(double normalized);
    [[nodiscard]] bool setMainMuted(bool muted);

signals:
    void transportChanged();
    void controlRejected(flow8::Flow8Device::Control control, const QString& reason);
    void protocolPacketObserved(quint8 type, const QByteArray& raw);

private:
    void handleTransportState(Flow8Transport::State state);
    void handleBytesReceived(const QByteArray& payload);
    void initializeSimulatorProfile();
    [[nodiscard]] bool simulatorReady() const noexcept;
    void reject(Control control, const QString& reason);

    Flow8State state_;
    std::unique_ptr<Flow8Transport> transport_;
};

} // namespace flow8
