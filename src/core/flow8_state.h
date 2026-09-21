#pragma once

#include "model/bus.h"
#include "model/channel.h"
#include "model/fx.h"
#include "model/preferences.h"
#include "model/meter.h"
#include "model/routing.h"
#include "model/session.h"
#include "model/snapshot.h"

#include <QObject>
#include <QVector>

namespace flow8 {

namespace protocol {
struct ParsedSysExState;
}

struct SysExApplyResult {
    bool applied {};
    int fieldsApplied {};
    int fieldsRejected {};
    QString reason;
};

enum class ConnectionState {
    Disconnected,
    Scanning,
    Connecting,
    Authenticating,
    Synchronizing,
    Connected,
    Ready,
    Error,
};

class Flow8State final : public QObject {
    Q_OBJECT

public:
    explicit Flow8State(QObject* parent = nullptr);

    [[nodiscard]] ConnectionState connectionState() const noexcept;
    void setConnectionState(ConnectionState state) noexcept;

    [[nodiscard]] const QVector<model::ChannelState>& channels() const noexcept;
    [[nodiscard]] const model::ChannelState* channel(int index) const noexcept;
    void replaceChannels(QVector<model::ChannelState> channels);

    [[nodiscard]] bool setChannelFader(int index, double normalized,
                                       model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelGain(int index, double normalized,
                                       model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelPhaseInverted(int index, bool inverted,
                                                model::EvidenceStatus evidence,
                                                const QString& source);
    [[nodiscard]] bool setChannelMuted(int index, bool muted,
                                       model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelSoloed(int index, bool soloed,
                                        model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelPan(int index, double pan,
                                     model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelName(int index, const QString& name,
                                      model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelIcon(int index, model::ChannelIcon icon,
                                      model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelVisible(int index, bool visible,
                                         model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelPhantom(int index, bool enabled,
                                         model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelLowCut(int index, bool enabled, double frequencyHz,
                                        model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setMonitorSendMode(int index, int monitor,
                                          model::MonitorSendMode mode,
                                          model::EvidenceStatus evidence,
                                          const QString& source);
    [[nodiscard]] bool setChannelEqGain(int index, int band, double gainDb,
                                        model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelCompressorAmount(int index, double amount,
                                                   model::EvidenceStatus evidence,
                                                   const QString& source);
    [[nodiscard]] bool setChannelSendLevelDb(int index, int send, double levelDb,
                                             model::EvidenceStatus evidence,
                                             const QString& source);
    [[nodiscard]] bool setChannelMeter(int index, double level, double peak, bool clipping,
                                       model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] const model::MeterState& meters() const noexcept;
    [[nodiscard]] const model::InputMeterState* inputMeter(int index) const noexcept;
    [[nodiscard]] const model::OutputMeterState* outputMeter(
        model::RoutingDestination destination) const noexcept;
    [[nodiscard]] bool setOutputMeter(model::RoutingDestination destination,
                                      double level, double peak, bool clipping,
                                      model::EvidenceStatus evidence,
                                      const QString& source);

    [[nodiscard]] const QVector<model::BusState>& buses() const noexcept;
    [[nodiscard]] const model::BusState* bus(int index) const noexcept;
    void replaceBuses(QVector<model::BusState> buses);
    [[nodiscard]] bool setBusFader(int index, double normalized,
                                   model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setBusMuted(int index, bool muted,
                                   model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setBusBalance(int index, double balance,
                                     model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setBusLimiterDb(int index, double thresholdDb,
                                       model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setBusEqGain(int index, int band, double gainDb,
                                    model::EvidenceStatus evidence, const QString& source);

    [[nodiscard]] const QVector<model::FxState>& effects() const noexcept;
    void replaceEffects(QVector<model::FxState> effects);
    [[nodiscard]] bool setFxPreset(int index, int preset, model::EvidenceStatus evidence,
                                   const QString& source);
    [[nodiscard]] bool setFxParameter(int index, int parameter, double value,
                                      model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setFxMuted(int index, bool muted, model::EvidenceStatus evidence,
                                  const QString& source);
    [[nodiscard]] bool setFxTapTempo(int index, double bpm, model::EvidenceStatus evidence,
                                     const QString& source);
    [[nodiscard]] const model::GlobalTempoState& globalTempo() const noexcept;
    [[nodiscard]] bool setGlobalTempo(double bpm, model::EvidenceStatus evidence,
                                      const QString& source);

    [[nodiscard]] const QVector<model::SnapshotState>& snapshots() const noexcept;
    void replaceSnapshots(QVector<model::SnapshotState> snapshots);
    [[nodiscard]] int activeSnapshotIndex() const noexcept;
    [[nodiscard]] bool setActiveSnapshotIndex(int index);
    [[nodiscard]] bool setSnapshotName(int index, const QString& name,
                                       model::EvidenceStatus evidence,
                                       const QString& source);

    [[nodiscard]] const model::RoutingState& routing() const noexcept;
    [[nodiscard]] const model::RouteLevelState* routeLevel(
        int sourceIndex, model::RoutingDestination destination) const noexcept;
    void replaceRouting(model::RoutingState routing);
    [[nodiscard]] bool setRouteLevel(int sourceIndex,
                                     model::RoutingDestination destination,
                                     double normalized,
                                     model::EvidenceStatus evidence,
                                     const QString& source);
    [[nodiscard]] bool setRouteLevelPending(int sourceIndex,
                                            model::RoutingDestination destination,
                                            double normalized);
    [[nodiscard]] bool failRouteLevel(int sourceIndex,
                                      model::RoutingDestination destination,
                                      const QString& error);
    [[nodiscard]] bool setRouteEnabled(int inputIndex, model::RoutingDestination destination,
                                       bool enabled, model::EvidenceStatus evidence,
                                       const QString& source);
    [[nodiscard]] bool setUsbMode(model::UsbMode mode, model::EvidenceStatus evidence,
                                  const QString& source);
    [[nodiscard]] bool setUsbRouteEnabled(model::UsbRouteDestination destination, bool enabled,
                                           model::EvidenceStatus evidence,
                                           const QString& source);
    [[nodiscard]] bool setFxOutputRouteEnabled(
        int effectIndex, model::FxOutputDestination destination, bool enabled,
        model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setHeadphoneSource(model::HeadphoneSource sourceValue,
                                           model::EvidenceStatus evidence,
                                           const QString& source);
    [[nodiscard]] bool setMonitorStereoLink(bool linked, model::EvidenceStatus evidence,
                                             const QString& source);

    [[nodiscard]] const model::AppPreferences& preferences() const noexcept;
    void setPreferences(model::AppPreferences preferences);

    [[nodiscard]] const model::AssistedSetupState& assistedSetup() const noexcept;
    void setAssistedSetup(model::AssistedSetupState setup);
    [[nodiscard]] const model::EzGainSession& ezGainSession() const noexcept;
    void setEzGainSession(model::EzGainSession session);

    // Applies only a complete dump matching the reference layout. Extracted
    // values retain their evidence/source and cannot downgrade stronger state.
    [[nodiscard]] SysExApplyResult applySysExState(const protocol::ParsedSysExState& parsed);

signals:
    void connectionStateChanged(flow8::ConnectionState state);
    void stateReset();
    void channelChanged(int index);
    void busChanged(int index);
    void effectChanged(int index);
    void snapshotChanged(int index);
    void routingChanged();
    void preferencesChanged();
    void assistedSetupChanged();
    void ezGainSessionChanged();
    void globalTempoChanged();
    void inputMeterChanged(int index);
    void outputMeterChanged(flow8::model::RoutingDestination destination);

private:
    [[nodiscard]] model::ChannelState* mutableChannel(int index) noexcept;
    [[nodiscard]] model::BusState* mutableBus(int index) noexcept;
    [[nodiscard]] model::FxState* mutableEffect(int index) noexcept;
    [[nodiscard]] model::RouteLevelState* mutableRouteLevel(
        int sourceIndex, model::RoutingDestination destination) noexcept;
    void ensureReferenceStateShape();
    [[nodiscard]] static bool isUnitInterval(double value) noexcept;

    ConnectionState connectionState_ {ConnectionState::Disconnected};
    QVector<model::ChannelState> channels_;
    QVector<model::BusState> buses_;
    QVector<model::FxState> effects_;
    model::GlobalTempoState globalTempo_;
    QVector<model::SnapshotState> snapshots_;
    int activeSnapshotIndex_ {-1};
    model::RoutingState routing_;
    model::AppPreferences preferences_;
    model::AssistedSetupState assistedSetup_;
    model::EzGainSession ezGainSession_;
    model::MeterState meters_;
};

} // namespace flow8

Q_DECLARE_METATYPE(flow8::ConnectionState)
