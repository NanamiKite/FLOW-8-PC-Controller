#include "protocol/command_codec.h"

#include "protocol/field_codec.h"
#include "protocol/packet.h"

#include <algorithm>
#include <type_traits>
#include <utility>

namespace flow8::protocol {
namespace {

template<class... Ts>
struct Overloaded : Ts... { using Ts::operator()...; };
template<class... Ts>
Overloaded(Ts...) -> Overloaded<Ts...>;

bool writeLabel(FieldWriter& writer, const ChannelLabel& label)
{
    writer.u8(label.endpoint);
    writer.u16be(label.icon);
    return writer.lengthPrefixed(label.text, 20);
}

std::optional<ChannelLabel> readLabel(FieldReader& reader)
{
    const auto endpoint = reader.u8();
    const auto icon = reader.u16be();
    const auto text = reader.lengthPrefixed(20);
    if (!endpoint.has_value() || !icon.has_value() || !text.has_value()) {
        return std::nullopt;
    }
    return ChannelLabel {.endpoint = *endpoint, .icon = *icon, .text = *text};
}

bool writeInputState(FieldWriter& writer, const InputStateCommand& state)
{
    writer.u8(state.id);
    writer.u8(state.flags);
    if (!writer.fix8(Fix8Format::GainDb, state.gainDb)
        || !writer.fix8(Fix8Format::UnitInterval, state.compressorAmount)) {
        return false;
    }
    writer.u16be(state.highPassFrequencyHz);
    if (!writer.fix8(Fix8Format::Pan, state.balance)) {
        return false;
    }
    for (const double value : state.eqGainDb) {
        if (!writer.fix8(Fix8Format::EqGainDb, value)) return false;
    }
    for (const quint16 value : state.eqFrequencyHz) writer.u16be(value);
    for (const double value : state.eqQ) {
        if (!writer.fix8(Fix8Format::Q, value)) return false;
    }
    return writeLabel(writer, state.label);
}

std::optional<InputStateCommand> readInputState(FieldReader& reader)
{
    InputStateCommand state;
    const auto id = reader.u8();
    const auto flags = reader.u8();
    const auto gain = reader.fix8(Fix8Format::GainDb);
    const auto compressor = reader.fix8(Fix8Format::UnitInterval);
    const auto hpf = reader.u16be();
    const auto balance = reader.fix8(Fix8Format::Pan);
    if (!id || !flags || !gain || !compressor || !hpf || !balance) return std::nullopt;
    state.id = *id;
    state.flags = *flags;
    state.gainDb = *gain;
    state.compressorAmount = *compressor;
    state.highPassFrequencyHz = *hpf;
    state.balance = *balance;
    for (auto& value : state.eqGainDb) {
        const auto parsed = reader.fix8(Fix8Format::EqGainDb);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    for (auto& value : state.eqFrequencyHz) {
        const auto parsed = reader.u16be();
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    for (auto& value : state.eqQ) {
        const auto parsed = reader.fix8(Fix8Format::Q);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    const auto label = readLabel(reader);
    if (!label) return std::nullopt;
    state.label = *label;
    return state;
}

bool writeOutputState(FieldWriter& writer, const OutputStateCommand& state)
{
    writer.u8(state.id);
    writer.u8(state.flags);
    if (!writer.fix8(Fix8Format::FaderDb, state.volumeDb)
        || !writer.fix8(Fix8Format::Pan, state.pan)
        || !writer.fix8(Fix8Format::GainDb, state.limiterDb)) return false;
    for (const double value : state.inputGainsDb) {
        if (!writer.fix8(Fix8Format::FaderDb, value)) return false;
    }
    for (const double value : state.eqGainDb) {
        if (!writer.fix8(Fix8Format::EqGainDb, value)) return false;
    }
    for (const quint16 value : state.eqFrequencyHz) writer.u16be(value);
    for (const double value : state.eqQ) {
        if (!writer.fix8(Fix8Format::Q, value)) return false;
    }
    writer.u32be(state.delayTicks);
    return true;
}

std::optional<OutputStateCommand> readOutputState(FieldReader& reader)
{
    OutputStateCommand state;
    const auto id = reader.u8();
    const auto flags = reader.u8();
    const auto volume = reader.fix8(Fix8Format::FaderDb);
    const auto pan = reader.fix8(Fix8Format::Pan);
    const auto limiter = reader.fix8(Fix8Format::GainDb);
    if (!id || !flags || !volume || !pan || !limiter) return std::nullopt;
    state.id = *id;
    state.flags = *flags;
    state.volumeDb = *volume;
    state.pan = *pan;
    state.limiterDb = *limiter;
    for (auto& value : state.inputGainsDb) {
        const auto parsed = reader.fix8(Fix8Format::FaderDb);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    for (auto& value : state.eqGainDb) {
        const auto parsed = reader.fix8(Fix8Format::EqGainDb);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    for (auto& value : state.eqFrequencyHz) {
        const auto parsed = reader.u16be();
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    for (auto& value : state.eqQ) {
        const auto parsed = reader.fix8(Fix8Format::Q);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    const auto delay = reader.u32be();
    if (!delay) return std::nullopt;
    state.delayTicks = *delay;
    return state;
}

bool writeFxState(FieldWriter& writer, const FxStateCommand& state)
{
    writer.u8(state.id);
    writer.u8(state.flags);
    if (!writer.fix8(Fix8Format::FaderDb, state.volumeDb)
        || !writer.fix8(Fix8Format::Pan, state.pan)) return false;
    writer.u8(state.value1);
    writer.u8(state.value2);
    writer.u8(state.value3);
    writer.u8(state.preset);
    for (const double value : state.inputGainsDb) {
        if (!writer.fix8(Fix8Format::FaderDb, value)) return false;
    }
    for (const double value : state.auxGainsDb) {
        if (!writer.fix8(Fix8Format::FaderDb, value)) return false;
    }
    return true;
}

std::optional<FxStateCommand> readFxState(FieldReader& reader)
{
    FxStateCommand state;
    const auto id = reader.u8();
    const auto flags = reader.u8();
    const auto volume = reader.fix8(Fix8Format::FaderDb);
    const auto pan = reader.fix8(Fix8Format::Pan);
    const auto value1 = reader.u8();
    const auto value2 = reader.u8();
    const auto value3 = reader.u8();
    const auto preset = reader.u8();
    if (!id || !flags || !volume || !pan || !value1 || !value2 || !value3 || !preset) {
        return std::nullopt;
    }
    state.id = *id;
    state.flags = *flags;
    state.volumeDb = *volume;
    state.pan = *pan;
    state.value1 = *value1;
    state.value2 = *value2;
    state.value3 = *value3;
    state.preset = *preset;
    for (auto& value : state.inputGainsDb) {
        const auto parsed = reader.fix8(Fix8Format::FaderDb);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    for (auto& value : state.auxGainsDb) {
        const auto parsed = reader.fix8(Fix8Format::FaderDb);
        if (!parsed) return std::nullopt;
        value = *parsed;
    }
    return state;
}

std::optional<QByteArray> serializePayload(const Flow8Command& command)
{
    FieldWriter writer;
    const bool ok = std::visit(Overloaded {
        [&writer](const PanCommand& value) {
            writer.u8(value.endpoint); return writer.fix8(Fix8Format::Pan, value.pan);
        },
        [&writer](const SoloCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.solo ? 1 : 0); return true;
        },
        [](const GainCommand&) { return false; },
        [&writer](const GraphicEqCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.band); writer.u16be(value.frequencyHz);
            return writer.fix8(Fix8Format::Q, value.q)
                && writer.fix8(Fix8Format::EqGainDb, value.gainDb);
        },
        [&writer](const HighPassFilterCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.enabled ? 1 : 0);
            writer.u16be(value.frequencyHz); return true;
        },
        [&writer](const LabelCommand& value) { return writeLabel(writer, value.label); },
        [](const RouteLevelCommand&) { return false; },
        [&writer](const RouteStateCommand& value) {
            writer.u8(value.endpointA); writer.u8(value.destination);
            return writer.fix8(Fix8Format::FaderDb, value.levelDb);
        },
        [&writer](const MuteCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.muted ? 1 : 0); return true;
        },
        [&writer](const ParametricEqCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.band); writer.u16be(value.frequencyHz);
            return writer.fix8(Fix8Format::Q, value.q)
                && writer.fix8(Fix8Format::EqGainDb, value.gainDb);
        },
        [&writer](const PhaseCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.inverted ? 1 : 0); return true;
        },
        [&writer](const FxSetupCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.value1); writer.u8(value.value2);
            writer.u8(value.value3); writer.u8(value.routeFlags); return true;
        },
        [&writer](const SnapshotDeleteCommand& value) { writer.u8(value.slot); return true; },
        [&writer](const CompressorCommand& value) {
            writer.u8(value.endpoint); return writer.fix8(Fix8Format::UnitInterval, value.amount);
        },
        [&writer](const LimiterCommand& value) {
            writer.u8(value.endpoint); return writer.fix8(Fix8Format::GainDb, value.thresholdDb);
        },
        [&writer](const PhantomCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.enabled ? 1 : 0); return true;
        },
        [&writer](const GetChannelStateCommand& value) { writer.u8(value.endpoint); return true; },
        [&writer](const InputStateCommand& value) { return writeInputState(writer, value); },
        [&writer](const OutputStateCommand& value) { return writeOutputState(writer, value); },
        [&writer](const SnapshotSaveCommand& value) {
            writer.u8(value.slot); return writer.lengthPrefixed(value.name, 20);
        },
        [&writer](const SnapshotLoadCommand& value) { writer.u8(value.slot); return true; },
        [&writer](const MeterRequestCommand& value) {
            writer.u8(value.count);
            for (const quint8 code : value.channelCodes) writer.u8(code);
            return static_cast<std::size_t>(value.count) <= value.channelCodes.size();
        },
        [&writer](const MeterUpdateCommand& value) {
            for (const double meter : value.metersDb) {
                if (!writer.fix8(Fix8Format::FaderDb, meter)) return false;
            }
            writer.u16be(value.gainReductionBits); return true;
        },
        [&writer](const SettingCommand& value) {
            writer.u8(value.settingId); return writer.lengthPrefixed(value.data, 255);
        },
        [](const FactoryResetCommand&) { return true; },
        [&writer](const FxStateCommand& value) { return writeFxState(writer, value); },
        [&writer](const FxPresetCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.preset); return true;
        },
        [&writer](const ChannelConnectionCommand& value) {
            writer.u8(value.endpoint); writer.u8(value.subchannel);
            writer.u8(value.connected ? 1 : 0); return true;
        },
        [&writer](const MixerStateCommand& value) {
            for (const auto& input : value.inputs) if (!writeInputState(writer, input)) return false;
            for (const auto& output : value.outputs) if (!writeOutputState(writer, output)) return false;
            for (const auto& effect : value.effects) if (!writeFxState(writer, effect)) return false;
            if (!writer.fix8(Fix8Format::FaderDb, value.headphoneVolumeDb)) return false;
            quint16 flags = 0;
            for (std::size_t index = 0; index < value.flags.size(); ++index) {
                if (value.flags[index]) flags |= static_cast<quint16>(1U << index);
            }
            writer.u8(static_cast<quint8>(flags));
            writer.u8(static_cast<quint8>(flags >> 8U));
            writer.u16be(value.tempoBpm);
            writer.u8(value.selectedOutput); writer.u8(value.lastSnapshot);
            writer.u8(value.monitorRouting); writer.u8(value.snapshotScope);
            return true;
        },
        [&writer](const FxTempoCommand& value) { writer.u16be(value.bpm); return true; },
        [&writer](const SelectOutputCommand& value) { writer.u8(value.endpoint); return true; },
        [&writer](const ChannelDelayCommand& value) {
            writer.u8(value.endpoint); writer.u32be(value.ticks); return true;
        },
        [](const GetSnapshotNamesCommand&) { return true; },
        [](const GetChannelLabelsCommand&) { return true; },
        [&writer](const ChannelLabelsCommand& value) {
            if (value.count > value.labels.size()) return false;
            writer.u8(value.count);
            for (const auto& label : value.labels) if (!writeLabel(writer, label)) return false;
            return true;
        },
        [&writer](const GetSettingCommand& value) { writer.u8(value.settingId); return true; },
        [&writer](const SnapshotNamesCommand& value) {
            for (const auto& name : value.names) if (!writer.lengthPrefixed(name, 20)) return false;
            return true;
        },
        [&writer](const SnapshotRenameCommand& value) {
            writer.u8(value.slot); return writer.lengthPrefixed(value.name, 20);
        },
        [&writer](const HandshakeHostCommand& value) {
            for (const quint8 byte : value.deviceId) writer.u8(byte);
            writer.u8(value.pairingAny ? 1 : 0); writer.u16be(value.protocolVersion);
            writer.u16be(value.firmwareBuild); return true;
        },
        [](const HandshakeReplyCommand&) { return true; },
        [](const GetMixerStateCommand&) { return true; },
        [&writer](const HandshakeClientCommand& value) {
            for (const quint8 byte : value.clientId) writer.u8(byte);
            return true;
        },
    }, command);
    return ok ? std::optional<QByteArray>(writer.takeBytes()) : std::nullopt;
}

template<typename T>
CommandDecodeResult decoded(const QByteArrayView payload, T value)
{
    return {
        .command = DecodedCommand {
            .value = Flow8Command {std::move(value)},
            .payload = QByteArray(payload.data(), payload.size()),
            .evidence = model::EvidenceStatus::VerifiedFromApk,
        },
    };
}

CommandDecodeResult invalid(const QString& message)
{
    return {.error = CommandCodecError::InvalidPayloadLength, .message = message};
}

} // namespace

