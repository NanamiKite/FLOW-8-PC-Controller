#include "core/flow8_state.h"

#include "model/flow8_capabilities.h"
#include "protocol/flow8_command.h"
#include "protocol/route_level_codec.h"

#include <algorithm>
#include <cmath>
#include <QSignalBlocker>
#include <type_traits>
#include <utility>

namespace flow8 {
namespace {

template<class... Ts>
struct Overloaded : Ts... { using Ts::operator()...; };
template<class... Ts>
Overloaded(Ts...) -> Overloaded<Ts...>;

constexpr auto inputFlags = [](const quint8 flags, const int bit) {
    return (flags & static_cast<quint8>(1U << bit)) != 0;
};

std::optional<int> busIndexForEndpoint(const quint8 endpoint) noexcept
{
    switch (static_cast<model::EndpointId>(endpoint)) {
    case model::EndpointId::MainLr: return 0;
    case model::EndpointId::Monitor1: return 1;
    case model::EndpointId::Monitor2: return 2;
    default: return std::nullopt;
    }
}

std::optional<int> fxIndexForEndpoint(const quint8 endpoint) noexcept
{
    if (endpoint == static_cast<quint8>(model::EndpointId::Fx1)) return 0;
    if (endpoint == static_cast<quint8>(model::EndpointId::Fx2)) return 1;
    return std::nullopt;
}

std::optional<int> inputIndexForWireEndpoint(const quint8 endpoint) noexcept
{
    return model::inputIndexForEndpoint(static_cast<model::EndpointId>(endpoint));
}

std::optional<model::RoutingDestination> destinationForWireEndpoint(
    const quint8 endpoint) noexcept
{
    return model::destinationForEndpoint(static_cast<model::EndpointId>(endpoint));
}

double normalizedForFaderDb(const double db) noexcept
{
    const auto code = protocol::encodeRouteLevelFix8(static_cast<float>(db));
    return code.has_value() ? static_cast<double>(*code) / 255.0 : 0.0;
}

double normalizedMeter(const double db) noexcept
{
    if (!std::isfinite(db) || db <= -60.0) return 0.0;
    return std::clamp((db + 60.0) / 70.0, 0.0, 1.0);
}

template<typename T>
void mergeField(model::StateValue<T>& target, T value,
                const model::EvidenceStatus evidence, const QString& source,
                int& fieldsApplied)
{
    if (model::mergeObservedValue(
            target, std::move(value), evidence, source)) {
        ++fieldsApplied;
    }
}

bool isValidMixerStateShape(const protocol::MixerStateCommand& state)
{
    for (std::size_t index = 0; index < state.inputs.size(); ++index) {
        if (state.inputs[index].id != static_cast<quint8>(index)
            || state.inputs[index].label.endpoint != static_cast<quint8>(index)) {
            return false;
        }
    }
    constexpr std::array<quint8, 3> outputIds {0x0f, 0x0a, 0x0b};
    for (std::size_t index = 0; index < state.outputs.size(); ++index) {
        if (state.outputs[index].id != outputIds[index]) return false;
    }
    constexpr std::array<quint8, 2> fxIds {0x0c, 0x0d};
    for (std::size_t index = 0; index < state.effects.size(); ++index) {
        if (state.effects[index].id != fxIds[index]) return false;
    }
    return true;
}

} // namespace

bool Flow8State::applyInputProtocolState(
    const protocol::InputStateCommand& input,
    const model::EvidenceStatus evidence, const QString& source,
    int& fieldsApplied)
{
    const auto index = inputIndexForWireEndpoint(input.id);
    if (!index.has_value() || input.label.endpoint != input.id) return false;
    auto* channel = mutableChannel(*index);
    if (channel == nullptr) return false;

    mergeField(channel->muted, inputFlags(input.flags, 0), evidence, source, fieldsApplied);
    mergeField(channel->soloed, inputFlags(input.flags, 6), evidence, source, fieldsApplied);
    mergeField(channel->gainDb, input.gainDb, evidence, source, fieldsApplied);
    if (input.gainDb >= model::inputGainMinimumDb
        && input.gainDb <= model::inputGainMaximumDb) {
        mergeField(channel->gain,
                   model::normalizedInputGainFromDb(input.gainDb),
                   evidence, source, fieldsApplied);
    }
    mergeField(channel->compressor.amount, input.compressorAmount,
               evidence, source, fieldsApplied);
    mergeField(channel->pan, input.balance, evidence, source, fieldsApplied);
    mergeField(channel->leftConnected, inputFlags(input.flags, 4),
               evidence, source, fieldsApplied);
    mergeField(channel->rightConnected, inputFlags(input.flags, 5),
               evidence, source, fieldsApplied);
    if (channel->phaseInverted.has_value()) {
        mergeField(*channel->phaseInverted, inputFlags(input.flags, 2),
                   evidence, source, fieldsApplied);
    }
    if (channel->phantom48V.has_value()) {
        mergeField(*channel->phantom48V, inputFlags(input.flags, 3),
                   evidence, source, fieldsApplied);
    }
    if (channel->lowCut.has_value()) {
        mergeField(channel->lowCut->enabled, inputFlags(input.flags, 1),
                   evidence, source, fieldsApplied);
        mergeField(channel->lowCut->frequencyHz,
                   static_cast<double>(input.highPassFrequencyHz),
                   evidence, source, fieldsApplied);
    }
    if (channel->lowCutHz.has_value()) {
        mergeField(*channel->lowCutHz, input.highPassFrequencyHz,
                   evidence, source, fieldsApplied);
    }
    for (std::size_t band = 0; band < input.eqGainDb.size(); ++band) {
        mergeField(channel->eq.gainDb[band], input.eqGainDb[band],
                   evidence, source, fieldsApplied);
        mergeField(channel->eq.frequencyHz[band],
                   static_cast<double>(input.eqFrequencyHz[band]),
                   evidence, source, fieldsApplied);
        mergeField(channel->eq.q[band], input.eqQ[band],
                   evidence, source, fieldsApplied);
    }
    mergeField(channel->rawIconId, input.label.icon, evidence, source, fieldsApplied);
    mergeField(channel->name, QString::fromUtf8(input.label.text),
               evidence, source, fieldsApplied);
    emit channelChanged(*index);
    return true;
}

bool Flow8State::applyOutputProtocolState(
    const protocol::OutputStateCommand& output,
    const model::EvidenceStatus evidence, const QString& source,
    int& fieldsApplied)
{
    const auto busIndex = busIndexForEndpoint(output.id);
    const auto destination = destinationForWireEndpoint(output.id);
    if (!busIndex.has_value() || !destination.has_value()) return false;
    auto* bus = mutableBus(*busIndex);
    if (bus == nullptr) return false;

    mergeField(bus->levelDb, output.volumeDb, evidence, source, fieldsApplied);
    mergeField(bus->fader, normalizedForFaderDb(output.volumeDb),
               evidence, source, fieldsApplied);
    if (bus->muted.has_value()) {
        mergeField(*bus->muted, inputFlags(output.flags, 0),
                   evidence, source, fieldsApplied);
    }
    mergeField(bus->soloed, inputFlags(output.flags, 1),
               evidence, source, fieldsApplied);
    if (bus->balance.has_value()) {
        mergeField(*bus->balance, output.pan, evidence, source, fieldsApplied);
    }
    if (bus->limiterDb.has_value()) {
        mergeField(*bus->limiterDb, output.limiterDb,
                   evidence, source, fieldsApplied);
    }
    if (bus->eq.has_value()) {
        for (std::size_t band = 0; band < output.eqGainDb.size(); ++band) {
            mergeField(bus->eq->gainDb[band], output.eqGainDb[band],
                       evidence, source, fieldsApplied);
            mergeField(bus->eq->frequencyHz[band],
                       static_cast<double>(output.eqFrequencyHz[band]),
                       evidence, source, fieldsApplied);
            mergeField(bus->eq->q[band], output.eqQ[band],
                       evidence, source, fieldsApplied);
        }
    }
    if (bus->outputDelay.has_value()) {
        mergeField(bus->outputDelay->milliseconds,
                   static_cast<double>(output.delayTicks) / 48.0,
                   evidence, source, fieldsApplied);
        mergeField(bus->outputDelay->enabled, output.delayTicks != 0,
                   evidence, source, fieldsApplied);
    }
    for (int input = 0; input < static_cast<int>(output.inputGainsDb.size()); ++input) {
        (void)setRouteLevel(input, *destination,
                            normalizedForFaderDb(output.inputGainsDb[static_cast<std::size_t>(input)]),
                            evidence, source);
    }
    emit busChanged(*busIndex);
    return true;
}

bool Flow8State::applyFxProtocolState(
    const protocol::FxStateCommand& effect,
    const model::EvidenceStatus evidence, const QString& source,
    int& fieldsApplied)
{
    const auto fxIndex = fxIndexForEndpoint(effect.id);
    const auto destination = destinationForWireEndpoint(effect.id);
    if (!fxIndex.has_value() || !destination.has_value()) return false;
    auto* fx = mutableEffect(*fxIndex);
    if (fx == nullptr) return false;

    mergeField(fx->masterDb, effect.volumeDb, evidence, source, fieldsApplied);
    mergeField(fx->master, normalizedForFaderDb(effect.volumeDb),
               evidence, source, fieldsApplied);
    mergeField(fx->pan, effect.pan, evidence, source, fieldsApplied);
    mergeField(fx->muted, inputFlags(effect.flags, 0), evidence, source, fieldsApplied);
    mergeField(fx->preset, static_cast<int>(effect.preset),
               evidence, source, fieldsApplied);
    const std::array<quint8, 3> raw {
        effect.value1, effect.value2, effect.value3};
    for (std::size_t index = 0; index < raw.size(); ++index) {
        mergeField(fx->rawParameters[index], raw[index], evidence, source, fieldsApplied);
    }
    mergeField(fx->parameter1, static_cast<double>(effect.value1) / 100.0,
               evidence, source, fieldsApplied);
    mergeField(fx->parameter2, static_cast<double>(effect.value2),
               evidence, source, fieldsApplied);
    for (std::size_t index = 0; index < effect.auxGainsDb.size(); ++index) {
        mergeField(fx->auxGainsDb[index], effect.auxGainsDb[index],
                   evidence, source, fieldsApplied);
    }
    for (int input = 0; input < static_cast<int>(effect.inputGainsDb.size()); ++input) {
        (void)setRouteLevel(input, *destination,
                            normalizedForFaderDb(effect.inputGainsDb[static_cast<std::size_t>(input)]),
                            evidence, source);
    }
    constexpr std::array fxDestinations {
        model::FxOutputDestination::Main,
        model::FxOutputDestination::Monitor1,
        model::FxOutputDestination::Monitor2,
    };
    for (std::size_t bit = 0; bit < fxDestinations.size(); ++bit) {
        (void)setFxOutputRouteEnabled(
            *fxIndex, fxDestinations[bit], inputFlags(effect.flags, static_cast<int>(bit + 1)),
            evidence, source);
    }
    emit effectChanged(*fxIndex);
    return true;
}

bool Flow8State::markProtocolCommandPending(
    const protocol::Flow8Command& command)
{
    ensureReferenceStateShape();
    bool marked = false;
    const auto pending = [&marked]<typename T>(model::StateValue<T>& target, T value) {
        target.pending = std::move(value);
        target.error.clear();
        marked = true;
    };

    std::visit(Overloaded {
        [&](const protocol::PanCommand& value) {
            if (const auto input = inputIndexForWireEndpoint(value.endpoint)) {
                pending(mutableChannel(*input)->pan, value.pan);
                emit channelChanged(*input);
            } else if (const auto bus = busIndexForEndpoint(value.endpoint)) {
                auto* target = mutableBus(*bus);
                if (target && target->balance) pending(*target->balance, value.pan);
                emit busChanged(*bus);
            } else if (const auto fx = fxIndexForEndpoint(value.endpoint)) {
                pending(mutableEffect(*fx)->pan, value.pan);
                emit effectChanged(*fx);
            }
        },
        [&](const protocol::SoloCommand& value) {
            if (const auto input = inputIndexForWireEndpoint(value.endpoint)) {
                pending(mutableChannel(*input)->soloed, value.solo);
                emit channelChanged(*input);
            }
        },
        [&](const protocol::GainCommand& value) {
            if (const auto input = model::inputIndexForEndpoint(value.inputEndpoint)) {
                auto* target = mutableChannel(*input);
                pending(target->gainDb, value.gainDb);
                if (value.gainDb >= model::inputGainMinimumDb
                    && value.gainDb <= model::inputGainMaximumDb) {
                    pending(target->gain, model::normalizedInputGainFromDb(value.gainDb));
                }
                emit channelChanged(*input);
            }
        },
        [&](const protocol::GraphicEqCommand& value) {
            const auto bus = busIndexForEndpoint(value.endpoint);
            if (!bus || value.band >= 9) return;
            auto* target = mutableBus(*bus);
            if (!target || !target->eq) return;
            const auto band = static_cast<std::size_t>(value.band);
            pending(target->eq->frequencyHz[band], static_cast<double>(value.frequencyHz));
            pending(target->eq->q[band], value.q);
            pending(target->eq->gainDb[band], value.gainDb);
            emit busChanged(*bus);
        },
        [&](const protocol::HighPassFilterCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            if (!input) return;
            auto* target = mutableChannel(*input);
            if (!target || !target->lowCut) return;
            pending(target->lowCut->enabled, value.enabled);
            pending(target->lowCut->frequencyHz, static_cast<double>(value.frequencyHz));
            emit channelChanged(*input);
        },
        [&](const protocol::LabelCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.label.endpoint);
            if (!input) return;
            auto* target = mutableChannel(*input);
            pending(target->rawIconId, value.label.icon);
            pending(target->name, QString::fromUtf8(value.label.text));
            emit channelChanged(*input);
        },
        [&](const protocol::RouteLevelCommand& value) {
            const auto destination = model::destinationForEndpoint(value.destinationEndpoint);
            if (!destination) return;
            if (value.isDestinationMaster()) {
                if (const auto bus = busIndexForEndpoint(
                        static_cast<quint8>(value.destinationEndpoint))) {
                    pending(mutableBus(*bus)->fader, value.normalizedValue);
                    emit busChanged(*bus);
                } else if (const auto fx = fxIndexForEndpoint(
                               static_cast<quint8>(value.destinationEndpoint))) {
                    pending(mutableEffect(*fx)->master, value.normalizedValue);
                    emit effectChanged(*fx);
                }
            } else if (const auto input = model::inputIndexForEndpoint(value.sourceEndpoint)) {
                (void)setRouteLevelPending(*input, *destination, value.normalizedValue);
                marked = true;
            }
        },
        [&](const protocol::MuteCommand& value) {
            if (const auto input = inputIndexForWireEndpoint(value.endpoint)) {
                pending(mutableChannel(*input)->muted, value.muted);
                emit channelChanged(*input);
            } else if (const auto bus = busIndexForEndpoint(value.endpoint)) {
                auto* target = mutableBus(*bus);
                if (target && target->muted) pending(*target->muted, value.muted);
                emit busChanged(*bus);
            } else if (const auto fx = fxIndexForEndpoint(value.endpoint)) {
                pending(mutableEffect(*fx)->muted, value.muted);
                emit effectChanged(*fx);
            }
        },
        [&](const protocol::ParametricEqCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            if (!input || value.band >= 4) return;
            auto* target = mutableChannel(*input);
            const auto band = static_cast<std::size_t>(value.band);
            pending(target->eq.frequencyHz[band], static_cast<double>(value.frequencyHz));
            pending(target->eq.q[band], value.q);
            pending(target->eq.gainDb[band], value.gainDb);
            emit channelChanged(*input);
        },
        [&](const protocol::PhaseCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            if (!input) return;
            auto* target = mutableChannel(*input);
            if (target && target->phaseInverted) pending(*target->phaseInverted, value.inverted);
            emit channelChanged(*input);
        },
        [&](const protocol::FxSetupCommand& value) {
            const auto fx = fxIndexForEndpoint(value.endpoint);
            if (!fx) return;
            auto* target = mutableEffect(*fx);
            const std::array raw {value.value1, value.value2, value.value3};
            for (std::size_t index = 0; index < raw.size(); ++index) {
                pending(target->rawParameters[index], raw[index]);
            }
            constexpr std::array destinations {
                model::FxOutputDestination::Main,
                model::FxOutputDestination::Monitor1,
                model::FxOutputDestination::Monitor2,
            };
            for (std::size_t bit = 0; bit < destinations.size(); ++bit) {
                for (auto& route : routing_.fxOutputRoutes) {
                    if (route.effectIndex == *fx && route.destination == destinations[bit]) {
                        pending(route.enabled,
                                inputFlags(value.routeFlags, static_cast<int>(bit)));
                    }
                }
            }
            emit effectChanged(*fx);
            emit routingChanged();
        },
        [&](const protocol::CompressorCommand& value) {
            if (const auto input = inputIndexForWireEndpoint(value.endpoint)) {
                pending(mutableChannel(*input)->compressor.amount, value.amount);
                emit channelChanged(*input);
            }
        },
        [&](const protocol::LimiterCommand& value) {
            const auto bus = busIndexForEndpoint(value.endpoint);
            if (!bus) return;
            auto* target = mutableBus(*bus);
            if (target && target->limiterDb) pending(*target->limiterDb, value.thresholdDb);
            emit busChanged(*bus);
        },
        [&](const protocol::PhantomCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            if (!input) return;
            auto* target = mutableChannel(*input);
            if (target && target->phantom48V) pending(*target->phantom48V, value.enabled);
            emit channelChanged(*input);
        },
        [&](const protocol::SettingCommand& value) {
            if (value.data.isEmpty()) return;
            const bool flag = static_cast<quint8>(value.data.front()) != 0;
            switch (value.settingId) {
            case 0x01: pending(routing_.bluetoothUsbSwitch, flag); break;
            case 0x02: pending(routing_.headphones.bluetoothUsbPhonesOnly, flag); break;
            case 0x03: pending(routing_.footswitchFxMode, flag); break;
            case 0x05: pending(routing_.deviceName, QString::fromUtf8(value.data)); break;
            case 0x07: pending(routing_.usbStreaming, flag); break;
            case 0x08:
                pending(routing_.monitorRoutingCode,
                        static_cast<quint8>(value.data.front()));
                break;
            case 0x0b: pending(monitorLink_.stereoLinked, flag); break;
            case 0x0c:
                pending(routing_.headphones.source,
                        flag ? model::HeadphoneSource::Monitor
                             : model::HeadphoneSource::Main);
                break;
            case 0x0d:
                pending(routing_.headphones.tapPoint,
                        flag ? model::RoutingTapPoint::PostFader
                             : model::RoutingTapPoint::PreFader);
                break;
            case 0x0e:
                if (auto* output = mutablePhysicalOutput(model::PhysicalOutputId::MainOut);
                    output && output->padMinus10Dbv) {
                    pending(*output->padMinus10Dbv, flag);
                }
                break;
            case 0x0f:
                for (const auto id : {model::PhysicalOutputId::MonitorOut1,
                                      model::PhysicalOutputId::MonitorOut2}) {
                    if (auto* output = mutablePhysicalOutput(id);
                        output && output->padMinus10Dbv) {
                        pending(*output->padMinus10Dbv, flag);
                    }
                }
                break;
            case 0x10:
                pending(routing_.snapshotScopeBits,
                        static_cast<quint8>(value.data.front()));
                break;
            case 0x11: pending(routing_.monitorPostFader, flag); break;
            case 0xb0: pending(routing_.linkAppMixerSelection, flag); break;
            default: break;
            }
            emit monitorLinkChanged();
            emit routingChanged();
        },
        [&](const protocol::FxPresetCommand& value) {
            if (const auto fx = fxIndexForEndpoint(value.endpoint)) {
                pending(mutableEffect(*fx)->preset, static_cast<int>(value.preset));
                emit effectChanged(*fx);
            }
        },
        [&](const protocol::FxTempoCommand& value) {
            pending(globalTempo_.bpm, static_cast<double>(value.bpm));
            emit globalTempoChanged();
        },
        [&](const protocol::SelectOutputCommand& value) {
            const auto endpoint = static_cast<model::EndpointId>(value.endpoint);
            if (model::isDestinationEndpoint(endpoint)) {
                pending(routing_.deviceSelectedOutput, endpoint);
                emit routingChanged();
            }
        },
        [&](const protocol::ChannelDelayCommand& value) {
            const auto bus = busIndexForEndpoint(value.endpoint);
            if (!bus) return;
            auto* target = mutableBus(*bus);
            if (target && target->outputDelay) {
                pending(target->outputDelay->milliseconds,
                        static_cast<double>(value.ticks) / 48.0);
                pending(target->outputDelay->enabled, value.ticks != 0);
                emit busChanged(*bus);
            }
        },
        [&](const auto&) {},
    }, command);
    return marked;
}

