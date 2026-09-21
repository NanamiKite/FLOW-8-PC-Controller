#include "core/flow8_state.h"

#include <cmath>
#include <utility>

namespace flow8 {

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
    target->fader = model::StateValue<double>::known(normalized, evidence, source);
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
    target->gain = model::StateValue<double>::known(normalized, evidence, source);
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
    target->muted = model::StateValue<bool>::known(muted, evidence, source);
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
    target->soloed = model::StateValue<bool>::known(soloed, evidence, source);
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
    target->pan = model::StateValue<double>::known(pan, evidence, source);
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
    main_.fader = model::StateValue<double>::known(normalized, evidence, source);
    emit mainChanged();
    return true;
}

bool Flow8State::setMainMuted(const bool muted, const model::EvidenceStatus evidence,
                              const QString& source)
{
    main_.muted = model::StateValue<bool>::known(muted, evidence, source);
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

model::ChannelState* Flow8State::mutableChannel(const int index) noexcept
{
    return index >= 0 && index < channels_.size() ? &channels_[index] : nullptr;
}

bool Flow8State::isUnitInterval(const double value) noexcept
{
    return std::isfinite(value) && value >= 0.0 && value <= 1.0;
}

} // namespace flow8