CommandEncodeResult encodeCommand(
    const Flow8Command& command, const qsizetype maxRawPacketSize,
    const quint8 sequenceId)
{
    if (const auto* gain = std::get_if<GainCommand>(&command)) {
        const auto result = encodeGain(*gain);
        return result.ok()
            ? CommandEncodeResult {.packets = {*result.packet}}
            : CommandEncodeResult {.error = CommandCodecError::InvalidSemanticValue,
                                   .message = result.message};
    }
    if (const auto* route = std::get_if<RouteLevelCommand>(&command)) {
        const auto result = encodeRouteLevel(*route);
        return result.ok()
            ? CommandEncodeResult {.packets = {*result.packet}}
            : CommandEncodeResult {.error = CommandCodecError::InvalidSemanticValue,
                                   .message = result.message};
    }
    const auto payload = serializePayload(command);
    if (!payload.has_value()) {
        return {.error = CommandCodecError::InvalidSemanticValue,
                .message = QStringLiteral("command contains an invalid field value")};
    }
    auto packets = frameCommand(commandId(command), *payload, maxRawPacketSize, sequenceId);
    if (packets.isEmpty()) {
        return {.error = CommandCodecError::FragmentationFailed,
                .message = QStringLiteral("command could not be framed within APK limits")};
    }
    return {.packets = std::move(packets)};
}

