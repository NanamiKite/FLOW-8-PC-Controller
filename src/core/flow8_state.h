#pragma once

#include "model/bus.h"
#include "model/channel.h"
#include "model/fx.h"
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
    [[nodiscard]] bool setChannelMuted(int index, bool muted,
                                       model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelSoloed(int index, bool soloed,
                                        model::EvidenceStatus evidence, const QString& source);
    [[nodiscard]] bool setChannelPan(int index, double pan,
                                     model::EvidenceStatus evidence, const QString& source);

    [[nodiscard]] const QVector<model::BusState>& buses() const noexcept;
    void replaceBuses(QVector<model::BusState> buses);

    [[nodiscard]] const model::MainState& main() const noexcept;
    [[nodiscard]] bool setMainFader(double normalized, model::EvidenceStatus evidence,
                                    const QString& source);
    [[nodiscard]] bool setMainMuted(bool muted, model::EvidenceStatus evidence,
                                    const QString& source);

    [[nodiscard]] const QVector<model::MonitorState>& monitors() const noexcept;
    void replaceMonitors(QVector<model::MonitorState> monitors);

    [[nodiscard]] const QVector<model::FxState>& effects() const noexcept;
    void replaceEffects(QVector<model::FxState> effects);

    [[nodiscard]] const QVector<model::SnapshotState>& snapshots() const noexcept;
    void replaceSnapshots(QVector<model::SnapshotState> snapshots);

    // Applies only a complete dump matching the reference layout. Extracted
    // values retain their evidence/source and cannot downgrade stronger state.
    [[nodiscard]] SysExApplyResult applySysExState(const protocol::ParsedSysExState& parsed);

signals:
    void connectionStateChanged(flow8::ConnectionState state);
    void stateReset();
    void channelChanged(int index);
    void mainChanged();

private:
    [[nodiscard]] model::ChannelState* mutableChannel(int index) noexcept;
    void ensureReferenceStateShape();
    [[nodiscard]] static bool isUnitInterval(double value) noexcept;

    ConnectionState connectionState_ {ConnectionState::Disconnected};
    QVector<model::ChannelState> channels_;
    QVector<model::BusState> buses_;
    model::MainState main_;
    QVector<model::MonitorState> monitors_;
    QVector<model::FxState> effects_;
    QVector<model::SnapshotState> snapshots_;
};

} // namespace flow8

Q_DECLARE_METATYPE(flow8::ConnectionState)
