#include "protocol/flow8_command.h"

#include <type_traits>

namespace flow8::protocol {
namespace {

template<class... Ts>
struct Overloaded : Ts... { using Ts::operator()...; };
template<class... Ts>
Overloaded(Ts...) -> Overloaded<Ts...>;

} // namespace

quint8 commandId(const Flow8Command& command) noexcept
{
    return std::visit(Overloaded {
        [](const PanCommand&) { return quint8(0x00); },
        [](const SoloCommand&) { return quint8(0x01); },
        [](const GainCommand&) { return quint8(0x02); },
        [](const GraphicEqCommand&) { return quint8(0x03); },
        [](const HighPassFilterCommand&) { return quint8(0x04); },
        [](const LabelCommand&) { return quint8(0x05); },
        [](const RouteLevelCommand&) { return quint8(0x06); },
        [](const RouteStateCommand&) { return quint8(0x06); },
        [](const GetSnapshotNamesCommand&) { return quint8(0x07); },
        [](const MuteCommand&) { return quint8(0x08); },
        [](const ParametricEqCommand&) { return quint8(0x09); },
        [](const PhaseCommand&) { return quint8(0x10); },
        [](const FxSetupCommand&) { return quint8(0x11); },
        [](const SnapshotDeleteCommand&) { return quint8(0x12); },
        [](const CompressorCommand&) { return quint8(0x13); },
        [](const LimiterCommand&) { return quint8(0x14); },
        [](const PhantomCommand&) { return quint8(0x15); },
        [](const GetChannelStateCommand&) { return quint8(0x16); },
        [](const InputStateCommand&) { return quint8(0x17); },
        [](const OutputStateCommand&) { return quint8(0x18); },
        [](const SnapshotSaveCommand&) { return quint8(0x19); },
        [](const SnapshotLoadCommand&) { return quint8(0x20); },
        [](const MeterRequestCommand&) { return quint8(0x21); },
        [](const MeterUpdateCommand&) { return quint8(0x22); },
        [](const GetChannelLabelsCommand&) { return quint8(0x23); },
        [](const ChannelLabelsCommand&) { return quint8(0x24); },
        [](const SettingCommand&) { return quint8(0x25); },
        [](const GetSettingCommand&) { return quint8(0x26); },
        [](const SnapshotNamesCommand&) { return quint8(0x27); },
        [](const FactoryResetCommand&) { return quint8(0x29); },
        [](const FxStateCommand&) { return quint8(0x30); },
        [](const FxPresetCommand&) { return quint8(0x31); },
        [](const SnapshotRenameCommand&) { return quint8(0x32); },
        [](const ChannelConnectionCommand&) { return quint8(0x33); },
        [](const HandshakeHostCommand&) { return quint8(0x35); },
        [](const HandshakeReplyCommand&) { return quint8(0x36); },
        [](const GetMixerStateCommand&) { return quint8(0x37); },
        [](const MixerStateCommand&) { return quint8(0x38); },
        [](const HandshakeClientCommand&) { return quint8(0x39); },
        [](const FxTempoCommand&) { return quint8(0x40); },
        [](const SelectOutputCommand&) { return quint8(0x41); },
        [](const ChannelDelayCommand&) { return quint8(0x4a); },
    }, command);
}

QString commandSemanticName(const Flow8Command& command)
{
    const auto descriptor = apkCommandDescriptor(commandId(command));
    return descriptor.has_value()
        ? QString::fromLatin1(descriptor->semanticName)
        : QStringLiteral("unknown");
}

} // namespace flow8::protocol