CommandDecodeResult decodeCommandPayload(
    const quint8 id, const QByteArrayView payload)
{
    FieldReader reader(payload);
    Flow8Command command = FactoryResetCommand {};
    switch (id) {
    case 0x00: {
        const auto endpoint = reader.u8(); const auto value = reader.fix8(Fix8Format::Pan);
        if (!endpoint || !value) return invalid(QStringLiteral("invalid Pan payload"));
        command = PanCommand {*endpoint, *value}; break;
    }
    case 0x01: {
        const auto endpoint = reader.u8(); const auto value = reader.u8();
        if (!endpoint || !value || *value > 1) return invalid(QStringLiteral("invalid Solo payload"));
        command = SoloCommand {*endpoint, *value != 0}; break;
    }
    case 0x02: {
        const auto endpoint = reader.u8(); const auto value = reader.fix8(Fix8Format::GainDb);
        if (!endpoint || !value) return invalid(QStringLiteral("invalid Gain payload"));
        command = GainCommand {.inputEndpoint = static_cast<model::EndpointId>(*endpoint), .gainDb = *value};
        break;
    }
    case 0x03:
    case 0x09: {
        const auto endpoint = reader.u8(); const auto band = reader.u8();
        const auto frequency = reader.u16be(); const auto q = reader.fix8(Fix8Format::Q);
        const auto gain = reader.fix8(Fix8Format::EqGainDb);
        if (!endpoint || !band || !frequency || !q || !gain) return invalid(QStringLiteral("invalid EQ payload"));
        command = id == 0x03
            ? Flow8Command {GraphicEqCommand {*endpoint, *band, *frequency, *q, *gain}}
            : Flow8Command {ParametricEqCommand {*endpoint, *band, *frequency, *q, *gain}};
        break;
    }
    case 0x04: {
        const auto endpoint = reader.u8(); const auto enabled = reader.u8(); const auto frequency = reader.u16be();
        if (!endpoint || !enabled || *enabled > 1 || !frequency) return invalid(QStringLiteral("invalid HPF payload"));
        command = HighPassFilterCommand {*endpoint, *enabled != 0, *frequency}; break;
    }
    case 0x05: {
        const auto label = readLabel(reader); if (!label) return invalid(QStringLiteral("invalid Label payload"));
        command = LabelCommand {*label}; break;
    }
    case 0x06: {
        const auto a = reader.u8(); const auto b = reader.u8(); const auto value = reader.fix8(Fix8Format::FaderDb);
        if (!a || !b || !value) return invalid(QStringLiteral("invalid Route payload"));
        command = RouteStateCommand {*a, *b, *value}; break;
    }
    case 0x07: command = GetSnapshotNamesCommand {}; break;
    case 0x08: {
        const auto endpoint = reader.u8(); const auto value = reader.u8();
        if (!endpoint || !value || *value > 1) return invalid(QStringLiteral("invalid Mute payload"));
        command = MuteCommand {*endpoint, *value != 0}; break;
    }
    case 0x10: {
        const auto endpoint = reader.u8(); const auto value = reader.u8();
        if (!endpoint || !value || *value > 1) return invalid(QStringLiteral("invalid Phase payload"));
        command = PhaseCommand {*endpoint, *value != 0}; break;
    }
    case 0x11: {
        const auto endpoint = reader.u8(); const auto a = reader.u8(); const auto b = reader.u8();
        const auto c = reader.u8(); const auto flags = reader.u8();
        if (!endpoint || !a || !b || !c || !flags) return invalid(QStringLiteral("invalid FX Setup payload"));
        command = FxSetupCommand {*endpoint, *a, *b, *c, *flags}; break;
    }
    case 0x12: { const auto slot = reader.u8(); if (!slot) return invalid(QStringLiteral("invalid Snapshot Delete payload")); command = SnapshotDeleteCommand {*slot}; break; }
    case 0x13: {
        const auto endpoint = reader.u8(); const auto amount = reader.fix8(Fix8Format::UnitInterval);
        if (!endpoint || !amount) return invalid(QStringLiteral("invalid Compressor payload"));
        command = CompressorCommand {*endpoint, *amount}; break;
    }
    case 0x14: {
        const auto endpoint = reader.u8(); const auto threshold = reader.fix8(Fix8Format::GainDb);
        if (!endpoint || !threshold) return invalid(QStringLiteral("invalid Limiter payload"));
        command = LimiterCommand {*endpoint, *threshold}; break;
    }
    case 0x15: {
        const auto endpoint = reader.u8(); const auto value = reader.u8();
        if (!endpoint || !value || *value > 1) return invalid(QStringLiteral("invalid Phantom payload"));
        command = PhantomCommand {*endpoint, *value != 0}; break;
    }
    case 0x16: { const auto endpoint = reader.u8(); if (!endpoint) return invalid(QStringLiteral("invalid Get Channel State payload")); command = GetChannelStateCommand {*endpoint}; break; }
    case 0x17: { const auto state = readInputState(reader); if (!state) return invalid(QStringLiteral("invalid Input State payload")); command = *state; break; }
    case 0x18: { const auto state = readOutputState(reader); if (!state) return invalid(QStringLiteral("invalid Output State payload")); command = *state; break; }
    case 0x19: {
        const auto slot = reader.u8(); const auto name = reader.lengthPrefixed(20);
        if (!slot || !name) return invalid(QStringLiteral("invalid Snapshot Save payload"));
        command = SnapshotSaveCommand {*slot, *name}; break;
    }
    case 0x20: { const auto slot = reader.u8(); if (!slot) return invalid(QStringLiteral("invalid Snapshot Load payload")); command = SnapshotLoadCommand {*slot}; break; }
    case 0x21: {
        MeterRequestCommand value; const auto count = reader.u8(); if (!count) return invalid(QStringLiteral("invalid Meter Request payload")); value.count = *count;
        for (auto& code : value.channelCodes) { const auto parsed = reader.u8(); if (!parsed) return invalid(QStringLiteral("invalid Meter Request payload")); code = *parsed; }
        if (value.count > value.channelCodes.size()) return invalid(QStringLiteral("invalid Meter Request count"));
        command = value; break;
    }
    case 0x22: {
        MeterUpdateCommand value;
        for (auto& meter : value.metersDb) { const auto parsed = reader.fix8(Fix8Format::FaderDb); if (!parsed) return invalid(QStringLiteral("invalid Meter Update payload")); meter = *parsed; }
        const auto gr = reader.u16be(); if (!gr) return invalid(QStringLiteral("invalid Meter Update payload")); value.gainReductionBits = *gr;
        command = value; break;
    }
    case 0x23: command = GetChannelLabelsCommand {}; break;
    case 0x24: {
        ChannelLabelsCommand value; const auto count = reader.u8(); if (!count || *count > value.labels.size()) return invalid(QStringLiteral("invalid Channel Labels count")); value.count = *count;
        for (auto& label : value.labels) { const auto parsed = readLabel(reader); if (!parsed) return invalid(QStringLiteral("invalid Channel Labels payload")); label = *parsed; }
        command = value; break;
    }
    case 0x25: {
        const auto setting = reader.u8(); const auto data = reader.lengthPrefixed(255);
        if (!setting || !data) return invalid(QStringLiteral("invalid Setting payload")); command = SettingCommand {*setting, *data}; break;
    }
    case 0x26: { const auto setting = reader.u8(); if (!setting) return invalid(QStringLiteral("invalid Get Setting payload")); command = GetSettingCommand {*setting}; break; }
    case 0x27: {
        SnapshotNamesCommand value;
        for (auto& name : value.names) { const auto parsed = reader.lengthPrefixed(20); if (!parsed) return invalid(QStringLiteral("invalid Snapshot Names payload")); name = *parsed; }
        command = value; break;
    }
    case 0x29: command = FactoryResetCommand {}; break;
    case 0x30: { const auto state = readFxState(reader); if (!state) return invalid(QStringLiteral("invalid FX State payload")); command = *state; break; }
    case 0x31: {
        const auto endpoint = reader.u8(); const auto preset = reader.u8();
        if (!endpoint || !preset) return invalid(QStringLiteral("invalid FX Preset payload")); command = FxPresetCommand {*endpoint, *preset}; break;
    }
    case 0x32: {
        const auto slot = reader.u8(); const auto name = reader.lengthPrefixed(20);
        if (!slot || !name) return invalid(QStringLiteral("invalid Snapshot Rename payload")); command = SnapshotRenameCommand {*slot, *name}; break;
    }
    case 0x33: {
        const auto endpoint = reader.u8(); const auto subchannel = reader.u8(); const auto connected = reader.u8();
        if (!endpoint || !subchannel || !connected || *connected > 1) return invalid(QStringLiteral("invalid Channel Connection payload"));
        command = ChannelConnectionCommand {*endpoint, *subchannel, *connected != 0}; break;
    }
    case 0x35: {
        HandshakeHostCommand value;
        for (auto& byte : value.deviceId) { const auto parsed = reader.u8(); if (!parsed) return invalid(QStringLiteral("invalid Handshake Host payload")); byte = *parsed; }
        const auto pairing = reader.u8(); const auto protocolVersion = reader.u16be(); const auto firmware = reader.u16be();
        if (!pairing || *pairing > 1 || !protocolVersion || !firmware) return invalid(QStringLiteral("invalid Handshake Host payload"));
        value.pairingAny = *pairing != 0; value.protocolVersion = *protocolVersion; value.firmwareBuild = *firmware; command = value; break;
    }
    case 0x36: command = HandshakeReplyCommand {}; break;
    case 0x37: command = GetMixerStateCommand {}; break;
    case 0x38: {
        MixerStateCommand value;
        for (auto& input : value.inputs) { const auto parsed = readInputState(reader); if (!parsed) return invalid(QStringLiteral("invalid Mixer State input")); input = *parsed; }
        for (auto& output : value.outputs) { const auto parsed = readOutputState(reader); if (!parsed) return invalid(QStringLiteral("invalid Mixer State output")); output = *parsed; }
        for (auto& effect : value.effects) { const auto parsed = readFxState(reader); if (!parsed) return invalid(QStringLiteral("invalid Mixer State FX")); effect = *parsed; }
        const auto headphones = reader.fix8(Fix8Format::FaderDb); const auto flagsLow = reader.u8(); const auto flagsHigh = reader.u8();
        const auto tempo = reader.u16be(); const auto selected = reader.u8(); const auto last = reader.u8(); const auto routing = reader.u8(); const auto scope = reader.u8();
        if (!headphones || !flagsLow || !flagsHigh || !tempo || !selected || !last || !routing || !scope) return invalid(QStringLiteral("invalid Mixer State tail"));
        value.headphoneVolumeDb = *headphones;
        const quint16 bits = static_cast<quint16>(*flagsLow | (static_cast<quint16>(*flagsHigh) << 8U));
        for (std::size_t index = 0; index < value.flags.size(); ++index) value.flags[index] = (bits & (1U << index)) != 0;
        value.tempoBpm = *tempo; value.selectedOutput = *selected; value.lastSnapshot = *last; value.monitorRouting = *routing; value.snapshotScope = *scope;
        command = value; break;
    }
    case 0x39: {
        HandshakeClientCommand value;
        for (auto& byte : value.clientId) { const auto parsed = reader.u8(); if (!parsed) return invalid(QStringLiteral("invalid Handshake Client payload")); byte = *parsed; }
        command = value; break;
    }
    case 0x40: { const auto bpm = reader.u16be(); if (!bpm) return invalid(QStringLiteral("invalid FX Tempo payload")); command = FxTempoCommand {*bpm}; break; }
    case 0x41: { const auto endpoint = reader.u8(); if (!endpoint) return invalid(QStringLiteral("invalid Select Output payload")); command = SelectOutputCommand {*endpoint}; break; }
    case 0x4a: {
        const auto endpoint = reader.u8(); const auto ticks = reader.u32be();
        if (!endpoint || !ticks) return invalid(QStringLiteral("invalid Channel Delay payload")); command = ChannelDelayCommand {*endpoint, *ticks}; break;
    }
    default:
        return {.error = CommandCodecError::UnsupportedCommand,
                .message = QStringLiteral("command has no implemented semantic parser")};
    }
    if (!reader.atEnd()) {
        return invalid(QStringLiteral("command payload contains trailing bytes"));
    }
    return decoded(payload, std::move(command));
}

} // namespace flow8::protocol
