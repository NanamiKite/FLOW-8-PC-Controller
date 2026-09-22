#pragma once

#include "model/evidence_status.h"
#include "model/endpoint.h"
#include "protocol/apk_command_catalog.h"
#include "protocol/gain_codec.h"
#include "protocol/route_level_codec.h"

#include <QByteArray>
#include <QString>
#include <QVector>

#include <array>
#include <optional>
#include <variant>

namespace flow8::protocol {

struct ChannelLabel {
    quint8 endpoint {};
    quint16 icon {};
    QByteArray text;
};

struct PanCommand { quint8 endpoint {}; double pan {}; };
struct SoloCommand { quint8 endpoint {}; bool solo {}; };
struct GraphicEqCommand {
    quint8 endpoint {};
    quint8 band {};
    quint16 frequencyHz {};
    double q {};
    double gainDb {};
};
struct HighPassFilterCommand { quint8 endpoint {}; bool enabled {}; quint16 frequencyHz {}; };
struct LabelCommand { ChannelLabel label; };
// Receive-side representation of 0x06. The outbound UI semantic continues to
// use RouteLevelCommand (normalized input), while decoded state is dB.
struct RouteStateCommand { quint8 endpointA {}; quint8 destination {}; double levelDb {}; };
struct MuteCommand { quint8 endpoint {}; bool muted {}; };
struct ParametricEqCommand {
    quint8 endpoint {};
    quint8 band {};
    quint16 frequencyHz {};
    double q {};
    double gainDb {};
};
struct PhaseCommand { quint8 endpoint {}; bool inverted {}; };
struct FxSetupCommand {
    quint8 endpoint {};
    quint8 value1 {};
    quint8 value2 {};
    quint8 value3 {};
    quint8 routeFlags {};
};
struct SnapshotDeleteCommand { quint8 slot {}; };
struct CompressorCommand { quint8 endpoint {}; double amount {}; };
struct LimiterCommand { quint8 endpoint {}; double thresholdDb {}; };
struct PhantomCommand { quint8 endpoint {}; bool enabled {}; };
struct GetChannelStateCommand { quint8 endpoint {}; };

struct InputStateCommand {
    quint8 id {};
    quint8 flags {};
    double gainDb {};
    double compressorAmount {};
    quint16 highPassFrequencyHz {};
    double balance {};
    std::array<double, 4> eqGainDb {};
    std::array<quint16, 4> eqFrequencyHz {};
    std::array<double, 4> eqQ {};
    ChannelLabel label;
};

struct OutputStateCommand {
    quint8 id {};
    quint8 flags {};
    double volumeDb {};
    double pan {};
    double limiterDb {};
    std::array<double, 7> inputGainsDb {};
    std::array<double, 9> eqGainDb {};
    std::array<quint16, 9> eqFrequencyHz {};
    std::array<double, 9> eqQ {};
    quint32 delayTicks {};
};

struct SnapshotSaveCommand { quint8 slot {}; QByteArray name; };
struct SnapshotLoadCommand { quint8 slot {}; };
struct MeterRequestCommand { quint8 count {}; std::array<quint8, 15> channelCodes {}; };
struct MeterUpdateCommand {
    std::array<double, 15> metersDb {};
    quint16 gainReductionBits {};
};
struct SettingCommand { quint8 settingId {}; QByteArray data; };
struct FactoryResetCommand {};

struct FxStateCommand {
    quint8 id {};
    quint8 flags {};
    double volumeDb {};
    double pan {};
    quint8 value1 {};
    quint8 value2 {};
    quint8 value3 {};
    quint8 preset {};
    std::array<double, 7> inputGainsDb {};
    std::array<double, 2> auxGainsDb {};
};

struct FxPresetCommand { quint8 endpoint {}; quint8 preset {}; };
struct ChannelConnectionCommand { quint8 endpoint {}; quint8 subchannel {}; bool connected {}; };

struct MixerStateCommand {
    std::array<InputStateCommand, 7> inputs;
    std::array<OutputStateCommand, 3> outputs;
    std::array<FxStateCommand, 2> effects;
    double headphoneVolumeDb {};
    std::array<bool, 13> flags {};
    quint16 tempoBpm {};
    quint8 selectedOutput {};
    quint8 lastSnapshot {};
    quint8 monitorRouting {};
    quint8 snapshotScope {};
};

struct FxTempoCommand { quint16 bpm {}; };
struct SelectOutputCommand { quint8 endpoint {}; };
struct ChannelDelayCommand { quint8 endpoint {}; quint32 ticks {}; };

// Companion commands required by state recovery and handshake.
struct GetSnapshotNamesCommand {};
struct GetChannelLabelsCommand {};
struct ChannelLabelsCommand { quint8 count {}; std::array<ChannelLabel, 10> labels; };
struct GetSettingCommand { quint8 settingId {}; };
struct SnapshotNamesCommand { std::array<QByteArray, 15> names; };
struct SnapshotRenameCommand { quint8 slot {}; QByteArray name; };
struct HandshakeHostCommand {
    std::array<quint8, 16> deviceId {};
    bool pairingAny {};
    quint16 protocolVersion {};
    quint16 firmwareBuild {};
};
struct HandshakeReplyCommand {};
struct GetMixerStateCommand {};
struct HandshakeClientCommand { std::array<quint8, 16> clientId {}; };

using Flow8Command = std::variant<
    PanCommand,
    SoloCommand,
    GainCommand,
    GraphicEqCommand,
    HighPassFilterCommand,
    LabelCommand,
    RouteLevelCommand,
    RouteStateCommand,
    MuteCommand,
    ParametricEqCommand,
    PhaseCommand,
    FxSetupCommand,
    SnapshotDeleteCommand,
    CompressorCommand,
    LimiterCommand,
    PhantomCommand,
    GetChannelStateCommand,
    InputStateCommand,
    OutputStateCommand,
    SnapshotSaveCommand,
    SnapshotLoadCommand,
    MeterRequestCommand,
    MeterUpdateCommand,
    SettingCommand,
    FactoryResetCommand,
    FxStateCommand,
    FxPresetCommand,
    ChannelConnectionCommand,
    MixerStateCommand,
    FxTempoCommand,
    SelectOutputCommand,
    ChannelDelayCommand,
    GetSnapshotNamesCommand,
    GetChannelLabelsCommand,
    ChannelLabelsCommand,
    GetSettingCommand,
    SnapshotNamesCommand,
    SnapshotRenameCommand,
    HandshakeHostCommand,
    HandshakeReplyCommand,
    GetMixerStateCommand,
    HandshakeClientCommand>;

struct DecodedCommand {
    Flow8Command value;
    QByteArray payload;
    model::EvidenceStatus evidence {model::EvidenceStatus::VerifiedFromApk};
};

[[nodiscard]] quint8 commandId(const Flow8Command& command) noexcept;
[[nodiscard]] QString commandSemanticName(const Flow8Command& command);

} // namespace flow8::protocol
