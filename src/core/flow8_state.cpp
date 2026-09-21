#include "core/flow8_state.h"

#include "model/flow8_capabilities.h"
#include "protocol/sysex.h"

#include <cmath>
#include <utility>

namespace flow8 {
namespace {

bool inRange(const double value, const double minimum, const double maximum) noexcept
{
    return std::isfinite(value) && value >= minimum && value <= maximum;
}

std::optional<qsizetype> pathIndex(const QStringList& parts, const qsizetype position,
                                   const qsizetype upperBound)
{
    if (position < 0 || position >= parts.size()) {
        return std::nullopt;
    }
    bool ok = false;
    const qsizetype value = parts[position].toLongLong(&ok);
    return ok && value >= 0 && value < upperBound ? std::optional(value) : std::nullopt;
}

template<typename T>
void mergeCounted(model::StateValue<T>& target, T value,
                  const model::EvidenceStatus evidence, const QString& source,
                  int& applied, int& rejected)
{
    if (model::mergeObservedValue(target, std::move(value), evidence, source)) {
        ++applied;
    } else {
        ++rejected;
    }
}

} // namespace

Flow8State::Flow8State(QObject* parent)
    : QObject(parent)
    , routing_(model::createRoutingProfile())
{
}

ConnectionState Flow8State::connectionState() const noexcept
{
    return connectionState_;
}

void Flow8State::setConnectionState(const ConnectionState state) noexcept
{
    if (connectionState_ == state) {
        return;
    }
    connectionState_ = state;
    emit connectionStateChanged(state);
}

const QVector<model::ChannelState>& Flow8State::channels() const noexcept
{
    return channels_;
}

const model::ChannelState* Flow8State::channel(const int index) const noexcept
{
    return index >= 0 && index < channels_.size() ? &channels_.at(index) : nullptr;
}

void Flow8State::replaceChannels(QVector<model::ChannelState> channels)
{
    channels_ = std::move(channels);
    meters_.inputs.clear();
    meters_.inputs.reserve(channels_.size());
    for (int index = 0; index < channels_.size(); ++index) {
        meters_.inputs.append(model::InputMeterState {
            .sourceIndex = index,
            .level = {},
            .peak = {},
            .clipping = {},
            .gainReductionDb = {},
        });
    }
    emit stateReset();
}

bool Flow8State::setChannelFader(const int index, const double normalized,
                                 const model::EvidenceStatus evidence, const QString& source)
{
    return setRouteLevel(index, model::RoutingDestination::Main,
                         normalized, evidence, source);
}

bool Flow8State::setChannelGain(const int index, const double normalized,
                                const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !isUnitInterval(normalized)) {
        return false;
    }
    if (!model::mergeObservedValue(target->gain, normalized, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelPhaseInverted(
    const int index, const bool inverted, const model::EvidenceStatus evidence,
    const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !target->capabilities.phase
        || !target->phaseInverted.has_value()
        || !model::mergeObservedValue(
            *target->phaseInverted, inverted, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelMuted(const int index, const bool muted,
                                 const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr) {
        return false;
    }
    if (!model::mergeObservedValue(target->muted, muted, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelSoloed(const int index, const bool soloed,
                                  const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr) {
        return false;
    }
    if (!model::mergeObservedValue(target->soloed, soloed, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelPan(const int index, const double pan,
                               const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !std::isfinite(pan) || pan < -1.0 || pan > 1.0) {
        return false;
    }
    if (!model::mergeObservedValue(target->pan, pan, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelName(const int index, const QString& name,
                                const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    const QString trimmed = name.trimmed();
    if (target == nullptr || trimmed.isEmpty() || trimmed.size() > 32
        || !model::mergeObservedValue(target->name, trimmed, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelIcon(const int index, const model::ChannelIcon icon,
                                const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr
        || !model::mergeObservedValue(target->icon, icon, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelVisible(const int index, const bool visible,
                                   const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr
        || !model::mergeObservedValue(target->visible, visible, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelPhantom(const int index, const bool enabled,
                                   const model::EvidenceStatus evidence,
                                   const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !target->capabilities.phantom48V
        || !target->phantom48V.has_value()
        || !model::mergeObservedValue(*target->phantom48V, enabled, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelLowCut(const int index, const bool enabled,
                                  const double frequencyHz,
                                  const model::EvidenceStatus evidence,
                                  const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !target->capabilities.lowCut || !target->lowCut.has_value()
        || !inRange(frequencyHz, 20.0, 600.0)) {
        return false;
    }
    const bool enabledApplied = model::mergeObservedValue(
        target->lowCut->enabled, enabled, evidence, source);
    const bool frequencyApplied = model::mergeObservedValue(
        target->lowCut->frequencyHz, frequencyHz, evidence, source);
    if (target->lowCutHz.has_value() && std::trunc(frequencyHz) == frequencyHz) {
        (void)model::mergeObservedValue(*target->lowCutHz,
                                       static_cast<quint16>(frequencyHz), evidence, source);
    }
    if (!enabledApplied && !frequencyApplied) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setMonitorSendMode(const int index, const int monitor,
                                    const model::MonitorSendMode mode,
                                    const model::EvidenceStatus evidence,
                                    const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || monitor < 0
        || monitor >= static_cast<int>(target->monitorSends.size())
        || !model::mergeObservedValue(
            target->monitorSends[static_cast<std::size_t>(monitor)].mode,
            mode, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelEqGain(const int index, const int band, const double gainDb,
                                  const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || band < 0 || band >= static_cast<int>(target->eq.gainDb.size())
        || !inRange(gainDb, -15.0, 15.0)) {
        return false;
    }
    if (!model::mergeObservedValue(target->eq.gainDb[static_cast<std::size_t>(band)], gainDb,
                                   evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelCompressorAmount(const int index, const double amount,
                                             const model::EvidenceStatus evidence,
                                             const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !isUnitInterval(amount)) {
        return false;
    }
    if (!model::mergeObservedValue(target->compressor.amount, amount, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
}

bool Flow8State::setChannelSendLevelDb(const int index, const int send, const double levelDb,
                                       const model::EvidenceStatus evidence,
                                       const QString& source)
{
    if (send < 0 || send >= 4 || !inRange(levelDb, -144.0, 10.0)) {
        return false;
    }
    const double normalized = levelDb <= -70.0 ? 0.0 : (levelDb + 70.0) / 80.0;
    return setRouteLevel(
        index, static_cast<model::RoutingDestination>(send + 1),
        normalized, evidence, source);
}

bool Flow8State::setChannelMeter(const int index, const double level, const double peak,
                                 const bool clipping, const model::EvidenceStatus evidence,
                                 const QString& source)
{
    if (index < 0 || index >= meters_.inputs.size()
        || !isUnitInterval(level) || !isUnitInterval(peak)) {
        return false;
    }
    auto& target = meters_.inputs[index];
    const bool levelApplied = model::mergeObservedValue(target.level, level, evidence, source);
    const bool peakApplied = model::mergeObservedValue(target.peak, peak, evidence, source);
    const bool clipApplied = model::mergeObservedValue(target.clipping, clipping, evidence, source);
    if (!levelApplied && !peakApplied && !clipApplied) {
        return false;
    }
    emit inputMeterChanged(index);
    return true;
}

const model::MeterState& Flow8State::meters() const noexcept
{
    return meters_;
}

const model::InputMeterState* Flow8State::inputMeter(const int index) const noexcept
{
    return index >= 0 && index < meters_.inputs.size()
        ? &meters_.inputs.at(index) : nullptr;
}

const model::OutputMeterState* Flow8State::outputMeter(
    const model::RoutingDestination destination) const noexcept
{
    for (const auto& meter : meters_.outputs) {
        if (meter.destination == destination) {
            return &meter;
        }
    }
    return nullptr;
}

bool Flow8State::setOutputMeter(
    const model::RoutingDestination destination, const double level,
    const double peak, const bool clipping, const model::EvidenceStatus evidence,
    const QString& source)
{
    if (!isUnitInterval(level) || !isUnitInterval(peak)) {
        return false;
    }
    for (auto& target : meters_.outputs) {
        if (target.destination != destination) {
            continue;
        }
        const bool levelApplied = model::mergeObservedValue(
            target.level, level, evidence, source);
        const bool peakApplied = model::mergeObservedValue(
            target.peak, peak, evidence, source);
        const bool clipApplied = model::mergeObservedValue(
            target.clipping, clipping, evidence, source);
        if (!levelApplied && !peakApplied && !clipApplied) {
            return false;
        }
        emit outputMeterChanged(destination);
        return true;
    }
    return false;
}

const QVector<model::BusState>& Flow8State::buses() const noexcept
{
    return buses_;
}

const model::BusState* Flow8State::bus(const int index) const noexcept
{
    return index >= 0 && index < buses_.size() ? &buses_.at(index) : nullptr;
}

void Flow8State::replaceBuses(QVector<model::BusState> buses)
{
    buses_ = std::move(buses);
    meters_.outputs.clear();
    meters_.outputs.reserve(buses_.size());
    for (int index = 0; index < buses_.size(); ++index) {
        meters_.outputs.append(model::OutputMeterState {
            .destination = static_cast<model::RoutingDestination>(index),
            .level = {},
            .peak = {},
            .clipping = {},
            .gainReductionDb = {},
        });
    }
    emit stateReset();
}

bool Flow8State::setBusFader(const int index, const double normalized,
                             const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableBus(index);
    if (target == nullptr || !isUnitInterval(normalized)
        || !model::mergeObservedValue(target->fader, normalized, evidence, source)) {
        return false;
    }
    emit busChanged(index);
    return true;
}

bool Flow8State::setBusMuted(const int index, const bool muted,
                             const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableBus(index);
    if (target == nullptr || !target->muted.has_value()
        || !model::mergeObservedValue(*target->muted, muted, evidence, source)) {
        return false;
    }
    emit busChanged(index);
    return true;
}

bool Flow8State::setBusBalance(const int index, const double balance,
                               const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableBus(index);
    if (target == nullptr || !target->balance.has_value() || !inRange(balance, -1.0, 1.0)
        || !model::mergeObservedValue(*target->balance, balance, evidence, source)) {
        return false;
    }
    emit busChanged(index);
    return true;
}

bool Flow8State::setBusLimiterDb(const int index, const double thresholdDb,
                                 const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableBus(index);
    if (target == nullptr || !target->limiterDb.has_value()
        || !inRange(thresholdDb, -30.0, 0.0)
        || !model::mergeObservedValue(*target->limiterDb, thresholdDb, evidence, source)) {
        return false;
    }
    emit busChanged(index);
    return true;
}

bool Flow8State::setBusEqGain(const int index, const int band, const double gainDb,
                              const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableBus(index);
    if (target == nullptr || !target->eq.has_value() || band < 0
        || band >= static_cast<int>(target->eq->gainDb.size())
        || !inRange(gainDb, -15.0, 15.0)
        || !model::mergeObservedValue(target->eq->gainDb[static_cast<std::size_t>(band)],
                                      gainDb, evidence, source)) {
        return false;
    }
    emit busChanged(index);
    return true;
}

const QVector<model::FxState>& Flow8State::effects() const noexcept
{
    return effects_;
}

void Flow8State::replaceEffects(QVector<model::FxState> effects)
{
    effects_ = std::move(effects);
    emit stateReset();
}

bool Flow8State::setFxPreset(const int index, const int preset,
                             const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableEffect(index);
    if (target == nullptr || preset < 1 || preset > 16
        || !model::mergeObservedValue(target->preset, preset, evidence, source)) {
        return false;
    }
    emit effectChanged(index);
    return true;
}

bool Flow8State::setFxParameter(const int index, const int parameter, const double value,
                                const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableEffect(index);
    if (target == nullptr || parameter < 0
        || parameter >= static_cast<int>(target->parameters.size()) || !isUnitInterval(value)
        || !model::mergeObservedValue(target->parameters[static_cast<std::size_t>(parameter)].value,
                                      value, evidence, source)) {
        return false;
    }
    if (parameter == 0) {
        (void)model::mergeObservedValue(target->parameter1, value, evidence, source);
    } else {
        (void)model::mergeObservedValue(target->parameter2, value, evidence, source);
    }
    emit effectChanged(index);
    return true;
}

bool Flow8State::setFxMuted(const int index, const bool muted,
                            const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableEffect(index);
    if (target == nullptr
        || !model::mergeObservedValue(target->muted, muted, evidence, source)) {
        return false;
    }
    emit effectChanged(index);
    return true;
}

bool Flow8State::setFxTapTempo(const int index, const double bpm,
                               const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableEffect(index);
    if (target == nullptr || !inRange(bpm, 50.0, 250.0)
        || !model::mergeObservedValue(target->tapTempoBpm, bpm, evidence, source)) {
        return false;
    }
    emit effectChanged(index);
    return true;
}

const model::GlobalTempoState& Flow8State::globalTempo() const noexcept
{
    return globalTempo_;
}

bool Flow8State::setGlobalTempo(const double bpm, const model::EvidenceStatus evidence,
                                const QString& source)
{
    if (!inRange(bpm, 50.0, 250.0)
        || !model::mergeObservedValue(globalTempo_.bpm, bpm, evidence, source)) {
        return false;
    }
    emit globalTempoChanged();
    return true;
}

const QVector<model::SnapshotState>& Flow8State::snapshots() const noexcept
{
    return snapshots_;
}

void Flow8State::replaceSnapshots(QVector<model::SnapshotState> snapshots)
{
    snapshots_ = std::move(snapshots);
    if (activeSnapshotIndex_ >= snapshots_.size()) {
        activeSnapshotIndex_ = -1;
    }
    emit stateReset();
}

int Flow8State::activeSnapshotIndex() const noexcept
{
    return activeSnapshotIndex_;
}

bool Flow8State::setActiveSnapshotIndex(const int index)
{
    if (index < 0 || index >= snapshots_.size()) {
        return false;
    }
    activeSnapshotIndex_ = index;
    emit snapshotChanged(index);
    return true;
}

bool Flow8State::setSnapshotName(const int index, const QString& name,
                                 const model::EvidenceStatus evidence,
                                 const QString& source)
{
    const QString trimmed = name.trimmed();
    if (index < 0 || index >= snapshots_.size() || trimmed.isEmpty()
        || trimmed.size() > 64
        || !model::mergeObservedValue(snapshots_[index].name, trimmed, evidence, source)) {
        return false;
    }
    emit snapshotChanged(index);
    return true;
}

const model::RoutingState& Flow8State::routing() const noexcept
{
    return routing_;
}

const model::RouteLevelState* Flow8State::routeLevel(
    const int sourceIndex, const model::RoutingDestination destination) const noexcept
{
    return routing_.routeLevels.level(sourceIndex, destination);
}

void Flow8State::replaceRouting(model::RoutingState routing)
{
    routing_ = std::move(routing);
    emit routingChanged();
}

bool Flow8State::setRouteLevel(const int sourceIndex,
                               const model::RoutingDestination destination,
                               const double normalized,
                               const model::EvidenceStatus evidence,
                               const QString& source)
{
    auto* cell = mutableRouteLevel(sourceIndex, destination);
    auto* channel = mutableChannel(sourceIndex);
    if (cell == nullptr || channel == nullptr || !isUnitInterval(normalized)
        || !model::mergeObservedValue(
            cell->confirmed, normalized, evidence, source)) {
        return false;
    }
    cell->pending.reset();
    cell->error.clear();

    if (destination == model::RoutingDestination::Main) {
        (void)model::mergeObservedValue(channel->fader, normalized, evidence, source);
    } else {
        const int send = static_cast<int>(destination) - 1;
        const double levelDb = normalized <= 0.0 ? -144.0 : -70.0 + normalized * 80.0;
        (void)model::mergeObservedValue(
            channel->sendLevelDb[static_cast<std::size_t>(send)],
            levelDb, evidence, source);
        if (send < 2) {
            (void)model::mergeObservedValue(
                channel->monitorSends[static_cast<std::size_t>(send)].levelDb,
                levelDb, evidence, source);
        } else {
            (void)model::mergeObservedValue(
                channel->fxSendLevelDb[static_cast<std::size_t>(send - 2)],
                levelDb, evidence, source);
        }
    }
    emit channelChanged(sourceIndex);
    emit routingChanged();
    return true;
}

bool Flow8State::setRouteLevelPending(
    const int sourceIndex, const model::RoutingDestination destination,
    const double normalized)
{
    auto* cell = mutableRouteLevel(sourceIndex, destination);
    if (cell == nullptr || !isUnitInterval(normalized)) {
        return false;
    }
    cell->pending = normalized;
    cell->error.clear();
    emit routingChanged();
    return true;
}

bool Flow8State::failRouteLevel(
    const int sourceIndex, const model::RoutingDestination destination,
    const QString& error)
{
    auto* cell = mutableRouteLevel(sourceIndex, destination);
    if (cell == nullptr || error.trimmed().isEmpty()) {
        return false;
    }
    cell->pending.reset();
    cell->error = error.trimmed();
    emit routingChanged();
    return true;
}

bool Flow8State::setRouteEnabled(const int inputIndex,
                                 const model::RoutingDestination destination,
                                 const bool enabled, const model::EvidenceStatus evidence,
                                 const QString& source)
{
    for (auto& route : routing_.routes) {
        if (route.inputIndex == inputIndex && route.destination == destination) {
            if (!model::mergeObservedValue(route.enabled, enabled, evidence, source)) {
                return false;
            }
            emit routingChanged();
            return true;
        }
    }
    return false;
}

bool Flow8State::setUsbMode(const model::UsbMode mode,
                            const model::EvidenceStatus evidence, const QString& source)
{
    if (!model::mergeObservedValue(routing_.usbMode, mode, evidence, source)) {
        return false;
    }
    emit routingChanged();
    return true;
}

bool Flow8State::setUsbRouteEnabled(const model::UsbRouteDestination destination,
                                    const bool enabled,
                                    const model::EvidenceStatus evidence,
                                    const QString& source)
{
    for (auto& route : routing_.usbRoutes) {
        if (route.destination == destination) {
            if (!model::mergeObservedValue(route.enabled, enabled, evidence, source)) {
                return false;
            }
            emit routingChanged();
            return true;
        }
    }
    return false;
}

bool Flow8State::setFxOutputRouteEnabled(
    const int effectIndex, const model::FxOutputDestination destination,
    const bool enabled, const model::EvidenceStatus evidence, const QString& source)
{
    for (auto& route : routing_.fxOutputRoutes) {
        if (route.effectIndex == effectIndex && route.destination == destination) {
            if (!model::mergeObservedValue(route.enabled, enabled, evidence, source)) {
                return false;
            }
            emit routingChanged();
            return true;
        }
    }
    return false;
}

bool Flow8State::setHeadphoneSource(const model::HeadphoneSource sourceValue,
                                    const model::EvidenceStatus evidence,
                                    const QString& source)
{
    if (!model::mergeObservedValue(routing_.headphoneSource, sourceValue,
                                   evidence, source)) {
        return false;
    }
    emit routingChanged();
    return true;
}

bool Flow8State::setMonitorStereoLink(const bool linked,
                                      const model::EvidenceStatus evidence,
                                      const QString& source)
{
    if (!model::mergeObservedValue(
            routing_.monitorLink.stereoLinked, linked, evidence, source)) {
        return false;
    }
    emit routingChanged();
    return true;
}

const model::AppPreferences& Flow8State::preferences() const noexcept
{
    return preferences_;
}

void Flow8State::setPreferences(model::AppPreferences preferences)
{
    preferences_ = std::move(preferences);
    emit preferencesChanged();
}

const model::AssistedSetupState& Flow8State::assistedSetup() const noexcept
{
    return assistedSetup_;
}

void Flow8State::setAssistedSetup(model::AssistedSetupState setup)
{
    assistedSetup_ = std::move(setup);
    emit assistedSetupChanged();
}

const model::EzGainSession& Flow8State::ezGainSession() const noexcept
{
    return ezGainSession_;
}

void Flow8State::setEzGainSession(model::EzGainSession session)
{
    ezGainSession_ = std::move(session);
    emit ezGainSessionChanged();
}

SysExApplyResult Flow8State::applySysExState(const protocol::ParsedSysExState& parsed)
{
    // Reparse the retained bytes so callers cannot inject decoded fields that
    // do not correspond to the packet being applied.
    const protocol::ParsedSysExState canonical =
        protocol::parseReferenceStateDump(parsed.raw);
    if (!canonical.validation.valid || !canonical.validation.isFlow8) {
        return {.reason = QStringLiteral("invalid or non-FLOW-8 SysEx framing")};
    }
    if (canonical.completeness != protocol::DumpCompleteness::CompleteReferenceLayout
        || canonical.raw.size() != protocol::referenceStateDumpByteCount) {
        return {.reason = QStringLiteral("partial dump is not safe to apply")};
    }

    ensureReferenceStateShape();
    int applied = 0;
    int rejected = 0;

    const QString nameSource = QStringLiteral(
        "reference/flow-8-midi/src/service/sysex_parser.rs:NAMES_START; NEED_HARDWARE");
    for (qsizetype index = 0;
         index < canonical.channelNames.size() && index < channels_.size();
         ++index) {
        if (canonical.channelNames[index].has_value()) {
            mergeCounted(channels_[index].name, *canonical.channelNames[index],
                         model::EvidenceStatus::Inferred, nameSource, applied, rejected);
        }
    }

    for (const auto& parameter : canonical.parameters) {
        const QStringList parts = parameter.path.split(QLatin1Char('.'));
        bool recognized = false;
        bool validValue = false;

        if (parts.size() >= 3 && parts[0] == QStringLiteral("channel")) {
            const auto index = pathIndex(parts, 1, channels_.size());
            if (!index.has_value()) {
                ++rejected;
                continue;
            }
            auto& channel = channels_[*index];
            if (parts.size() == 3 && parts[2] == QStringLiteral("level_db")) {
                recognized = true;
                validValue = inRange(parameter.value, -144.0, 10.0);
                if (validValue) mergeCounted(channel.levelDb, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("gain_db")) {
                recognized = true;
                validValue = inRange(parameter.value, -20.0, 60.0);
                if (validValue) mergeCounted(channel.gainDb, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("pan")) {
                recognized = true;
                validValue = inRange(parameter.value, -1.0, 1.0);
                if (validValue) mergeCounted(channel.pan, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("compressor")) {
                recognized = true;
                validValue = inRange(parameter.value, 0.0, 1.0);
                if (validValue) mergeCounted(channel.compressor.amount, parameter.value,
                                             parameter.evidence, parameter.source,
                                             applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("low_cut_hz")) {
                recognized = true;
                validValue = inRange(parameter.value, 20.0, 600.0)
                    && std::trunc(parameter.value) == parameter.value
                    && channel.lowCutHz.has_value();
                if (validValue) {
                    mergeCounted(*channel.lowCutHz, static_cast<quint16>(parameter.value),
                                 parameter.evidence, parameter.source, applied, rejected);
                    if (channel.lowCut.has_value()) {
                        (void)model::mergeObservedValue(
                            channel.lowCut->frequencyHz, parameter.value,
                            parameter.evidence, parameter.source);
                    }
                }
            } else if (parts.size() == 4 && parts[2] == QStringLiteral("eq")) {
                static const QStringList bands {
                    QStringLiteral("low_gain_db"), QStringLiteral("low_mid_gain_db"),
                    QStringLiteral("high_mid_gain_db"), QStringLiteral("high_gain_db")};
                const qsizetype band = bands.indexOf(parts[3]);
                recognized = band >= 0;
                validValue = recognized && inRange(parameter.value, -15.0, 15.0);
                if (validValue) mergeCounted(channel.eq.gainDb[static_cast<std::size_t>(band)],
                                             parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 4 && parts[2] == QStringLiteral("send")) {
                static const QStringList sends {
                    QStringLiteral("monitor1_level_db"), QStringLiteral("monitor2_level_db"),
                    QStringLiteral("fx1_level_db"), QStringLiteral("fx2_level_db")};
                const qsizetype send = sends.indexOf(parts[3]);
                recognized = send >= 0;
                validValue = recognized && inRange(parameter.value, -144.0, 10.0);
                if (validValue) {
                    mergeCounted(channel.sendLevelDb[static_cast<std::size_t>(send)],
                                 parameter.value, parameter.evidence,
                                 parameter.source, applied, rejected);
                    if (send < 2) {
                        (void)model::mergeObservedValue(
                            channel.monitorSends[static_cast<std::size_t>(send)].levelDb,
                            parameter.value, parameter.evidence, parameter.source);
                    } else {
                        (void)model::mergeObservedValue(
                            channel.fxSendLevelDb[static_cast<std::size_t>(send - 2)],
                            parameter.value, parameter.evidence, parameter.source);
                    }
                }
            }
        } else if (parts.size() >= 3 && parts[0] == QStringLiteral("bus")) {
            const auto index = pathIndex(parts, 1, buses_.size());
            if (!index.has_value()) {
                ++rejected;
                continue;
            }
            auto& bus = buses_[*index];
            if (parts.size() == 3 && parts[2] == QStringLiteral("level_db")) {
                recognized = true;
                validValue = inRange(parameter.value, -144.0, 10.0);
                if (validValue) mergeCounted(bus.levelDb, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("balance")) {
                recognized = true;
                validValue = bus.balance.has_value() && inRange(parameter.value, -1.0, 1.0);
                if (validValue) mergeCounted(*bus.balance, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("limiter_db")) {
                recognized = true;
                validValue = bus.limiterDb.has_value() && inRange(parameter.value, -30.0, 0.0);
                if (validValue) mergeCounted(*bus.limiterDb, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 4 && parts[2] == QStringLiteral("graphic_eq")) {
                static const QStringList bands {
                    QStringLiteral("62hz_gain_db"), QStringLiteral("125hz_gain_db"),
                    QStringLiteral("250hz_gain_db"), QStringLiteral("500hz_gain_db"),
                    QStringLiteral("1khz_gain_db"), QStringLiteral("2khz_gain_db"),
                    QStringLiteral("4khz_gain_db"), QStringLiteral("8khz_gain_db"),
                    QStringLiteral("16khz_gain_db")};
                const qsizetype band = bands.indexOf(parts[3]);
                recognized = band >= 0 && bus.eq.has_value();
                validValue = recognized && inRange(parameter.value, -15.0, 15.0);
                if (validValue) mergeCounted(bus.eq->gainDb[static_cast<std::size_t>(band)],
                                             parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            }
        } else if (parts.size() == 3 && parts[0] == QStringLiteral("fx")) {
            const auto index = pathIndex(parts, 1, effects_.size());
            if (!index.has_value()) {
                ++rejected;
                continue;
            }
            auto& effect = effects_[*index];
            if (parts[2] == QStringLiteral("parameter1_percent")) {
                recognized = true;
                validValue = inRange(parameter.value, 0.0, 100.0);
                if (validValue) mergeCounted(effect.parameter1Percent, parameter.value,
                                             parameter.evidence, parameter.source,
                                             applied, rejected);
            } else if (parts[2] == QStringLiteral("parameter2_percent")) {
                recognized = true;
                validValue = inRange(parameter.value, 0.0, 100.0);
                if (validValue) mergeCounted(effect.parameter2Percent, parameter.value,
                                             parameter.evidence, parameter.source,
                                             applied, rejected);
            } else if (parts[2] == QStringLiteral("preset_reference_index")) {
                recognized = true;
                validValue = inRange(parameter.value, 0.0, 15.0)
                    && std::trunc(parameter.value) == parameter.value;
                if (validValue) {
                    mergeCounted(effect.presetReferenceIndex, static_cast<int>(parameter.value),
                                 parameter.evidence, parameter.source, applied, rejected);
                }
            }
        }

        if (!recognized || !validValue) {
            ++rejected;
        }
    }

    for (const auto& flag : canonical.flags) {
        const QStringList parts = flag.path.split(QLatin1Char('.'));
        if (parts.size() != 3 || parts[0] != QStringLiteral("channel")) {
            ++rejected;
            continue;
        }
        const auto index = pathIndex(parts, 1, channels_.size());
        if (!index.has_value()) {
            ++rejected;
            continue;
        }
        auto& channel = channels_[*index];
        if (parts[2] == QStringLiteral("muted")) {
            mergeCounted(channel.muted, flag.value, flag.evidence, flag.source, applied, rejected);
        } else if (parts[2] == QStringLiteral("soloed")) {
            mergeCounted(channel.soloed, flag.value, flag.evidence, flag.source, applied, rejected);
        } else if (parts[2] == QStringLiteral("phantom_48v")) {
            if (channel.phantom48V.has_value()) {
                mergeCounted(*channel.phantom48V, flag.value, flag.evidence, flag.source,
                             applied, rejected);
            } else {
                ++rejected;
            }
        } else {
            ++rejected;
        }
    }

    if (applied > 0) {
        emit stateReset();
    }
    return {
        .applied = applied > 0,
        .fieldsApplied = applied,
        .fieldsRejected = rejected,
        .reason = applied > 0 ? QString() : QStringLiteral("no safe fields were applied"),
    };
}

void Flow8State::ensureReferenceStateShape()
{
    const auto inputProfile = model::createOfficialInputProfile();
    if (channels_.size() < inputProfile.size()) {
        const qsizetype oldSize = channels_.size();
        channels_.resize(inputProfile.size());
        for (qsizetype index = oldSize; index < channels_.size(); ++index) {
            channels_[index] = inputProfile[index];
        }
    }
    for (qsizetype index = 0; index < channels_.size() && index < inputProfile.size(); ++index) {
        auto& channel = channels_[index];
        const auto& profile = inputProfile[index];
        channel.index = profile.index;
        channel.inputId = profile.inputId;
        channel.inputType = profile.inputType;
        channel.spatialControl = profile.spatialControl;
        channel.stereoPair = profile.stereoPair;
        channel.capabilities = profile.capabilities;
        channel.defaultLabel = profile.defaultLabel;
        if (!channel.icon.value.has_value()) {
            channel.icon = profile.icon;
        }
        if (!channel.visible.value.has_value()) {
            channel.visible = profile.visible;
        }
        if (profile.lowCut.has_value() && !channel.lowCut.has_value()) {
            channel.lowCut.emplace();
        } else if (!profile.lowCut.has_value()) {
            channel.lowCut.reset();
        }
        if (profile.lowCutHz.has_value() && !channel.lowCutHz.has_value()) {
            channel.lowCutHz.emplace();
        } else if (!profile.lowCutHz.has_value()) {
            channel.lowCutHz.reset();
        }
        if (profile.phantom48V.has_value() && !channel.phantom48V.has_value()) {
            channel.phantom48V.emplace();
        } else if (!profile.phantom48V.has_value()) {
            channel.phantom48V.reset();
        }
        if (profile.phaseInverted.has_value() && !channel.phaseInverted.has_value()) {
            channel.phaseInverted.emplace();
        } else if (!profile.phaseInverted.has_value()) {
            channel.phaseInverted.reset();
        }
    }

    const auto busProfile = model::createOfficialBusProfile();
    if (buses_.size() < busProfile.size()) {
        const qsizetype oldSize = buses_.size();
        buses_.resize(busProfile.size());
        for (qsizetype index = oldSize; index < buses_.size(); ++index) {
            buses_[index] = busProfile[index];
        }
    }
    for (qsizetype index = 0; index < buses_.size() && index < busProfile.size(); ++index) {
        auto& bus = buses_[index];
        const auto& profile = busProfile[index];
        bus.index = profile.index;
        bus.busId = profile.busId;
        bus.capabilities = profile.capabilities;
        if (!bus.name.value.has_value()) {
            bus.name = profile.name;
        }
        if (profile.balance.has_value() && !bus.balance.has_value()) {
            bus.balance.emplace();
        } else if (!profile.balance.has_value()) {
            bus.balance.reset();
        }
        if (profile.muted.has_value() && !bus.muted.has_value()) {
            bus.muted.emplace();
        } else if (!profile.muted.has_value()) {
            bus.muted.reset();
        }
        if (profile.limiterDb.has_value() && !bus.limiterDb.has_value()) {
            bus.limiterDb.emplace();
        } else if (!profile.limiterDb.has_value()) {
            bus.limiterDb.reset();
        }
        if (profile.eq.has_value() && !bus.eq.has_value()) {
            bus.eq.emplace();
        } else if (!profile.eq.has_value()) {
            bus.eq.reset();
        }
        if (profile.outputDelay.has_value() && !bus.outputDelay.has_value()) {
            bus.outputDelay.emplace();
        } else if (!profile.outputDelay.has_value()) {
            bus.outputDelay.reset();
        }
    }
    if (effects_.size() < model::fxEngineCount) {
        const qsizetype oldSize = effects_.size();
        effects_.resize(model::fxEngineCount);
        for (qsizetype index = oldSize; index < effects_.size(); ++index) {
            effects_[index].index = static_cast<int>(index);
        }
    }
}

model::ChannelState* Flow8State::mutableChannel(const int index) noexcept
{
    return index >= 0 && index < channels_.size() ? &channels_[index] : nullptr;
}

model::BusState* Flow8State::mutableBus(const int index) noexcept
{
    return index >= 0 && index < buses_.size() ? &buses_[index] : nullptr;
}

model::FxState* Flow8State::mutableEffect(const int index) noexcept
{
    return index >= 0 && index < effects_.size() ? &effects_[index] : nullptr;
}

model::RouteLevelState* Flow8State::mutableRouteLevel(
    const int sourceIndex, const model::RoutingDestination destination) noexcept
{
    for (auto& cell : routing_.routeLevels.cells) {
        if (cell.sourceIndex == sourceIndex && cell.destination == destination) {
            return &cell;
        }
    }
    return nullptr;
}

bool Flow8State::isUnitInterval(const double value) noexcept
{
    return std::isfinite(value) && value >= 0.0 && value <= 1.0;
}

} // namespace flow8