void Flow8State::failAllPendingProtocolCommands(const QString& error)
{
    const QString reason = error.trimmed().isEmpty()
        ? QStringLiteral("protocol operation failed") : error.trimmed();
    const auto fail = [&reason]<typename T>(model::StateValue<T>& value) {
        if (!value.pending.has_value()) return;
        value.pending.reset();
        value.error = reason;
    };
    QSignalBlocker blocker(this);
    for (auto& channel : channels_) {
        fail(channel.name); fail(channel.rawIconId); fail(channel.gain);
        fail(channel.gainDb); fail(channel.muted); fail(channel.soloed); fail(channel.pan);
        if (channel.phaseInverted) fail(*channel.phaseInverted);
        if (channel.phantom48V) fail(*channel.phantom48V);
        if (channel.lowCut) {
            fail(channel.lowCut->enabled);
            fail(channel.lowCut->frequencyHz);
        }
        for (auto& value : channel.eq.gainDb) fail(value);
        for (auto& value : channel.eq.frequencyHz) fail(value);
        for (auto& value : channel.eq.q) fail(value);
        fail(channel.compressor.amount);
    }
    for (auto& bus : buses_) {
        fail(bus.fader); fail(bus.levelDb);
        if (bus.muted) fail(*bus.muted);
        if (bus.balance) fail(*bus.balance);
        if (bus.limiterDb) fail(*bus.limiterDb);
        if (bus.eq) {
            for (auto& value : bus.eq->gainDb) fail(value);
            for (auto& value : bus.eq->frequencyHz) fail(value);
            for (auto& value : bus.eq->q) fail(value);
        }
        if (bus.outputDelay) {
            fail(bus.outputDelay->enabled);
            fail(bus.outputDelay->milliseconds);
        }
    }
    for (auto& fx : effects_) {
        fail(fx.master); fail(fx.masterDb); fail(fx.pan); fail(fx.preset); fail(fx.muted);
        for (auto& value : fx.rawParameters) fail(value);
    }
    for (auto& route : routing_.routeLevels.cells) {
        if (route.pending.has_value()) {
            route.pending.reset();
            route.error = reason;
        }
    }
    for (auto& route : routing_.fxOutputRoutes) fail(route.enabled);
    fail(routing_.bluetoothUsbSwitch); fail(routing_.footswitchFxMode);
    fail(routing_.muteInputs); fail(routing_.usbStreaming);
    fail(routing_.monitorPostFader); fail(routing_.monitorRoutingCode);
    fail(routing_.snapshotScopeBits); fail(routing_.deviceSelectedOutput);
    fail(routing_.linkAppMixerSelection); fail(routing_.deviceName);
    fail(routing_.headphones.source); fail(routing_.headphones.tapPoint);
    fail(routing_.headphones.bluetoothUsbPhonesOnly);
    fail(monitorLink_.stereoLinked); fail(globalTempo_.bpm);
    for (auto& output : physicalOutputs_) {
        if (output.padMinus10Dbv) fail(*output.padMinus10Dbv);
    }
    blocker.unblock();
    emit stateReset();
}

