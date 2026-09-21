#include "core/flow8_device.h"

#include "model/flow8_capabilities.h"
#include "protocol/codec.h"
#include "protocol/flow8_protocol.h"
#include "protocol/packet.h"

#include <QDateTime>
#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

#include <algorithm>
#include <array>
#include <cmath>

namespace flow8 {
namespace {

constexpr auto simulatorSource = "SYNTHETIC simulator state; not hardware evidence";

model::StateValue<double> simulatorDouble(const double value)
{
    return model::StateValue<double>::known(value, model::EvidenceStatus::Synthetic,
                                            QString::fromLatin1(simulatorSource));
}

model::StateValue<bool> simulatorBool(const bool value)
{
    return model::StateValue<bool>::known(value, model::EvidenceStatus::Synthetic,
                                          QString::fromLatin1(simulatorSource));
}

bool snapshotIncludes(const model::SnapshotScope selected,
                      const model::SnapshotScope domain) noexcept
{
    return selected == model::SnapshotScope::Full || selected == domain;
}

double dbToNormalized(const double db) noexcept
{
    return db <= -70.0 ? 0.0 : std::clamp((db + 70.0) / 80.0, 0.0, 1.0);
}

QJsonObject observedDouble(const model::StateValue<double>& value)
{
    QJsonObject result;
    if (value.value.has_value()) {
        result.insert(QStringLiteral("value"), *value.value);
    }
    return result;
}

QJsonObject observedBool(const model::StateValue<bool>& value)
{
    QJsonObject result;
    if (value.value.has_value()) {
        result.insert(QStringLiteral("value"), *value.value);
    }
    return result;
}

} // namespace

Flow8Device::Flow8Device(QObject* parent)
    : QObject(parent)
    , state_()
{
    simulatorMeterTimer_.setInterval(60);
    connect(&simulatorMeterTimer_, &QTimer::timeout,
            this, &Flow8Device::updateSimulatorMeters);
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
    case Control::ChannelPhase:
    case Control::ChannelMute:
    case Control::ChannelSolo:
    case Control::ChannelPan:
    case Control::ChannelIdentity:
    case Control::ChannelVisibility:
    case Control::ChannelPhantom:
    case Control::ChannelLowCut:
    case Control::MonitorSendMode:
    case Control::ChannelEq:
    case Control::ChannelCompressor:
    case Control::ChannelSend:
    case Control::RouteLevel:
    case Control::BusFader:
    case Control::BusMute:
    case Control::BusBalance:
    case Control::BusLimiter:
    case Control::BusEq:
    case Control::FxPreset:
    case Control::FxParameter:
    case Control::FxMute:
    case Control::FxTapTempo:
    case Control::SnapshotRecall:
    case Control::SnapshotStore:
    case Control::Routing:
    case Control::AssistedSetup:
    case Control::EzGain:
    case Control::MainFader:
    case Control::MainMute:
        return true;
    }
    return false;
}

bool Flow8Device::setChannelFader(const int index, const double normalized)
{
    return setRouteLevel(index, model::RoutingDestination::Main, normalized);
}

