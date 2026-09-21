#pragma once

#include "core/flow8_state.h"
#include "core/flow8_transport.h"

#include <QObject>
#include <QElapsedTimer>
#include <QTimer>

#include <memory>

namespace flow8 {

class Flow8Device final : public QObject {
    Q_OBJECT

public:
    enum class Control {
        ChannelFader,
        ChannelGain,
        ChannelPhase,
        ChannelMute,
        ChannelSolo,
        ChannelPan,
        ChannelIdentity,
        ChannelVisibility,
        ChannelPhantom,
        ChannelLowCut,
        MonitorSendMode,
        ChannelEq,
        ChannelCompressor,
        ChannelSend,
        RouteLevel,
        BusFader,
        BusMute,
        BusBalance,
        BusLimiter,
        BusEq,
        FxPreset,
        FxParameter,
        FxMute,
        FxTapTempo,
        SnapshotRecall,
        SnapshotStore,
        Routing,
        AssistedSetup,
        EzGain,
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
    [[nodiscard]] bool setChannelPhaseInverted(int index, bool inverted);
    [[nodiscard]] bool setChannelMuted(int index, bool muted);
    [[nodiscard]] bool setChannelSoloed(int index, bool soloed);
    [[nodiscard]] bool setChannelPan(int index, double pan);
    [[nodiscard]] bool setChannelName(int index, const QString& name);
    [[nodiscard]] bool setChannelIcon(int index, model::ChannelIcon icon);
    [[nodiscard]] bool setChannelVisible(int index, bool visible);
    [[nodiscard]] bool setChannelPhantom(int index, bool enabled);
    [[nodiscard]] bool setChannelLowCut(int index, bool enabled, double frequencyHz);
    [[nodiscard]] bool setMonitorSendMode(int index, int monitor,
                                          model::MonitorSendMode mode);
    [[nodiscard]] bool setChannelEqGain(int index, int band, double gainDb);
    [[nodiscard]] bool setChannelCompressorAmount(int index, double amount);
    [[nodiscard]] bool setChannelSendLevel(int index, int send, double normalized);
    [[nodiscard]] bool setRouteLevel(int sourceIndex,
                                     model::RoutingDestination destination,
                                     double normalized);
    [[nodiscard]] bool setDestinationMaster(
        model::RoutingDestination destination, double normalized);
    [[nodiscard]] bool setBusFader(int index, double normalized);
    [[nodiscard]] bool setBusMuted(int index, bool muted);
    [[nodiscard]] bool setBusBalance(int index, double balance);
    [[nodiscard]] bool setBusLimiterDb(int index, double thresholdDb);
    [[nodiscard]] bool setBusEqGain(int index, int band, double gainDb);
    [[nodiscard]] bool setFxPreset(int index, int preset);
    [[nodiscard]] bool setFxParameter(int index, int parameter, double normalized);
    [[nodiscard]] bool setFxMuted(int index, bool muted);
    [[nodiscard]] bool setFxMaster(int index, double normalized);
    [[nodiscard]] bool tapTempo();
    [[nodiscard]] bool recallSnapshot(int index);
    [[nodiscard]] bool storeAppSnapshot(const QString& name, model::SnapshotScope scope);
    [[nodiscard]] bool loadAppSnapshot(int libraryIndex);
    [[nodiscard]] bool renameAppSnapshot(int libraryIndex, const QString& name);
    [[nodiscard]] bool deleteAppSnapshot(int libraryIndex);
    [[nodiscard]] bool setUsbMode(model::UsbMode mode);
    [[nodiscard]] bool setUsbInputAssignment(
        int pairIndex, model::UsbPlaybackAssignment assignment);
    [[nodiscard]] bool setPhysicalMonitorOutputFeed(
        int outputIndex, model::PhysicalMonitorOutputFeed feed);
    [[nodiscard]] bool setFxOutputRouteEnabled(
        int effectIndex, model::FxOutputDestination destination, bool enabled);
    [[nodiscard]] bool setHeadphoneSource(model::HeadphoneSource source);
    [[nodiscard]] bool setHeadphoneTapPoint(model::RoutingTapPoint tapPoint);
    [[nodiscard]] bool setBluetoothUsbPhonesOnly(bool enabled);
    [[nodiscard]] bool setOutputPadMinus10Dbv(model::PhysicalOutputId output, bool enabled);
    [[nodiscard]] bool setMonitorStereoLink(bool linked);
    void setPreferences(model::AppPreferences preferences);
    [[nodiscard]] bool configureAssistedSetup(model::InputId input,
                                               model::AssistedSourceType sourceType);
    [[nodiscard]] bool applyAssistedSetup();
    [[nodiscard]] bool startEzGain(const QVector<model::InputId>& targets);
    void cancelEzGain();
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
    void updateSimulatorMeters();
    [[nodiscard]] bool simulatorReady() const noexcept;
    void reject(Control control, const QString& reason);

    Flow8State state_;
    std::unique_ptr<Flow8Transport> transport_;
    QTimer simulatorMeterTimer_;
    QElapsedTimer tapTimer_;
    int simulatorMeterStep_ {};
};

} // namespace flow8
