#include "core/flow8_state.h"

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
    emit stateReset();
}

bool Flow8State::setChannelFader(const int index, const double normalized,
                                 const model::EvidenceStatus evidence, const QString& source)
{
    auto* target = mutableChannel(index);
    if (target == nullptr || !isUnitInterval(normalized)) {
        return false;
    }
    if (!model::mergeObservedValue(target->fader, normalized, evidence, source)) {
        return false;
    }
    emit channelChanged(index);
    return true;
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

const QVector<model::BusState>& Flow8State::buses() const noexcept
{
    return buses_;
}

void Flow8State::replaceBuses(QVector<model::BusState> buses)
{
    buses_ = std::move(buses);
    emit stateReset();
}

const model::MainState& Flow8State::main() const noexcept
{
    return main_;
}

bool Flow8State::setMainFader(const double normalized, const model::EvidenceStatus evidence,
                              const QString& source)
{
    if (!isUnitInterval(normalized)) {
        return false;
    }
    if (!model::mergeObservedValue(main_.fader, normalized, evidence, source)) {
        return false;
    }
    emit mainChanged();
    return true;
}

bool Flow8State::setMainMuted(const bool muted, const model::EvidenceStatus evidence,
                              const QString& source)
{
    if (!model::mergeObservedValue(main_.muted, muted, evidence, source)) {
        return false;
    }
    emit mainChanged();
    return true;
}

const QVector<model::MonitorState>& Flow8State::monitors() const noexcept
{
    return monitors_;
}

void Flow8State::replaceMonitors(QVector<model::MonitorState> monitors)
{
    monitors_ = std::move(monitors);
    emit stateReset();
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

const QVector<model::SnapshotState>& Flow8State::snapshots() const noexcept
{
    return snapshots_;
}

void Flow8State::replaceSnapshots(QVector<model::SnapshotState> snapshots)
{
    snapshots_ = std::move(snapshots);
    emit stateReset();
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
                    && std::trunc(parameter.value) == parameter.value;
                if (validValue) {
                    mergeCounted(channel.lowCutHz, static_cast<quint16>(parameter.value),
                                 parameter.evidence, parameter.source, applied, rejected);
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
                if (validValue) mergeCounted(channel.sendLevelDb[static_cast<std::size_t>(send)],
                                             parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
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
                validValue = inRange(parameter.value, -1.0, 1.0);
                if (validValue) mergeCounted(bus.balance, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 3 && parts[2] == QStringLiteral("limiter_db")) {
                recognized = true;
                validValue = inRange(parameter.value, -30.0, 0.0);
                if (validValue) mergeCounted(bus.limiterDb, parameter.value, parameter.evidence,
                                             parameter.source, applied, rejected);
            } else if (parts.size() == 4 && parts[2] == QStringLiteral("graphic_eq")) {
                static const QStringList bands {
                    QStringLiteral("62hz_gain_db"), QStringLiteral("125hz_gain_db"),
                    QStringLiteral("250hz_gain_db"), QStringLiteral("500hz_gain_db"),
                    QStringLiteral("1khz_gain_db"), QStringLiteral("2khz_gain_db"),
                    QStringLiteral("4khz_gain_db"), QStringLiteral("8khz_gain_db"),
                    QStringLiteral("16khz_gain_db")};
                const qsizetype band = bands.indexOf(parts[3]);
                recognized = band >= 0 && *index < 3;
                validValue = recognized && inRange(parameter.value, -15.0, 15.0);
                if (validValue) mergeCounted(bus.eq.gainDb[static_cast<std::size_t>(band)],
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
            mergeCounted(channel.phantom48V, flag.value, flag.evidence, flag.source,
                         applied, rejected);
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
    if (channels_.size() < 7) {
        const qsizetype oldSize = channels_.size();
        channels_.resize(7);
        for (qsizetype index = oldSize; index < channels_.size(); ++index) {
            channels_[index].index = static_cast<int>(index);
        }
    }
    if (buses_.size() < 5) {
        const qsizetype oldSize = buses_.size();
        buses_.resize(5);
        for (qsizetype index = oldSize; index < buses_.size(); ++index) {
            buses_[index].index = static_cast<int>(index);
        }
    }
    if (effects_.size() < 2) {
        const qsizetype oldSize = effects_.size();
        effects_.resize(2);
        for (qsizetype index = oldSize; index < effects_.size(); ++index) {
            effects_[index].index = static_cast<int>(index);
        }
    }
}

model::ChannelState* Flow8State::mutableChannel(const int index) noexcept
{
    return index >= 0 && index < channels_.size() ? &channels_[index] : nullptr;
}

bool Flow8State::isUnitInterval(const double value) noexcept
{
    return std::isfinite(value) && value >= 0.0 && value <= 1.0;
}

} // namespace flow8