bool Flow8Device::setChannelGain(const int index, const double normalized)
{
    const auto* channel = state_.channel(index);
    if (!isControlAvailable(Control::ChannelGain) || channel == nullptr
        || !channel->capabilities.gain) {
        reject(Control::ChannelGain, QCoreApplication::translate(
            "Flow8Device", "Gain mapping is not hardware-verified."));
        return false;
    }
    return state_.setChannelGain(index, normalized, model::EvidenceStatus::Unknown,
                                 QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelPhaseInverted(const int index, const bool inverted)
{
    const auto* channel = state_.channel(index);
    if (!isControlAvailable(Control::ChannelPhase) || channel == nullptr
        || !channel->capabilities.phase || !channel->phaseInverted.has_value()) {
        reject(Control::ChannelPhase, QCoreApplication::translate(
            "Flow8Device", "Phase control is unavailable."));
        return false;
    }
    return state_.setChannelPhaseInverted(
        index, inverted, model::EvidenceStatus::Unknown,
        QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelMuted(const int index, const bool muted)
{
    if (!isControlAvailable(Control::ChannelMute)) {
        reject(Control::ChannelMute, QCoreApplication::translate(
            "Flow8Device", "Mute mapping is not hardware-verified."));
        return false;
    }
    return state_.setChannelMuted(index, muted, model::EvidenceStatus::Unknown,
                                  QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelSoloed(const int index, const bool soloed)
{
    if (!isControlAvailable(Control::ChannelSolo)) {
        reject(Control::ChannelSolo, QCoreApplication::translate(
            "Flow8Device", "Solo mapping is not hardware-verified."));
        return false;
    }
    return state_.setChannelSoloed(index, soloed, model::EvidenceStatus::Unknown,
                                   QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelPan(const int index, const double pan)
{
    if (!isControlAvailable(Control::ChannelPan)) {
        reject(Control::ChannelPan, QCoreApplication::translate(
            "Flow8Device", "Pan mapping is not hardware-verified."));
        return false;
    }
    return state_.setChannelPan(index, pan, model::EvidenceStatus::Unknown,
                                QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelName(const int index, const QString& name)
{
    if (!isControlAvailable(Control::ChannelIdentity)) {
        reject(Control::ChannelIdentity, QCoreApplication::translate(
            "Flow8Device", "Channel customization is unavailable."));
        return false;
    }
    return state_.setChannelName(index, name, model::EvidenceStatus::Unknown,
                                 QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelIcon(const int index, const model::ChannelIcon icon)
{
    if (!isControlAvailable(Control::ChannelIdentity)) {
        reject(Control::ChannelIdentity, QCoreApplication::translate(
            "Flow8Device", "Channel customization is unavailable."));
        return false;
    }
    return state_.setChannelIcon(index, icon, model::EvidenceStatus::Unknown,
                                 QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelVisible(const int index, const bool visible)
{
    if (!isControlAvailable(Control::ChannelVisibility)) {
        reject(Control::ChannelVisibility, QCoreApplication::translate(
            "Flow8Device", "Channel visibility is unavailable."));
        return false;
    }
    return state_.setChannelVisible(index, visible, model::EvidenceStatus::Unknown,
                                    QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelPhantom(const int index, const bool enabled)
{
    const auto* channel = state_.channel(index);
    if (!isControlAvailable(Control::ChannelPhantom) || channel == nullptr
        || !channel->capabilities.phantom48V || !channel->phantom48V.has_value()) {
        reject(Control::ChannelPhantom, QCoreApplication::translate(
            "Flow8Device", "Phantom Power is unavailable for this input."));
        return false;
    }
    return state_.setChannelPhantom(index, enabled, model::EvidenceStatus::Unknown,
                                    QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelLowCut(const int index, const bool enabled,
                                   const double frequencyHz)
{
    if (!isControlAvailable(Control::ChannelLowCut)) {
        reject(Control::ChannelLowCut, QCoreApplication::translate(
            "Flow8Device", "Low Cut is unavailable."));
        return false;
    }
    return state_.setChannelLowCut(index, enabled, frequencyHz,
                                   model::EvidenceStatus::Unknown,
                                   QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setMonitorSendMode(const int index, const int monitor,
                                     const model::MonitorSendMode mode)
{
    if (!isControlAvailable(Control::MonitorSendMode)) {
        reject(Control::MonitorSendMode, QCoreApplication::translate(
            "Flow8Device", "Monitor send mode is unavailable."));
        return false;
    }
    return state_.setMonitorSendMode(index, monitor, mode,
                                     model::EvidenceStatus::Unknown,
                                     QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelEqGain(const int index, const int band, const double gainDb)
{
    const auto* channel = state_.channel(index);
    if (!isControlAvailable(Control::ChannelEq) || channel == nullptr
        || !channel->capabilities.equalizer) {
        reject(Control::ChannelEq, QCoreApplication::translate(
            "Flow8Device", "Channel EQ is unavailable."));
        return false;
    }
    return state_.setChannelEqGain(index, band, gainDb, model::EvidenceStatus::Unknown,
                                   QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelCompressorAmount(const int index, const double amount)
{
    const auto* channel = state_.channel(index);
    if (!isControlAvailable(Control::ChannelCompressor) || channel == nullptr
        || !channel->capabilities.compressor) {
        reject(Control::ChannelCompressor,
               QCoreApplication::translate(
                   "Flow8Device", "Compressor is unavailable for this input."));
        return false;
    }
    return state_.setChannelCompressorAmount(index, amount, model::EvidenceStatus::Unknown,
                                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setChannelSendLevel(const int index, const int send,
                                      const double normalized)
{
    if (send < 0 || send >= 4) {
        reject(Control::ChannelSend, QCoreApplication::translate(
            "Flow8Device", "Channel send is unavailable."));
        return false;
    }
    return setRouteLevel(index, static_cast<model::RoutingDestination>(send + 1), normalized);
}

bool Flow8Device::setRouteLevel(const int sourceIndex,
                                const model::RoutingDestination destination,
                                const double normalized)
{
    if (!isControlAvailable(Control::RouteLevel)
        || sourceIndex < 0 || sourceIndex >= state_.channels().size()
        || !std::isfinite(normalized) || normalized < 0.0 || normalized > 1.0) {
        reject(Control::RouteLevel, QCoreApplication::translate(
            "Flow8Device", "Route level is unavailable."));
        return false;
    }

    // A future real transport will leave this pending until the matching
    // device notification arrives. The simulator confirms synchronously and
    // never fabricates a BLE payload whose APK layout is still unknown.
    if (!state_.setRouteLevelPending(sourceIndex, destination, normalized)) {
        return false;
    }
    return state_.setRouteLevel(sourceIndex, destination, normalized,
                                model::EvidenceStatus::Synthetic,
                                QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setDestinationMaster(
    const model::RoutingDestination destination, const double normalized)
{
    const int bus = model::busIndexForDestination(destination);
    if (bus >= 0) {
        return setBusFader(bus, normalized);
    }
    if (destination == model::RoutingDestination::Fx1
        || destination == model::RoutingDestination::Fx2) {
        return setFxMaster(destination == model::RoutingDestination::Fx1 ? 0 : 1,
                           normalized);
    }
    return false;
}

bool Flow8Device::setBusFader(const int index, const double normalized)
{
    if (!isControlAvailable(Control::BusFader)) {
        reject(Control::BusFader, QCoreApplication::translate(
            "Flow8Device", "Bus level is unavailable."));
        return false;
    }
    return state_.setBusFader(index, normalized, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setBusMuted(const int index, const bool muted)
{
    if (!isControlAvailable(Control::BusMute)) {
        reject(Control::BusMute, QCoreApplication::translate(
            "Flow8Device", "Bus mute is unavailable."));
        return false;
    }
    return state_.setBusMuted(index, muted, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setBusBalance(const int index, const double balance)
{
    if (!isControlAvailable(Control::BusBalance)) {
        reject(Control::BusBalance, QCoreApplication::translate(
            "Flow8Device", "Bus balance is unavailable."));
        return false;
    }
    return state_.setBusBalance(index, balance, model::EvidenceStatus::Unknown,
                                QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setBusLimiterDb(const int index, const double thresholdDb)
{
    if (!isControlAvailable(Control::BusLimiter)) {
        reject(Control::BusLimiter, QCoreApplication::translate(
            "Flow8Device", "Bus limiter is unavailable."));
        return false;
    }
    return state_.setBusLimiterDb(index, thresholdDb, model::EvidenceStatus::Unknown,
                                  QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setBusEqGain(const int index, const int band, const double gainDb)
{
    if (!isControlAvailable(Control::BusEq)) {
        reject(Control::BusEq, QCoreApplication::translate(
            "Flow8Device", "Bus EQ is unavailable."));
        return false;
    }
    return state_.setBusEqGain(index, band, gainDb, model::EvidenceStatus::Unknown,
                               QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setFxPreset(const int index, const int preset)
{
    if (!isControlAvailable(Control::FxPreset)) {
        reject(Control::FxPreset, QCoreApplication::translate(
            "Flow8Device", "FX preset is unavailable."));
        return false;
    }
    return state_.setFxPreset(index, preset, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setFxParameter(const int index, const int parameter,
                                 const double normalized)
{
    if (!isControlAvailable(Control::FxParameter)) {
        reject(Control::FxParameter, QCoreApplication::translate(
            "Flow8Device", "FX parameter is unavailable."));
        return false;
    }
    return state_.setFxParameter(index, parameter, normalized, model::EvidenceStatus::Unknown,
                                 QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setFxMuted(const int index, const bool muted)
{
    if (!isControlAvailable(Control::FxMute)) {
        reject(Control::FxMute, QCoreApplication::translate(
            "Flow8Device", "FX mute is unavailable."));
        return false;
    }
    return state_.setFxMuted(index, muted, model::EvidenceStatus::Unknown,
                             QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setFxMaster(const int index, const double normalized)
{
    if (!isControlAvailable(Control::BusFader)) {
        reject(Control::BusFader, QCoreApplication::translate(
            "Flow8Device", "FX master is unavailable."));
        return false;
    }
    return state_.setFxMaster(index, normalized, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::tapTempo()
{
    if (!isControlAvailable(Control::FxTapTempo)) {
        reject(Control::FxTapTempo, QCoreApplication::translate(
            "Flow8Device", "Tap tempo is unavailable."));
        return false;
    }
    double bpm = 120.0;
    if (tapTimer_.isValid()) {
        const qint64 elapsed = tapTimer_.elapsed();
        if (elapsed >= 240 && elapsed <= 1200) {
            bpm = std::clamp(60000.0 / static_cast<double>(elapsed), 50.0, 250.0);
        }
    }
    tapTimer_.restart();
    (void)state_.setGlobalTempo(bpm, model::EvidenceStatus::Unknown,
                                QString::fromLatin1(simulatorSource));
    bool changed = false;
    for (int index = 0; index < state_.effects().size(); ++index) {
        changed = state_.setFxTapTempo(index, bpm, model::EvidenceStatus::Unknown,
                                       QString::fromLatin1(simulatorSource)) || changed;
    }
    return changed;
}

bool Flow8Device::recallSnapshot(const int index)
{
    if (!isControlAvailable(Control::SnapshotRecall)) {
        reject(Control::SnapshotRecall, QCoreApplication::translate(
            "Flow8Device", "Snapshot recall is unavailable."));
        return false;
    }
    return state_.setActiveSnapshotIndex(index);
}

bool Flow8Device::storeAppSnapshot(const QString& name, const model::SnapshotScope scope)
{
    if (!isControlAvailable(Control::SnapshotStore) || name.trimmed().isEmpty()) {
        reject(Control::SnapshotStore, QCoreApplication::translate(
            "Flow8Device", "App snapshot storage is unavailable."));
        return false;
    }
    auto snapshots = state_.snapshots();
    int libraryIndex = 0;
    for (const auto& existing : snapshots) {
        if (existing.storage == model::SnapshotStorage::AppLibrary) {
            ++libraryIndex;
        }
    }
    model::SnapshotState snapshot;
    snapshot.index = libraryIndex;
    snapshot.id = QStringLiteral("app:%1")
        .arg(QDateTime::currentDateTimeUtc().toMSecsSinceEpoch());
    snapshot.storage = model::SnapshotStorage::AppLibrary;
    snapshot.name = model::StateValue<QString>::known(
        name.trimmed(), model::EvidenceStatus::Synthetic,
        QString::fromLatin1(simulatorSource));
    snapshot.timestamp = model::StateValue<QDateTime>::known(
        QDateTime::currentDateTimeUtc(), model::EvidenceStatus::Synthetic,
        QString::fromLatin1(simulatorSource));
    snapshot.scope = model::StateValue<model::SnapshotScope>::known(
        scope, model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
    QJsonObject document;
    document.insert(QStringLiteral("format"), QStringLiteral("flow8-simulator-snapshot-v2"));
    document.insert(QStringLiteral("scope"), static_cast<int>(scope));
    if (snapshotIncludes(scope, model::SnapshotScope::Channel)) {
        QJsonArray channels;
        for (const auto& channel : state_.channels()) {
            QJsonObject entry;
            entry.insert(QStringLiteral("index"), channel.index);
            entry.insert(QStringLiteral("name"), channel.name.value.value_or(QString()));
            entry.insert(QStringLiteral("icon"), static_cast<int>(
                channel.icon.value.value_or(model::ChannelIcon::None)));
            entry.insert(QStringLiteral("visible"), channel.visible.value.value_or(true));
            entry.insert(QStringLiteral("gain"), observedDouble(channel.gain));
            if (channel.phaseInverted.has_value()) {
                entry.insert(QStringLiteral("phaseInverted"),
                             observedBool(*channel.phaseInverted));
            }
            entry.insert(QStringLiteral("fader"), observedDouble(channel.fader));
            entry.insert(QStringLiteral("muted"), observedBool(channel.muted));
            entry.insert(QStringLiteral("soloed"), observedBool(channel.soloed));
            entry.insert(QStringLiteral("pan"), observedDouble(channel.pan));
            if (channel.lowCut.has_value()) {
                QJsonObject lowCut;
                lowCut.insert(QStringLiteral("enabled"), observedBool(channel.lowCut->enabled));
                lowCut.insert(QStringLiteral("frequencyHz"),
                              observedDouble(channel.lowCut->frequencyHz));
                entry.insert(QStringLiteral("lowCut"), lowCut);
            }
            QJsonArray sends;
            for (const auto& send : channel.sendLevelDb) {
                sends.append(send.value.value_or(-144.0));
            }
            entry.insert(QStringLiteral("sendsDb"), sends);
            QJsonArray monitorModes;
            for (const auto& send : channel.monitorSends) {
                monitorModes.append(static_cast<int>(
                    send.mode.value.value_or(model::MonitorSendMode::PostFader)));
            }
            entry.insert(QStringLiteral("monitorModes"), monitorModes);
            channels.append(entry);
        }
        document.insert(QStringLiteral("channels"), channels);
    }
    if (snapshotIncludes(scope, model::SnapshotScope::Main)
        || snapshotIncludes(scope, model::SnapshotScope::Monitor)) {
        QJsonArray buses;
        for (const auto& bus : state_.buses()) {
            const bool wanted = (bus.busId == model::BusId::Main
                    && snapshotIncludes(scope, model::SnapshotScope::Main))
                || ((bus.busId == model::BusId::Monitor1
                     || bus.busId == model::BusId::Monitor2)
                    && snapshotIncludes(scope, model::SnapshotScope::Monitor));
            if (!wanted) continue;
            QJsonObject entry;
            entry.insert(QStringLiteral("index"), bus.index);
            entry.insert(QStringLiteral("fader"), observedDouble(bus.fader));
            if (bus.muted.has_value()) {
                entry.insert(QStringLiteral("muted"), observedBool(*bus.muted));
            }
            if (bus.balance.has_value()) {
                entry.insert(QStringLiteral("balance"), observedDouble(*bus.balance));
            }
            if (bus.limiterDb.has_value()) {
                entry.insert(QStringLiteral("limiterDb"), observedDouble(*bus.limiterDb));
            }
            buses.append(entry);
        }
        document.insert(QStringLiteral("buses"), buses);
    }
    if (snapshotIncludes(scope, model::SnapshotScope::Fx)) {
        QJsonArray effects;
        for (const auto& effect : state_.effects()) {
            QJsonObject entry;
            entry.insert(QStringLiteral("index"), effect.index);
            entry.insert(QStringLiteral("master"), effect.master.value.value_or(0.7));
            entry.insert(QStringLiteral("preset"), effect.preset.value.value_or(1));
            entry.insert(QStringLiteral("muted"), effect.muted.value.value_or(false));
            entry.insert(QStringLiteral("parameter1"),
                         effect.parameters[0].value.value.value_or(0.0));
            entry.insert(QStringLiteral("parameter2"),
                         effect.parameters[1].value.value.value_or(0.0));
            effects.append(entry);
        }
        document.insert(QStringLiteral("effects"), effects);
        document.insert(QStringLiteral("tempoBpm"),
                        state_.globalTempo().bpm.value.value_or(120.0));
    }
    if (snapshotIncludes(scope, model::SnapshotScope::Routing)) {
        QJsonObject routing;
        routing.insert(QStringLiteral("usbMode"), static_cast<int>(
            state_.routing().usb.mode.value.value_or(model::UsbMode::Streaming)));
        routing.insert(QStringLiteral("input56Source"), static_cast<int>(
            state_.routing().usb.input56Source.value.value_or(
                model::UsbPlaybackAssignment::AnalogInput)));
        routing.insert(QStringLiteral("input78Source"), static_cast<int>(
            state_.routing().usb.input78Source.value.value_or(
                model::UsbPlaybackAssignment::AnalogInput)));
        routing.insert(QStringLiteral("monitorStereoLink"),
                       state_.routing().monitor.stereoLinked.value.value_or(false));
        QJsonArray monitorSources;
        for (const auto& monitor : state_.routing().monitor.outputSources) {
            monitorSources.append(static_cast<int>(
                monitor.value.value_or(model::MonitorRouteSource::MonitorMix)));
        }
        routing.insert(QStringLiteral("monitorSources"), monitorSources);
        routing.insert(QStringLiteral("bluetoothUsbPhonesOnly"),
                       state_.routing().headphones.bluetoothUsbPhonesOnly.value.value_or(false));
        routing.insert(QStringLiteral("headphoneSource"), static_cast<int>(
            state_.routing().headphones.source.value.value_or(
                model::HeadphoneSource::Main)));
        routing.insert(QStringLiteral("headphoneTapPoint"), static_cast<int>(
            state_.routing().headphones.tapPoint.value.value_or(
                model::RoutingTapPoint::PostFader)));
        QJsonArray outputPads;
        for (const auto output : {model::PhysicalOutputId::MainOut,
                                  model::PhysicalOutputId::MonitorOut1,
                                  model::PhysicalOutputId::MonitorOut2}) {
            const auto* state = state_.physicalOutput(output);
            outputPads.append(state != nullptr && state->padMinus10Dbv.has_value()
                ? state->padMinus10Dbv->value.value_or(false) : false);
        }
        routing.insert(QStringLiteral("outputPadsMinus10Dbv"), outputPads);
        QJsonArray fxRoutes;
        for (const auto& route : state_.routing().fxOutputRoutes) {
            fxRoutes.append(route.enabled.value.value_or(false));
        }
        routing.insert(QStringLiteral("fxOutputRoutes"), fxRoutes);
        document.insert(QStringLiteral("routing"), routing);
    }
    snapshot.data = model::StateValue<QByteArray>::known(
        QJsonDocument(document).toJson(QJsonDocument::Compact),
        model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
    snapshot.shareable = simulatorBool(true);
    snapshot.metadata.insert(QStringLiteral("origin"), QStringLiteral("simulator"));
    snapshots.append(std::move(snapshot));
    state_.replaceSnapshots(std::move(snapshots));
    return true;
}

bool Flow8Device::loadAppSnapshot(const int libraryIndex)
{
    if (!isControlAvailable(Control::SnapshotRecall) || libraryIndex < 0) {
        return false;
    }
    int current = 0;
    for (int index = 0; index < state_.snapshots().size(); ++index) {
        if (state_.snapshots().at(index).storage != model::SnapshotStorage::AppLibrary) {
            continue;
        }
        if (current++ == libraryIndex) {
            const auto& snapshot = state_.snapshots().at(index);
            if (!snapshot.data.value.has_value()) {
                return false;
            }
            QJsonParseError error;
            const QJsonDocument parsed = QJsonDocument::fromJson(*snapshot.data.value, &error);
            if (error.error != QJsonParseError::NoError || !parsed.isObject()) {
                return false;
            }
            const QJsonObject root = parsed.object();
            const QString format = root.value(QStringLiteral("format")).toString();
            if (format != QStringLiteral("flow8-simulator-snapshot-v1")
                && format != QStringLiteral("flow8-simulator-snapshot-v2")) {
                return false;
            }
            const QString source = QString::fromLatin1(simulatorSource);
            for (const auto value : root.value(QStringLiteral("channels")).toArray()) {
                const auto entry = value.toObject();
                const int channel = entry.value(QStringLiteral("index")).toInt(-1);
                const auto scalar = [](const QJsonObject& object, const char* key) {
                    return object.value(QString::fromLatin1(key)).toObject()
                        .value(QStringLiteral("value"));
                };
                if (!entry.value(QStringLiteral("name")).toString().isEmpty()) {
                    (void)state_.setChannelName(channel,
                        entry.value(QStringLiteral("name")).toString(),
                        model::EvidenceStatus::Unknown, source);
                }
                (void)state_.setChannelIcon(channel, static_cast<model::ChannelIcon>(
                    entry.value(QStringLiteral("icon")).toInt()),
                    model::EvidenceStatus::Unknown, source);
                (void)state_.setChannelVisible(channel,
                    entry.value(QStringLiteral("visible")).toBool(true),
                    model::EvidenceStatus::Unknown, source);
                if (!scalar(entry, "gain").isUndefined())
                    (void)state_.setChannelGain(channel, scalar(entry, "gain").toDouble(),
                                                model::EvidenceStatus::Unknown, source);
                if (!scalar(entry, "phaseInverted").isUndefined())
                    (void)state_.setChannelPhaseInverted(
                        channel, scalar(entry, "phaseInverted").toBool(),
                        model::EvidenceStatus::Unknown, source);
                if (!scalar(entry, "fader").isUndefined())
                    (void)state_.setChannelFader(channel, scalar(entry, "fader").toDouble(),
                                                 model::EvidenceStatus::Unknown, source);
                if (!scalar(entry, "muted").isUndefined())
                    (void)state_.setChannelMuted(channel, scalar(entry, "muted").toBool(),
                                                 model::EvidenceStatus::Unknown, source);
                if (!scalar(entry, "soloed").isUndefined())
                    (void)state_.setChannelSoloed(channel, scalar(entry, "soloed").toBool(),
                                                  model::EvidenceStatus::Unknown, source);
                if (!scalar(entry, "pan").isUndefined())
                    (void)state_.setChannelPan(channel, scalar(entry, "pan").toDouble(),
                                               model::EvidenceStatus::Unknown, source);
                const auto lowCut = entry.value(QStringLiteral("lowCut")).toObject();
                if (!lowCut.isEmpty()) {
                    (void)state_.setChannelLowCut(
                        channel, scalar(lowCut, "enabled").toBool(),
                        scalar(lowCut, "frequencyHz").toDouble(20.0),
                        model::EvidenceStatus::Unknown, source);
                }
                const auto sends = entry.value(QStringLiteral("sendsDb")).toArray();
                for (int send = 0; send < sends.size() && send < 4; ++send) {
                    (void)state_.setChannelSendLevelDb(channel, send, sends.at(send).toDouble(),
                                                       model::EvidenceStatus::Unknown, source);
                }
                const auto modes = entry.value(QStringLiteral("monitorModes")).toArray();
                for (int monitor = 0; monitor < modes.size() && monitor < 2; ++monitor) {
                    (void)state_.setMonitorSendMode(channel, monitor,
                        static_cast<model::MonitorSendMode>(modes.at(monitor).toInt()),
                        model::EvidenceStatus::Unknown, source);
                }
            }
            for (const auto value : root.value(QStringLiteral("buses")).toArray()) {
                const auto entry = value.toObject();
                const int bus = entry.value(QStringLiteral("index")).toInt(-1);
                const auto scalar = [&entry](const char* key) {
                    return entry.value(QString::fromLatin1(key)).toObject()
                        .value(QStringLiteral("value"));
                };
                if (!scalar("fader").isUndefined()) {
                    (void)state_.setBusFader(bus, scalar("fader").toDouble(),
                                             model::EvidenceStatus::Unknown, source);
                }
                if (!scalar("muted").isUndefined())
                    (void)state_.setBusMuted(bus, scalar("muted").toBool(),
                                             model::EvidenceStatus::Unknown, source);
                if (!scalar("balance").isUndefined())
                    (void)state_.setBusBalance(bus, scalar("balance").toDouble(),
                                               model::EvidenceStatus::Unknown, source);
                if (!scalar("limiterDb").isUndefined())
                    (void)state_.setBusLimiterDb(bus, scalar("limiterDb").toDouble(),
                                                 model::EvidenceStatus::Unknown, source);
            }
            for (const auto value : root.value(QStringLiteral("effects")).toArray()) {
                const auto entry = value.toObject();
                const int effect = entry.value(QStringLiteral("index")).toInt(-1);
                if (entry.contains(QStringLiteral("master"))) {
                    (void)state_.setFxMaster(
                        effect, entry.value(QStringLiteral("master")).toDouble(0.7),
                        model::EvidenceStatus::Unknown, source);
                }
                (void)state_.setFxPreset(effect, entry.value(QStringLiteral("preset")).toInt(1),
                                         model::EvidenceStatus::Unknown, source);
                (void)state_.setFxMuted(effect, entry.value(QStringLiteral("muted")).toBool(),
                                        model::EvidenceStatus::Unknown, source);
                (void)state_.setFxParameter(effect, 0,
                    entry.value(QStringLiteral("parameter1")).toDouble(),
                    model::EvidenceStatus::Unknown, source);
                (void)state_.setFxParameter(effect, 1,
                    entry.value(QStringLiteral("parameter2")).toDouble(),
                    model::EvidenceStatus::Unknown, source);
            }
            if (root.contains(QStringLiteral("tempoBpm"))) {
                (void)state_.setGlobalTempo(
                    root.value(QStringLiteral("tempoBpm")).toDouble(120.0),
                    model::EvidenceStatus::Unknown, source);
            }
            const auto routing = root.value(QStringLiteral("routing")).toObject();
            if (!routing.isEmpty()) {
                const int usbMode = routing.value(QStringLiteral("usbMode")).toInt(-1);
                if (usbMode >= static_cast<int>(model::UsbMode::Streaming)
                    && usbMode <= static_cast<int>(model::UsbMode::Recording)) {
                    (void)state_.setUsbMode(
                        static_cast<model::UsbMode>(usbMode),
                        model::EvidenceStatus::Unknown, source);
                }
                if (routing.contains(QStringLiteral("input56Source"))) {
                    const int assignment =
                        routing.value(QStringLiteral("input56Source")).toInt(-1);
                    if (assignment >= 0 && assignment <= 1) {
                        (void)state_.setUsbPlaybackAssignment(
                            0, static_cast<model::UsbPlaybackAssignment>(assignment),
                            model::EvidenceStatus::Unknown, source);
                    }
                }
                if (routing.contains(QStringLiteral("input78Source"))) {
                    const int assignment =
                        routing.value(QStringLiteral("input78Source")).toInt(-1);
                    if (assignment >= 0 && assignment <= 1) {
                        (void)state_.setUsbPlaybackAssignment(
                            1, static_cast<model::UsbPlaybackAssignment>(assignment),
                            model::EvidenceStatus::Unknown, source);
                    }
                }
                int headphoneSource =
                    routing.value(QStringLiteral("headphoneSource")).toInt(0);
                if (format == QStringLiteral("flow8-simulator-snapshot-v1")) {
                    // v1 stored MAIN/MON1/MON2. The APK-confirmed model is
                    // MAIN/MON, so both legacy monitor values collapse safely.
                    headphoneSource = headphoneSource == 0 ? 0 : 1;
                }
                if (headphoneSource >= 0 && headphoneSource <= 1) {
                    (void)state_.setHeadphoneSource(
                        static_cast<model::HeadphoneSource>(headphoneSource),
                        model::EvidenceStatus::Unknown, source);
                }
                if (routing.contains(QStringLiteral("headphoneTapPoint"))) {
                    const int tapPoint =
                        routing.value(QStringLiteral("headphoneTapPoint")).toInt(-1);
                    if (tapPoint >= 0 && tapPoint <= 1) {
                        (void)state_.setHeadphoneTapPoint(
                            static_cast<model::RoutingTapPoint>(tapPoint),
                            model::EvidenceStatus::Unknown, source);
                    }
                }
                (void)state_.setMonitorStereoLink(
                    routing.value(QStringLiteral("monitorStereoLink")).toBool(),
                    model::EvidenceStatus::Unknown, source);
                const auto monitorSources =
                    routing.value(QStringLiteral("monitorSources")).toArray();
                for (int monitor = 0; monitor < monitorSources.size() && monitor < 2;
                     ++monitor) {
                    const int routeSource = monitorSources.at(monitor).toInt(-1);
                    if (routeSource >= 0 && routeSource <= 2) {
                        (void)state_.setMonitorRouteSource(
                            monitor, static_cast<model::MonitorRouteSource>(routeSource),
                            model::EvidenceStatus::Unknown, source);
                    }
                }
                if (routing.contains(QStringLiteral("bluetoothUsbPhonesOnly"))) {
                    (void)state_.setBluetoothUsbPhonesOnly(
                        routing.value(QStringLiteral("bluetoothUsbPhonesOnly")).toBool(),
                        model::EvidenceStatus::Unknown, source);
                }
                const auto pads =
                    routing.value(QStringLiteral("outputPadsMinus10Dbv")).toArray();
                constexpr std::array outputIds {
                    model::PhysicalOutputId::MainOut,
                    model::PhysicalOutputId::MonitorOut1,
                    model::PhysicalOutputId::MonitorOut2,
                };
                for (int output = 0; output < pads.size()
                     && output < static_cast<int>(outputIds.size()); ++output) {
                    (void)state_.setOutputPadMinus10Dbv(
                        outputIds[static_cast<std::size_t>(output)],
                        pads.at(output).toBool(), model::EvidenceStatus::Unknown, source);
                }
                auto fxRoutes = routing.value(QStringLiteral("fxOutputRoutes")).toArray();
                if (fxRoutes.isEmpty()) {
                    // Compatibility with snapshots created before FX → MAIN was modelled.
                    const auto legacyRoutes =
                        routing.value(QStringLiteral("fxMonitorRoutes")).toArray();
                    for (int route = 0; route < legacyRoutes.size() && route < 4; ++route) {
                        (void)state_.setFxOutputRouteEnabled(
                            route / 2,
                            route % 2 == 0 ? model::FxOutputDestination::Monitor1
                                           : model::FxOutputDestination::Monitor2,
                            legacyRoutes.at(route).toBool(),
                            model::EvidenceStatus::Unknown, source);
                    }
                }
                for (int route = 0; route < fxRoutes.size() && route < 6; ++route) {
                    (void)state_.setFxOutputRouteEnabled(
                        route / 3,
                        static_cast<model::FxOutputDestination>(route % 3),
                        fxRoutes.at(route).toBool(),
                        model::EvidenceStatus::Unknown, source);
                }
            }
            return state_.setActiveSnapshotIndex(index);
        }
    }
    return false;
}

bool Flow8Device::renameAppSnapshot(const int libraryIndex, const QString& name)
{
    if (!isControlAvailable(Control::SnapshotStore) || libraryIndex < 0) {
        return false;
    }
    int current = 0;
    for (int index = 0; index < state_.snapshots().size(); ++index) {
        if (state_.snapshots().at(index).storage != model::SnapshotStorage::AppLibrary) {
            continue;
        }
        if (current++ == libraryIndex) {
            return state_.setSnapshotName(index, name, model::EvidenceStatus::Unknown,
                                          QString::fromLatin1(simulatorSource));
        }
    }
    return false;
}

bool Flow8Device::deleteAppSnapshot(const int libraryIndex)
{
    if (!isControlAvailable(Control::SnapshotStore) || libraryIndex < 0) {
        return false;
    }
    auto snapshots = state_.snapshots();
    int current = 0;
    for (int index = 0; index < snapshots.size(); ++index) {
        if (snapshots.at(index).storage != model::SnapshotStorage::AppLibrary) {
            continue;
        }
        if (current++ == libraryIndex) {
            snapshots.removeAt(index);
            int appIndex = 0;
            for (auto& snapshot : snapshots) {
                if (snapshot.storage == model::SnapshotStorage::AppLibrary) {
                    snapshot.index = appIndex++;
                }
            }
            state_.replaceSnapshots(std::move(snapshots));
            return true;
        }
    }
    return false;
}

bool Flow8Device::setUsbMode(const model::UsbMode mode)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "USB routing is unavailable."));
        return false;
    }
    return state_.setUsbMode(mode, model::EvidenceStatus::Unknown,
                             QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setUsbPlaybackAssignment(
    const int pairIndex, const model::UsbPlaybackAssignment assignment)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "USB routing is unavailable."));
        return false;
    }
    return state_.setUsbPlaybackAssignment(
        pairIndex, assignment, model::EvidenceStatus::Unknown,
        QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setMonitorRouteSource(
    const int monitorIndex, const model::MonitorRouteSource source)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "Monitor routing is unavailable."));
        return false;
    }
    return state_.setMonitorRouteSource(
        monitorIndex, source, model::EvidenceStatus::Unknown,
        QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setFxOutputRouteEnabled(
    const int effectIndex, const model::FxOutputDestination destination,
    const bool enabled)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "FX output routing is unavailable."));
        return false;
    }
    return state_.setFxOutputRouteEnabled(effectIndex, destination, enabled,
                                           model::EvidenceStatus::Unknown,
                                           QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setHeadphoneSource(const model::HeadphoneSource source)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "Headphone routing is unavailable."));
        return false;
    }
    return state_.setHeadphoneSource(source, model::EvidenceStatus::Unknown,
                                     QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setHeadphoneTapPoint(const model::RoutingTapPoint tapPoint)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "Headphone routing is unavailable."));
        return false;
    }
    return state_.setHeadphoneTapPoint(
        tapPoint, model::EvidenceStatus::Unknown,
        QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setBluetoothUsbPhonesOnly(const bool enabled)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "Bluetooth / USB routing is unavailable."));
        return false;
    }
    return state_.setBluetoothUsbPhonesOnly(
        enabled, model::EvidenceStatus::Unknown,
        QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setOutputPadMinus10Dbv(
    const model::PhysicalOutputId output, const bool enabled)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "Output routing is unavailable."));
        return false;
    }
    return state_.setOutputPadMinus10Dbv(
        output, enabled, model::EvidenceStatus::Unknown,
        QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setMonitorStereoLink(const bool linked)
{
    if (!isControlAvailable(Control::Routing)) {
        reject(Control::Routing, QCoreApplication::translate(
            "Flow8Device", "Monitor stereo link is unavailable."));
        return false;
    }
    return state_.setMonitorStereoLink(linked, model::EvidenceStatus::Unknown,
                                       QString::fromLatin1(simulatorSource));
}

void Flow8Device::setPreferences(model::AppPreferences preferences)
{
    state_.setPreferences(std::move(preferences));
}

bool Flow8Device::configureAssistedSetup(const model::InputId input,
                                          const model::AssistedSourceType sourceType)
{
    if (!isControlAvailable(Control::AssistedSetup)) {
        reject(Control::AssistedSetup, QCoreApplication::translate(
            "Flow8Device", "Assisted Setup requires Simulator mode or verified hardware support."));
        return false;
    }
    const int inputIndex = static_cast<int>(input);
    const auto* channel = state_.channel(inputIndex);
    if (channel == nullptr || input == model::InputId::UsbBluetooth) {
        return false;
    }
    model::AssistedSetupState setup;
    setup.step = model::AssistedSetupStep::Apply;
    setup.input = input;
    setup.sourceType = sourceType;
    setup.source = QString::fromLatin1(simulatorSource);
    switch (sourceType) {
    case model::AssistedSourceType::DynamicMicrophone:
        setup.recommendedPreset = QStringLiteral("dynamic_microphone");
        setup.connectionInstruction = QStringLiteral("connect_xlr");
        break;
    case model::AssistedSourceType::CondenserMicrophone:
        setup.recommendedPreset = QStringLiteral("condenser_microphone");
        setup.connectionInstruction = QStringLiteral("connect_xlr_phantom");
        break;
    case model::AssistedSourceType::LineInstrument:
        setup.recommendedPreset = QStringLiteral("line_instrument");
        setup.connectionInstruction = QStringLiteral("connect_line");
        break;
    case model::AssistedSourceType::GuitarBass:
        setup.recommendedPreset = QStringLiteral("guitar_bass");
        setup.connectionInstruction = QStringLiteral("connect_guitar_bass");
        break;
    }
    state_.setAssistedSetup(std::move(setup));
    return true;
}

bool Flow8Device::applyAssistedSetup()
{
    if (!isControlAvailable(Control::AssistedSetup)
        || !state_.assistedSetup().input.has_value()
        || !state_.assistedSetup().sourceType.has_value()) {
        return false;
    }
    auto setup = state_.assistedSetup();
    const int index = static_cast<int>(*setup.input);
    const auto icon = *setup.sourceType == model::AssistedSourceType::GuitarBass
        ? model::ChannelIcon::GuitarBass
        : (*setup.sourceType == model::AssistedSourceType::LineInstrument
            ? model::ChannelIcon::Instrument : model::ChannelIcon::Microphone);
    (void)state_.setChannelIcon(index, icon, model::EvidenceStatus::Unknown,
                                QString::fromLatin1(simulatorSource));
    if (state_.channel(index) != nullptr && state_.channel(index)->capabilities.lowCut) {
        (void)state_.setChannelLowCut(index, true, 80.0,
                                      model::EvidenceStatus::Unknown,
                                      QString::fromLatin1(simulatorSource));
    }
    setup.step = model::AssistedSetupStep::Complete;
    setup.applied = true;
    state_.setAssistedSetup(std::move(setup));
    return true;
}

bool Flow8Device::startEzGain(const QVector<model::InputId>& targets)
{
    if (!isControlAvailable(Control::EzGain) || targets.isEmpty()) {
        reject(Control::EzGain, QCoreApplication::translate(
            "Flow8Device", "EZ-GAIN requires Simulator mode or verified hardware support."));
        return false;
    }
    model::EzGainSession session;
    session.targets = targets;
    session.running = true;
    session.durationSeconds = 8;
    session.source = QString::fromLatin1(simulatorSource);
    state_.setEzGainSession(session);
    QTimer::singleShot(250, this, [this, targets] {
        if (!state_.ezGainSession().running) {
            return;
        }
        auto completed = state_.ezGainSession();
        completed.running = false;
        completed.results.clear();
        for (const auto input : targets) {
            const int index = static_cast<int>(input);
            if (state_.channel(index) == nullptr) {
                continue;
            }
            model::EzGainChannelResult result;
            result.input = input;
            result.signalDetected = simulatorBool(true);
            result.gain = simulatorDouble(std::clamp(0.42 + index * 0.03, 0.0, 1.0));
            result.headroomDb = simulatorDouble(12.0);
            completed.results.append(result);
            if (state_.channel(index)->capabilities.gain) {
                (void)state_.setChannelGain(index, *result.gain.value,
                                             model::EvidenceStatus::Unknown,
                                             QString::fromLatin1(simulatorSource));
            }
        }
        state_.setEzGainSession(std::move(completed));
    });
    return true;
}

void Flow8Device::cancelEzGain()
{
    auto session = state_.ezGainSession();
    session.running = false;
    session.cancelled = true;
    state_.setEzGainSession(std::move(session));
}

bool Flow8Device::setMainFader(const double normalized)
{
    if (!isControlAvailable(Control::MainFader)) {
        reject(Control::MainFader, QCoreApplication::translate(
            "Flow8Device", "Main fader BLE address is UNKNOWN."));
        return false;
    }
    return state_.setBusFader(0, normalized, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

bool Flow8Device::setMainMuted(const bool muted)
{
    if (!isControlAvailable(Control::MainMute)) {
        reject(Control::MainMute, QCoreApplication::translate(
            "Flow8Device", "Main mute mapping is UNKNOWN."));
        return false;
    }
    return state_.setBusMuted(0, muted, model::EvidenceStatus::Unknown,
                              QString::fromLatin1(simulatorSource));
}

void Flow8Device::handleTransportState(const Flow8Transport::State state)
{
    switch (state) {
    case Flow8Transport::State::Disconnected:
        simulatorMeterTimer_.stop();
        state_.setConnectionState(ConnectionState::Disconnected);
        break;
    case Flow8Transport::State::Scanning:
        state_.setConnectionState(ConnectionState::Scanning);
        break;
    case Flow8Transport::State::Connecting:
    case Flow8Transport::State::Reconnecting:
        state_.setConnectionState(ConnectionState::Connecting);
        break;
    case Flow8Transport::State::RequestingMtu:
    case Flow8Transport::State::DiscoveringServices:
        state_.setConnectionState(ConnectionState::Synchronizing);
        break;
    case Flow8Transport::State::WaitingForHandshake:
    case Flow8Transport::State::WaitingForHandshakeReply:
        state_.setConnectionState(ConnectionState::Authenticating);
        break;
    case Flow8Transport::State::Connected:
        state_.setConnectionState(ConnectionState::Connected);
        if (transport_ && transport_->isSimulator()) {
            initializeSimulatorProfile();
            state_.setConnectionState(ConnectionState::Ready);
        }
        break;
    case Flow8Transport::State::Error:
        simulatorMeterTimer_.stop();
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

    // The old reference project describes a different candidate payload for
    // 0x06. New APK/native evidence confirms the route/master semantic but not
    // its final wire schema, so no incoming 0x06 bytes are applied to state.
    // Raw observation remains available through protocolPacketObserved.
}

void Flow8Device::initializeSimulatorProfile()
{
    auto channels = model::createOfficialInputProfile();
    for (int index = 0; index < channels.size(); ++index) {
        auto& channel = channels[index];
        // Default input labels are rendered from InputId by the UI so they can
        // follow the selected language. This field is reserved for a device or
        // user supplied channel name and therefore remains unset here.
        if (channel.capabilities.gain) {
            channel.gain = simulatorDouble(0.5);
        }
        if (channel.phaseInverted.has_value()) {
            *channel.phaseInverted = simulatorBool(false);
        }
        channel.fader = simulatorDouble(std::max(0.38, 0.72 - index * 0.045));
        channel.icon = model::StateValue<model::ChannelIcon>::known(
            channel.inputType == model::InputType::UsbBluetooth
                ? model::ChannelIcon::Playback : model::ChannelIcon::Microphone,
            model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
        channel.visible = simulatorBool(true);
        channel.muted = simulatorBool(false);
        channel.soloed = simulatorBool(false);
        channel.pan = simulatorDouble(0.0);
        channel.meterLevel = simulatorDouble(0.0);
        channel.meterPeak = simulatorDouble(0.0);
        channel.clipping = simulatorBool(false);
        for (std::size_t band = 0; band < channel.eq.gainDb.size(); ++band) {
            channel.eq.gainDb[band] = simulatorDouble(0.0);
            channel.eq.frequencyHz[band] = simulatorDouble(
                model::defaultChannelEqFrequenciesHz[band]);
            channel.eq.q[band] = simulatorDouble(1.0);
        }
        if (channel.capabilities.compressor) {
            channel.compressor.amount = simulatorDouble(0.0);
            channel.compressor.thresholdDb = simulatorDouble(-18.0);
            channel.compressor.ratio = simulatorDouble(3.0);
            channel.compressor.attackMs = simulatorDouble(10.0);
            channel.compressor.releaseMs = simulatorDouble(120.0);
            channel.compressor.makeupGainDb = simulatorDouble(0.0);
            channel.compressor.gainReductionDb = simulatorDouble(0.0);
        }
        constexpr std::array<double, 4> initialSendDb {-10.0, -15.0, -8.0, -12.0};
        for (int send = 0; send < static_cast<int>(channel.sendLevelDb.size()); ++send) {
            const double levelDb = initialSendDb[static_cast<std::size_t>(send)] - index * 1.5;
            channel.sendLevelDb[static_cast<std::size_t>(send)] = simulatorDouble(levelDb);
            if (send < 2) {
                channel.monitorSends[static_cast<std::size_t>(send)].levelDb =
                    simulatorDouble(levelDb);
                channel.monitorSends[static_cast<std::size_t>(send)].mode =
                    model::StateValue<model::MonitorSendMode>::known(
                        model::MonitorSendMode::PostFader,
                        model::EvidenceStatus::Synthetic,
                        QString::fromLatin1(simulatorSource));
            } else {
                channel.fxSendLevelDb[static_cast<std::size_t>(send - 2)] =
                    simulatorDouble(levelDb);
            }
        }
        if (channel.lowCut.has_value()) {
            channel.lowCut->enabled = simulatorBool(false);
            channel.lowCut->frequencyHz = simulatorDouble(20.0);
        }
        if (channel.lowCutHz.has_value()) {
            *channel.lowCutHz = model::StateValue<quint16>::known(
                20, model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
        }
        if (channel.phantom48V.has_value()) {
            *channel.phantom48V = simulatorBool(false);
        }
    }
    state_.replaceChannels(std::move(channels));

    auto buses = model::createOfficialBusProfile();
    for (auto& bus : buses) {
        bus.fader = simulatorDouble(bus.busId == model::BusId::Main ? 0.75 : 0.65);
        if (bus.muted.has_value()) {
            *bus.muted = simulatorBool(false);
        }
        if (bus.balance.has_value()) {
            *bus.balance = simulatorDouble(0.0);
        }
        if (bus.limiterDb.has_value()) {
            *bus.limiterDb = simulatorDouble(-3.0);
        }
        if (bus.eq.has_value()) {
            for (std::size_t band = 0; band < bus.eq->gainDb.size(); ++band) {
                bus.eq->gainDb[band] = simulatorDouble(0.0);
                bus.eq->frequencyHz[band] = simulatorDouble(model::busEqFrequenciesHz[band]);
            }
        }
        if (bus.outputDelay.has_value()) {
            bus.outputDelay->enabled = simulatorBool(false);
            bus.outputDelay->milliseconds = simulatorDouble(0.0);
        }
    }
    state_.replaceBuses(std::move(buses));

    QVector<model::FxState> effects;
    effects.reserve(model::fxEngineCount);
    for (int index = 0; index < model::fxEngineCount; ++index) {
        model::FxState effect;
        effect.index = index;
        effect.master = simulatorDouble(index == 0 ? 0.68 : 0.64);
        effect.preset = model::StateValue<int>::known(
            1, model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
        effect.presetName = model::StateValue<QString>::known(
            QStringLiteral("Synthetic Preset 1"), model::EvidenceStatus::Synthetic,
            QString::fromLatin1(simulatorSource));
        effect.muted = simulatorBool(false);
        effect.tapTempoBpm = simulatorDouble(120.0);
        for (int parameter = 0; parameter < 2; ++parameter) {
            effect.parameters[static_cast<std::size_t>(parameter)].index = parameter;
            effect.parameters[static_cast<std::size_t>(parameter)].name =
                model::StateValue<QString>::known(
                    QStringLiteral("Parameter %1 (UNKNOWN)").arg(parameter + 1),
                    model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
            effect.parameters[static_cast<std::size_t>(parameter)].value = simulatorDouble(0.5);
        }
        effect.parameter1 = simulatorDouble(0.5);
        effect.parameter2 = simulatorDouble(0.5);
        effects.append(std::move(effect));
    }
    state_.replaceEffects(std::move(effects));
    (void)state_.setGlobalTempo(120.0, model::EvidenceStatus::Synthetic,
                                QString::fromLatin1(simulatorSource));

    auto snapshots = model::createHardwareSnapshotProfile();
    for (auto& snapshot : snapshots) {
        // Empty-slot text is UI terminology, not simulated device state.
        snapshot.timestamp = model::StateValue<QDateTime>::known(
            QDateTime::currentDateTimeUtc(), model::EvidenceStatus::Synthetic,
            QString::fromLatin1(simulatorSource));
        snapshot.scope = model::StateValue<model::SnapshotScope>::known(
            model::SnapshotScope::Full, model::EvidenceStatus::Synthetic,
            QString::fromLatin1(simulatorSource));
    }
    state_.replaceSnapshots(std::move(snapshots));

    auto routing = model::createRoutingProfile();
    for (auto& cell : routing.routeLevels.cells) {
        const auto inputIndex = model::inputIndexForEndpoint(cell.sourceEndpoint);
        const auto destination = model::destinationForEndpoint(cell.destinationEndpoint);
        if (!inputIndex.has_value() || !destination.has_value()) {
            continue;
        }
        const auto& channel = state_.channels().at(*inputIndex);
        switch (*destination) {
        case model::RoutingDestination::Main:
            cell.confirmed = channel.fader;
            break;
        case model::RoutingDestination::Monitor1:
        case model::RoutingDestination::Monitor2: {
            const int monitor = static_cast<int>(*destination) - 1;
            cell.confirmed = simulatorDouble(dbToNormalized(
                channel.monitorSends[static_cast<std::size_t>(monitor)]
                    .levelDb.value.value_or(-144.0)));
            break;
        }
        case model::RoutingDestination::Fx1:
        case model::RoutingDestination::Fx2: {
            const int effect = static_cast<int>(*destination) - 3;
            cell.confirmed = simulatorDouble(dbToNormalized(
                channel.fxSendLevelDb[static_cast<std::size_t>(effect)]
                    .value.value_or(-144.0)));
            break;
        }
        }
    }
    routing.usb.mode = model::StateValue<model::UsbMode>::known(
        model::UsbMode::Streaming, model::EvidenceStatus::Synthetic,
        QString::fromLatin1(simulatorSource));
    routing.usb.input56Source =
        model::StateValue<model::UsbPlaybackAssignment>::known(
            model::UsbPlaybackAssignment::AnalogInput,
            model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
    routing.usb.input78Source =
        model::StateValue<model::UsbPlaybackAssignment>::known(
            model::UsbPlaybackAssignment::AnalogInput,
            model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
    for (auto& source : routing.monitor.outputSources) {
        source = model::StateValue<model::MonitorRouteSource>::known(
            model::MonitorRouteSource::MonitorMix,
            model::EvidenceStatus::Synthetic, QString::fromLatin1(simulatorSource));
    }
    for (auto& route : routing.fxOutputRoutes) {
        const bool enabled = route.destination == model::FxOutputDestination::Main
            || (route.effectIndex == 0
                && route.destination == model::FxOutputDestination::Monitor1)
            || (route.effectIndex == 1
                && route.destination == model::FxOutputDestination::Monitor2);
        route.enabled = simulatorBool(enabled);
    }
    routing.headphones.source = model::StateValue<model::HeadphoneSource>::known(
        model::HeadphoneSource::Main, model::EvidenceStatus::Synthetic,
        QString::fromLatin1(simulatorSource));
    routing.headphones.tapPoint = model::StateValue<model::RoutingTapPoint>::known(
        model::RoutingTapPoint::PostFader, model::EvidenceStatus::Synthetic,
        QString::fromLatin1(simulatorSource));
    routing.headphones.bluetoothUsbPhonesOnly = simulatorBool(false);
    routing.monitor.stereoLinked = simulatorBool(false);
    state_.replaceRouting(std::move(routing));

    auto outputs = model::createPhysicalOutputProfile();
    for (auto& output : outputs) {
        if (output.padMinus10Dbv.has_value()) {
            *output.padMinus10Dbv = simulatorBool(false);
        }
    }
    state_.replacePhysicalOutputs(std::move(outputs));

    model::AssistedSetupState setup;
    setup.source = QString::fromLatin1(simulatorSource);
    state_.setAssistedSetup(std::move(setup));
    model::EzGainSession ezGain;
    ezGain.source = QString::fromLatin1(simulatorSource);
    state_.setEzGainSession(std::move(ezGain));

    simulatorMeterStep_ = 0;
    simulatorMeterTimer_.start();
}

void Flow8Device::updateSimulatorMeters()
{
    if (!simulatorReady()) {
        simulatorMeterTimer_.stop();
        return;
    }
    const double time = static_cast<double>(simulatorMeterStep_++) * 0.09;
    for (int index = 0; index < state_.channels().size(); ++index) {
        const double envelope = 0.16 + 0.62
            * std::abs(std::sin(time * (0.65 + index * 0.035) + index * 0.71));
        const double transient = std::max(0.0, std::sin(time * 2.7 + index)) * 0.12;
        const double level = std::min(1.0, envelope + transient);
        const double peak = std::min(1.0, level + 0.08);
        (void)state_.setChannelMeter(index, level, peak, level > 0.985,
                                     model::EvidenceStatus::Synthetic,
                                     QString::fromLatin1(simulatorSource));
    }
    for (int destination = 0; destination < model::mixerDestinationCount; ++destination) {
        double master = 0.0;
        if (destination < model::mixBusCount) {
            const auto* bus = state_.bus(destination);
            master = bus == nullptr ? 0.0 : bus->fader.value.value_or(0.0);
        } else {
            const int effectIndex = destination - model::mixBusCount;
            if (effectIndex >= 0 && effectIndex < state_.effects().size()) {
                master = state_.effects().at(effectIndex).master.value.value_or(0.0);
            }
        }
        const double motion = 0.70 + 0.22 * std::abs(std::sin(
            time * (0.48 + destination * 0.04) + destination));
        const double level = std::clamp(master * motion, 0.0, 1.0);
        const double peak = std::min(1.0, level + 0.06);
        (void)state_.setOutputMeter(
            static_cast<model::RoutingDestination>(destination), level, peak,
            level > 0.985, model::EvidenceStatus::Synthetic,
            QString::fromLatin1(simulatorSource));
    }
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