ProtocolApplyResult Flow8State::applyProtocolCommand(
    const protocol::DecodedCommand& decoded,
    const model::EvidenceStatus originEvidence, const QString& source)
{
    if (originEvidence == model::EvidenceStatus::Blocked
        || originEvidence == model::EvidenceStatus::Unknown) {
        return {.reason = QStringLiteral("RX origin evidence must be explicit")};
    }
    ensureReferenceStateShape();
    int fieldsApplied = 0;
    bool composite = false;
    bool mixerStateReset = false;
    bool recognized = true;
    bool valid = true;

    std::visit(Overloaded {
        [&](const protocol::PanCommand& value) {
            if (const auto input = inputIndexForWireEndpoint(value.endpoint)) {
                valid = setChannelPan(*input, value.pan, originEvidence, source);
            } else if (const auto bus = busIndexForEndpoint(value.endpoint)) {
                auto* target = mutableBus(*bus);
                valid = target != nullptr;
                if (valid && target->balance.has_value()) {
                    mergeField(*target->balance, value.pan, originEvidence, source, fieldsApplied);
                    emit busChanged(*bus);
                }
            } else if (const auto fx = fxIndexForEndpoint(value.endpoint)) {
                auto* target = mutableEffect(*fx);
                valid = target != nullptr;
                if (valid) {
                    mergeField(target->pan, value.pan, originEvidence, source, fieldsApplied);
                    emit effectChanged(*fx);
                }
            } else valid = false;
        },
        [&](const protocol::SoloCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value()
                && setChannelSoloed(*input, value.solo, originEvidence, source);
        },
        [&](const protocol::GainCommand& value) {
            const auto input = model::inputIndexForEndpoint(value.inputEndpoint);
            if (!input.has_value()) { valid = false; return; }
            auto* channel = mutableChannel(*input);
            valid = channel != nullptr;
            if (valid) {
                mergeField(channel->gainDb, value.gainDb,
                           originEvidence, source, fieldsApplied);
                if (value.gainDb >= model::inputGainMinimumDb
                    && value.gainDb <= model::inputGainMaximumDb) {
                    mergeField(channel->gain,
                               model::normalizedInputGainFromDb(value.gainDb),
                               originEvidence, source, fieldsApplied);
                }
                emit channelChanged(*input);
            }
        },
        [&](const protocol::GraphicEqCommand& value) {
            const auto bus = busIndexForEndpoint(value.endpoint);
            valid = bus.has_value() && value.band < 9;
            if (!valid) return;
            auto* target = mutableBus(*bus);
            valid = target != nullptr && target->eq.has_value();
            if (!valid) return;
            const auto band = static_cast<std::size_t>(value.band);
            mergeField(target->eq->frequencyHz[band],
                       static_cast<double>(value.frequencyHz), originEvidence, source, fieldsApplied);
            mergeField(target->eq->q[band], value.q, originEvidence, source, fieldsApplied);
            mergeField(target->eq->gainDb[band], value.gainDb, originEvidence, source, fieldsApplied);
            emit busChanged(*bus);
        },
        [&](const protocol::HighPassFilterCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value()
                && setChannelLowCut(*input, value.enabled,
                                    value.frequencyHz, originEvidence, source);
        },
        [&](const protocol::LabelCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.label.endpoint);
            valid = input.has_value();
            if (!valid) return;
            auto* channel = mutableChannel(*input);
            mergeField(channel->rawIconId, value.label.icon,
                       originEvidence, source, fieldsApplied);
            mergeField(channel->name, QString::fromUtf8(value.label.text),
                       originEvidence, source, fieldsApplied);
            emit channelChanged(*input);
        },
        [&](const protocol::RouteLevelCommand&) { recognized = false; },
        [&](const protocol::RouteStateCommand& value) {
            const auto destination = destinationForWireEndpoint(value.destination);
            if (!destination.has_value()) { valid = false; return; }
            if (value.endpointA == value.destination) {
                if (const auto bus = busIndexForEndpoint(value.destination)) {
                    auto* target = mutableBus(*bus);
                    valid = target != nullptr;
                    if (valid) {
                        mergeField(target->levelDb, value.levelDb,
                                   originEvidence, source, fieldsApplied);
                        mergeField(target->fader, normalizedForFaderDb(value.levelDb),
                                   originEvidence, source, fieldsApplied);
                        emit busChanged(*bus);
                    }
                } else if (const auto fx = fxIndexForEndpoint(value.destination)) {
                    auto* target = mutableEffect(*fx);
                    valid = target != nullptr;
                    if (valid) {
                        mergeField(target->masterDb, value.levelDb,
                                   originEvidence, source, fieldsApplied);
                        mergeField(target->master, normalizedForFaderDb(value.levelDb),
                                   originEvidence, source, fieldsApplied);
                        emit effectChanged(*fx);
                    }
                } else valid = false;
            } else {
                const auto input = inputIndexForWireEndpoint(value.endpointA);
                valid = input.has_value()
                    && setRouteLevel(*input, *destination,
                                     normalizedForFaderDb(value.levelDb),
                                     originEvidence, source);
            }
        },
        [&](const protocol::MuteCommand& value) {
            if (const auto input = inputIndexForWireEndpoint(value.endpoint)) {
                valid = setChannelMuted(*input, value.muted, originEvidence, source);
            } else if (const auto bus = busIndexForEndpoint(value.endpoint)) {
                valid = setBusMuted(*bus, value.muted, originEvidence, source);
            } else if (const auto fx = fxIndexForEndpoint(value.endpoint)) {
                valid = setFxMuted(*fx, value.muted, originEvidence, source);
            } else valid = false;
        },
        [&](const protocol::ParametricEqCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value() && value.band < 4;
            if (!valid) return;
            auto* target = mutableChannel(*input);
            const auto band = static_cast<std::size_t>(value.band);
            mergeField(target->eq.frequencyHz[band], static_cast<double>(value.frequencyHz),
                       originEvidence, source, fieldsApplied);
            mergeField(target->eq.q[band], value.q, originEvidence, source, fieldsApplied);
            mergeField(target->eq.gainDb[band], value.gainDb,
                       originEvidence, source, fieldsApplied);
            emit channelChanged(*input);
        },
        [&](const protocol::PhaseCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value()
                && setChannelPhaseInverted(*input, value.inverted, originEvidence, source);
        },
        [&](const protocol::FxSetupCommand& value) {
            const auto index = fxIndexForEndpoint(value.endpoint);
            valid = index.has_value();
            if (!valid) return;
            auto* fx = mutableEffect(*index);
            const std::array raw {value.value1, value.value2, value.value3};
            for (std::size_t i = 0; i < raw.size(); ++i) {
                mergeField(fx->rawParameters[i], raw[i], originEvidence, source, fieldsApplied);
            }
            constexpr std::array destinations {
                model::FxOutputDestination::Main,
                model::FxOutputDestination::Monitor1,
                model::FxOutputDestination::Monitor2,
            };
            for (std::size_t bit = 0; bit < destinations.size(); ++bit) {
                (void)setFxOutputRouteEnabled(*index, destinations[bit],
                    inputFlags(value.routeFlags, static_cast<int>(bit)), originEvidence, source);
            }
            emit effectChanged(*index);
        },
        [&](const protocol::SnapshotDeleteCommand& value) {
            valid = value.slot < snapshots_.size();
            if (valid) {
                mergeField(snapshots_[value.slot].name, QString(), originEvidence, source, fieldsApplied);
                emit snapshotChanged(value.slot);
            }
        },
        [&](const protocol::CompressorCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value()
                && setChannelCompressorAmount(*input, value.amount, originEvidence, source);
        },
        [&](const protocol::LimiterCommand& value) {
            const auto bus = busIndexForEndpoint(value.endpoint);
            valid = bus.has_value()
                && setBusLimiterDb(*bus, value.thresholdDb, originEvidence, source);
        },
        [&](const protocol::PhantomCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value()
                && setChannelPhantom(*input, value.enabled, originEvidence, source);
        },
        [&](const protocol::InputStateCommand& value) {
            composite = true;
            valid = applyInputProtocolState(value, originEvidence, source, fieldsApplied);
        },
        [&](const protocol::OutputStateCommand& value) {
            composite = true;
            valid = applyOutputProtocolState(value, originEvidence, source, fieldsApplied);
        },
        [&](const protocol::SnapshotSaveCommand& value) {
            valid = value.slot < snapshots_.size();
            if (valid) {
                mergeField(snapshots_[value.slot].name, QString::fromUtf8(value.name),
                           originEvidence, source, fieldsApplied);
                activeSnapshotIndex_ = value.slot;
                emit snapshotChanged(value.slot);
            }
        },
        [&](const protocol::SnapshotLoadCommand& value) {
            valid = value.slot == 0x7f || value.slot < snapshots_.size();
            if (valid && value.slot != 0x7f) {
                activeSnapshotIndex_ = value.slot;
                emit snapshotChanged(value.slot);
                ++fieldsApplied;
            }
        },
        [&](const protocol::MeterUpdateCommand& value) {
            composite = true;
            const std::array<std::pair<int, int>, 7> meterSlots {{
                {0, 0}, {1, 1}, {2, 2}, {3, 3}, {4, 5}, {6, 7}, {8, 9}}};
            for (int input = 0; input < static_cast<int>(meterSlots.size()); ++input) {
                const auto [left, right] = meterSlots[static_cast<std::size_t>(input)];
                const auto leftIndex = static_cast<std::size_t>(left);
                const auto rightIndex = static_cast<std::size_t>(right);
                const double db = left == right ? value.metersDb[leftIndex]
                    : std::max(value.metersDb[leftIndex], value.metersDb[rightIndex]);
                if (input < meters_.inputs.size()) {
                    auto& meter = meters_.inputs[input];
                    mergeField(meter.level, normalizedMeter(db), originEvidence, source, fieldsApplied);
                    mergeField(meter.peak, normalizedMeter(db), originEvidence, source, fieldsApplied);
                    mergeField(meter.clipping, db >= 10.0, originEvidence, source, fieldsApplied);
                    const quint8 gr = static_cast<quint8>((value.gainReductionBits >> (input * 2)) & 0x03U);
                    mergeField(meter.gainReductionCode, gr, originEvidence, source, fieldsApplied);
                    emit inputMeterChanged(input);
                }
            }
            const auto selected = routing_.deviceSelectedOutput.value;
            if (selected.has_value()) {
                const auto destination = model::destinationForEndpoint(*selected);
                if (destination.has_value()) {
                    for (auto& meter : meters_.outputs) {
                        if (meter.destination != *destination) continue;
                        const double db = std::max(value.metersDb[10], value.metersDb[11]);
                        mergeField(meter.level, normalizedMeter(db), originEvidence, source, fieldsApplied);
                        mergeField(meter.peak, normalizedMeter(db), originEvidence, source, fieldsApplied);
                        mergeField(meter.clipping, db >= 10.0, originEvidence, source, fieldsApplied);
                        mergeField(meter.gainReductionCode,
                                   static_cast<quint8>((value.gainReductionBits >> 14U) & 0x03U),
                                   originEvidence, source, fieldsApplied);
                        emit outputMeterChanged(*destination);
                    }
                }
            }
        },
        [&](const protocol::SettingCommand& value) {
            if (value.data.isEmpty()) { valid = false; return; }
            const bool flag = static_cast<quint8>(value.data.front()) != 0;
            switch (value.settingId) {
            case 0x01: mergeField(routing_.bluetoothUsbSwitch, flag, originEvidence, source, fieldsApplied); break;
            case 0x02: valid = setBluetoothUsbPhonesOnly(flag, originEvidence, source); break;
            case 0x03: mergeField(routing_.footswitchFxMode, flag, originEvidence, source, fieldsApplied); break;
            case 0x05: mergeField(routing_.deviceName, QString::fromUtf8(value.data), originEvidence, source, fieldsApplied); break;
            case 0x07: mergeField(routing_.usbStreaming, flag, originEvidence, source, fieldsApplied); break;
            case 0x08: mergeField(routing_.monitorRoutingCode, static_cast<quint8>(value.data.front()), originEvidence, source, fieldsApplied); break;
            case 0x0b: valid = setMonitorStereoLink(flag, originEvidence, source); break;
            case 0x0c: valid = setHeadphoneSource(flag ? model::HeadphoneSource::Monitor : model::HeadphoneSource::Main, originEvidence, source); break;
            case 0x0d: valid = setHeadphoneTapPoint(flag ? model::RoutingTapPoint::PostFader : model::RoutingTapPoint::PreFader, originEvidence, source); break;
            case 0x0e: valid = setOutputPadMinus10Dbv(model::PhysicalOutputId::MainOut, flag, originEvidence, source); break;
            case 0x0f:
            {
                const bool out1 = setOutputPadMinus10Dbv(
                    model::PhysicalOutputId::MonitorOut1, flag, originEvidence, source);
                const bool out2 = setOutputPadMinus10Dbv(
                    model::PhysicalOutputId::MonitorOut2, flag, originEvidence, source);
                valid = out1 || out2;
                break;
            }
            case 0x10: mergeField(routing_.snapshotScopeBits, static_cast<quint8>(value.data.front()), originEvidence, source, fieldsApplied); break;
            case 0x11: mergeField(routing_.monitorPostFader, flag, originEvidence, source, fieldsApplied); break;
            case 0xb0: mergeField(routing_.linkAppMixerSelection, flag, originEvidence, source, fieldsApplied); break;
            case 0x04:
            case 0x06:
                recognized = false;
                break;
            default:
                // Known opaque preset records and future settings retain their
                // parsed envelope without inventing model semantics.
                recognized = false;
                break;
            }
            if (recognized) emit routingChanged();
        },
        [&](const protocol::FxStateCommand& value) {
            composite = true;
            valid = applyFxProtocolState(value, originEvidence, source, fieldsApplied);
        },
        [&](const protocol::FxPresetCommand& value) {
            const auto index = fxIndexForEndpoint(value.endpoint);
            valid = index.has_value()
                && setFxPreset(*index, value.preset, originEvidence, source);
        },
        [&](const protocol::ChannelConnectionCommand& value) {
            const auto input = inputIndexForWireEndpoint(value.endpoint);
            valid = input.has_value();
            if (!valid) return;
            auto* channel = mutableChannel(*input);
            if (value.subchannel == 0) {
                mergeField(channel->leftConnected, value.connected, originEvidence, source, fieldsApplied);
            } else {
                mergeField(channel->rightConnected, value.connected, originEvidence, source, fieldsApplied);
            }
            emit channelChanged(*input);
        },
        [&](const protocol::MixerStateCommand& value) {
            composite = true;
            if (!isValidMixerStateShape(value)) { valid = false; return; }
            // Validate first, then suppress all fine-grained intermediate
            // signals. Observers see one coherent reconstructed image.
            const QSignalBlocker blocker(this);
            for (const auto& input : value.inputs) {
                valid = applyInputProtocolState(input, originEvidence, source, fieldsApplied) && valid;
            }
            for (const auto& output : value.outputs) {
                valid = applyOutputProtocolState(output, originEvidence, source, fieldsApplied) && valid;
            }
            for (const auto& effect : value.effects) {
                valid = applyFxProtocolState(effect, originEvidence, source, fieldsApplied) && valid;
            }
            if (auto* headphones = mutablePhysicalOutput(model::PhysicalOutputId::Headphones)) {
                mergeField(headphones->levelDb, value.headphoneVolumeDb,
                           originEvidence, source, fieldsApplied);
                emit physicalOutputChanged(model::PhysicalOutputId::Headphones);
            }
            mergeField(routing_.bluetoothUsbSwitch, value.flags[0], originEvidence, source, fieldsApplied);
            (void)setBluetoothUsbPhonesOnly(value.flags[1], originEvidence, source);
            mergeField(routing_.footswitchFxMode, value.flags[2], originEvidence, source, fieldsApplied);
            mergeField(routing_.muteInputs, value.flags[3], originEvidence, source, fieldsApplied);
            mergeField(routing_.usbStreaming, value.flags[4], originEvidence, source, fieldsApplied);
            (void)setUsbInputAssignment(0, value.flags[5] ? model::UsbPlaybackAssignment::UsbAudioLoopback : model::UsbPlaybackAssignment::AnalogInput, originEvidence, source);
            (void)setUsbInputAssignment(1, value.flags[6] ? model::UsbPlaybackAssignment::UsbAudioLoopback : model::UsbPlaybackAssignment::AnalogInput, originEvidence, source);
            (void)setMonitorStereoLink(value.flags[7], originEvidence, source);
            (void)setHeadphoneSource(value.flags[8] ? model::HeadphoneSource::Monitor : model::HeadphoneSource::Main, originEvidence, source);
            (void)setHeadphoneTapPoint(value.flags[9] ? model::RoutingTapPoint::PostFader : model::RoutingTapPoint::PreFader, originEvidence, source);
            (void)setOutputPadMinus10Dbv(model::PhysicalOutputId::MainOut, value.flags[10], originEvidence, source);
            (void)setOutputPadMinus10Dbv(model::PhysicalOutputId::MonitorOut1, value.flags[11], originEvidence, source);
            (void)setOutputPadMinus10Dbv(model::PhysicalOutputId::MonitorOut2, value.flags[11], originEvidence, source);
            mergeField(routing_.monitorPostFader, value.flags[12], originEvidence, source, fieldsApplied);
            mergeField(globalTempo_.bpm, static_cast<double>(value.tempoBpm), originEvidence, source, fieldsApplied);
            mergeField(routing_.deviceSelectedOutput, static_cast<model::EndpointId>(value.selectedOutput), originEvidence, source, fieldsApplied);
            mergeField(routing_.monitorRoutingCode, value.monitorRouting, originEvidence, source, fieldsApplied);
            mergeField(routing_.snapshotScopeBits, value.snapshotScope, originEvidence, source, fieldsApplied);
            if (value.lastSnapshot < snapshots_.size()) activeSnapshotIndex_ = value.lastSnapshot;
            emit globalTempoChanged();
            emit routingChanged();
            mixerStateReset = true;
        },
        [&](const protocol::FxTempoCommand& value) {
            // Hardware-supported range remains unknown; retain the exact u16
            // device value rather than applying the old 50..250 UI guard.
            mergeField(globalTempo_.bpm, static_cast<double>(value.bpm),
                       originEvidence, source, fieldsApplied);
            emit globalTempoChanged();
        },
        [&](const protocol::SelectOutputCommand& value) {
            const auto endpoint = static_cast<model::EndpointId>(value.endpoint);
            valid = model::isDestinationEndpoint(endpoint);
            if (valid) {
                mergeField(routing_.deviceSelectedOutput, endpoint,
                           originEvidence, source, fieldsApplied);
                emit routingChanged();
            }
        },
        [&](const protocol::ChannelDelayCommand& value) {
            const auto bus = busIndexForEndpoint(value.endpoint);
            valid = bus.has_value();
            if (!valid) return;
            auto* target = mutableBus(*bus);
            valid = target != nullptr && target->outputDelay.has_value();
            if (valid) {
                mergeField(target->outputDelay->milliseconds,
                           static_cast<double>(value.ticks) / 48.0,
                           originEvidence, source, fieldsApplied);
                mergeField(target->outputDelay->enabled, value.ticks != 0,
                           originEvidence, source, fieldsApplied);
                emit busChanged(*bus);
            }
        },
        [&](const protocol::ChannelLabelsCommand& value) {
            composite = true;
            for (std::size_t i = 0; i < value.labels.size(); ++i) {
                const auto input = inputIndexForWireEndpoint(value.labels[i].endpoint);
                if (!input.has_value()) continue;
                auto* channel = mutableChannel(*input);
                mergeField(channel->rawIconId, value.labels[i].icon,
                           originEvidence, source, fieldsApplied);
                mergeField(channel->name, QString::fromUtf8(value.labels[i].text),
                           originEvidence, source, fieldsApplied);
                emit channelChanged(*input);
            }
        },
        [&](const protocol::SnapshotNamesCommand& value) {
            composite = true;
            for (std::size_t i = 0; i < value.names.size() && i < static_cast<std::size_t>(snapshots_.size()); ++i) {
                mergeField(snapshots_[static_cast<int>(i)].name,
                           QString::fromUtf8(value.names[i]), originEvidence, source, fieldsApplied);
                emit snapshotChanged(static_cast<int>(i));
            }
        },
        [&](const protocol::SnapshotRenameCommand& value) {
            valid = value.slot < snapshots_.size();
            if (valid) {
                mergeField(snapshots_[value.slot].name, QString::fromUtf8(value.name),
                           originEvidence, source, fieldsApplied);
                emit snapshotChanged(value.slot);
            }
        },
        [&](const auto&) { recognized = false; },
    }, decoded.value);

    if (mixerStateReset && valid) {
        emit stateReset();
    }

    return {
        .applied = recognized && valid,
        .composite = composite,
        .fieldsApplied = fieldsApplied,
        .reason = !recognized ? QStringLiteral("command has no confirmed-state mutation")
            : (!valid ? QStringLiteral("command target is invalid for current state") : QString()),
    };
}

} // namespace flow8
