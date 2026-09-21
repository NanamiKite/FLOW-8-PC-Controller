#include "model/flow8_capabilities.h"

#include <array>
#include <utility>

namespace flow8::model {
namespace {

constexpr auto officialQsg =
    "Behringer FLOW 8 Quick Start Guide, MIDI Implementation, pp. 44-45";

CapabilityEvidence qsgEvidence()
{
    return {
        .source = CapabilitySource::OfficialManual,
        .reference = QString::fromLatin1(officialQsg),
    };
}

} // namespace

QVector<ChannelState> createOfficialInputProfile()
{
    struct Definition {
        InputId id;
        InputType type;
        const char* name;
        bool stereoPair;
        bool gain;
        bool phase;
        bool lowCut;
        bool compressor;
        bool phantom;
    };

    constexpr std::array definitions {
        Definition {InputId::Input1, InputType::Microphone, "Input 1", false, true, true, true, true, true},
        Definition {InputId::Input2, InputType::Microphone, "Input 2", false, true, true, true, true, true},
        Definition {InputId::Input3, InputType::MicrophoneLine, "Input 3", false, true, true, true, true, false},
        Definition {InputId::Input4, InputType::MicrophoneLine, "Input 4", false, true, true, true, true, false},
        Definition {InputId::Input56, InputType::StereoLinePair, "Input 5/6", true, true, true, true, true, false},
        Definition {InputId::Input78, InputType::StereoLinePair, "Input 7/8", true, true, true, true, true, false},
        Definition {InputId::UsbBluetooth, InputType::UsbBluetooth, "USB/BT", true, false, false, false, false, false},
    };

    QVector<ChannelState> channels;
    channels.reserve(static_cast<qsizetype>(definitions.size()));
    for (std::size_t index = 0; index < definitions.size(); ++index) {
        const auto& definition = definitions[index];
        ChannelState channel;
        channel.index = static_cast<int>(index);
        channel.inputId = definition.id;
        channel.inputType = definition.type;
        channel.stereoPair = definition.stereoPair;
        channel.spatialControl = definition.stereoPair
            ? SpatialControl::Balance : SpatialControl::Pan;
        channel.capabilities = {
            .gain = definition.gain,
            .phase = definition.phase,
            .lowCut = definition.lowCut,
            .phantom48V = definition.phantom,
            .equalizer = true,
            .compressor = definition.compressor,
            .monitorSends = true,
            .fxSends = true,
            .evidence = qsgEvidence(),
        };
        channel.defaultLabel = QString::fromLatin1(definition.name);
        const ChannelIcon defaultIcon = definition.type == InputType::UsbBluetooth
            ? ChannelIcon::Playback
            : (definition.type == InputType::StereoLinePair
                ? ChannelIcon::Instrument : ChannelIcon::Microphone);
        channel.icon = StateValue<ChannelIcon>::known(
            defaultIcon,
            EvidenceStatus::Unknown,
            QStringLiteral("SYNTHETIC/default UI icon; not a device observation"));
        channel.visible = StateValue<bool>::known(
            true, EvidenceStatus::Unknown,
            QStringLiteral("Application mixer visibility; not a device observation"));
        if (definition.lowCut) {
            channel.lowCut.emplace();
            channel.lowCutHz.emplace();
        }
        if (definition.phase) {
            channel.phaseInverted.emplace();
        }
        if (definition.phantom) {
            channel.phantom48V.emplace();
        }
        channels.append(std::move(channel));
    }
    return channels;
}

QVector<BusState> createOfficialBusProfile()
{
    struct Definition {
        BusId id;
        const char* name;
        bool mute;
        bool balance;
        bool equalizer;
        bool limiter;
        bool outputDelay;
    };
    constexpr std::array definitions {
        Definition {BusId::Main, "MAIN", true, true, true, true, true},
        Definition {BusId::Monitor1, "MON 1", true, false, true, true, true},
        Definition {BusId::Monitor2, "MON 2", true, false, true, true, true},
    };

    QVector<BusState> buses;
    buses.reserve(static_cast<qsizetype>(definitions.size()));
    for (std::size_t index = 0; index < definitions.size(); ++index) {
        const auto& definition = definitions[index];
        BusState bus;
        bus.index = static_cast<int>(index);
        bus.busId = definition.id;
        bus.capabilities = {
            .mute = definition.mute,
            .balance = definition.balance,
            .equalizer = definition.equalizer,
            .limiter = definition.limiter,
            .outputDelay = definition.outputDelay,
            .evidence = qsgEvidence(),
        };
        bus.name = StateValue<QString>::known(
            QString::fromLatin1(definition.name), EvidenceStatus::Verified,
            QStringLiteral("OfficialManual capability label"));
        if (definition.mute) {
            bus.muted.emplace();
        }
        if (definition.balance) {
            bus.balance.emplace();
        }
        if (definition.equalizer) {
            bus.eq.emplace();
        }
        if (definition.limiter) {
            bus.limiterDb.emplace();
        }
        if (definition.outputDelay) {
            bus.outputDelay.emplace();
        }
        buses.append(std::move(bus));
    }
    return buses;
}

QVector<SignalSourceState> createSignalSourceProfile()
{
    struct Definition {
        SignalSourceId id;
        SignalSourceType type;
        const char* label;
        std::optional<int> mixerInputIndex;
    };
    constexpr std::array definitions {
        Definition {SignalSourceId::Input1, SignalSourceType::PhysicalInput,
                    "Input 1", 0},
        Definition {SignalSourceId::Input2, SignalSourceType::PhysicalInput,
                    "Input 2", 1},
        Definition {SignalSourceId::Input3, SignalSourceType::PhysicalInput,
                    "Input 3", 2},
        Definition {SignalSourceId::Input4, SignalSourceType::PhysicalInput,
                    "Input 4", 3},
        Definition {SignalSourceId::Input56, SignalSourceType::StereoPhysicalInput,
                    "Input 5/6", 4},
        Definition {SignalSourceId::Input78, SignalSourceType::StereoPhysicalInput,
                    "Input 7/8", 5},
        Definition {SignalSourceId::BluetoothUsbMixer, SignalSourceType::BluetoothUsbMixer,
                    "Bluetooth / USB Mixer Channel", 6},
    };
    QVector<SignalSourceState> sources;
    sources.reserve(static_cast<qsizetype>(definitions.size()));
    for (const auto& definition : definitions) {
        const auto endpoint = definition.mixerInputIndex.has_value()
            ? inputEndpointForIndex(*definition.mixerInputIndex) : std::nullopt;
        sources.append(SignalSourceState {
            .id = definition.id,
            .type = definition.type,
            .defaultLabel = QString::fromLatin1(definition.label),
            .mixerInputIndex = definition.mixerInputIndex,
            .mixerEndpoint = endpoint,
            .evidence = {
                .source = CapabilitySource::OfficialApk,
                .reference = QStringLiteral("docs/reverse-engineering.md sections 7 and 12"),
            },
        });
    }
    return sources;
}

QVector<UsbAudioEndpointState> createUsbAudioEndpointProfile()
{
    const CapabilityEvidence apkEvidence {
        .source = CapabilitySource::OfficialApk,
        .reference = QStringLiteral("docs/reverse-engineering.md section 12"),
    };
    return {
        UsbAudioEndpointState {
            .id = UsbAudioEndpointId::Usb12,
            .defaultLabel = QStringLiteral("USB 1/2"),
            .evidence = apkEvidence,
        },
        UsbAudioEndpointState {
            .id = UsbAudioEndpointId::Usb34,
            .defaultLabel = QStringLiteral("USB 3/4"),
            .evidence = apkEvidence,
        },
    };
}

MonitorLinkState createMonitorLinkProfile()
{
    MonitorLinkState link;
    link.propagationEvidence = EvidenceStatus::Unknown;
    link.propagationSource = QStringLiteral(
        "MON stereo-link propagation rules remain UNKNOWN");
    link.capabilityEvidence = {
        .source = CapabilitySource::OfficialApk,
        .reference = QStringLiteral("docs/reverse-engineering.md section 10"),
    };
    return link;
}

QVector<PhysicalOutputState> createPhysicalOutputProfile()
{
    const CapabilityEvidence apkEvidence {
        .source = CapabilitySource::OfficialApk,
        .reference = QStringLiteral("docs/reverse-engineering.md sections 10 and 12"),
    };
    QVector<PhysicalOutputState> outputs;
    outputs.reserve(4);
    outputs.append(PhysicalOutputState {
        .id = PhysicalOutputId::MainOut,
        .defaultLabel = QStringLiteral("MAIN OUT"),
        .sourceSelectable = false,
        .nominalSource = PhysicalOutputSource::Main,
        .padMinus10Dbv = StateValue<bool> {},
        .evidence = apkEvidence,
    });
    outputs.append(PhysicalOutputState {
        .id = PhysicalOutputId::MonitorOut1,
        .defaultLabel = QStringLiteral("MON OUT 1"),
        .sourceSelectable = true,
        .nominalSource = PhysicalOutputSource::Monitor1,
        .padMinus10Dbv = StateValue<bool> {},
        .evidence = apkEvidence,
    });
    outputs.append(PhysicalOutputState {
        .id = PhysicalOutputId::MonitorOut2,
        .defaultLabel = QStringLiteral("MON OUT 2"),
        .sourceSelectable = true,
        .nominalSource = PhysicalOutputSource::Monitor2,
        .padMinus10Dbv = StateValue<bool> {},
        .evidence = apkEvidence,
    });
    outputs.append(PhysicalOutputState {
        .id = PhysicalOutputId::Headphones,
        .defaultLabel = QStringLiteral("HEADPHONES"),
        .sourceSelectable = true,
        .nominalSource = std::nullopt,
        .padMinus10Dbv = std::nullopt,
        .evidence = apkEvidence,
    });
    return outputs;
}

QVector<SnapshotState> createHardwareSnapshotProfile()
{
    QVector<SnapshotState> snapshots;
    snapshots.reserve(hardwareSnapshotSlotCount);
    for (int slot = 0; slot < hardwareSnapshotSlotCount; ++slot) {
        SnapshotState snapshot;
        snapshot.index = slot;
        snapshot.id = QStringLiteral("hardware:%1").arg(slot + 1);
        snapshot.storage = SnapshotStorage::HardwareSlot;
        snapshot.minimumFirmware = StateValue<QString>::known(
            QStringLiteral("11739"), EvidenceStatus::Unknown,
            QStringLiteral("Official FLOW Mix capability note; not runtime firmware evidence"));
        snapshots.append(std::move(snapshot));
    }
    return snapshots;
}

RoutingState createRoutingProfile()
{
    RoutingState routing;
    constexpr std::array destinations {
        RoutingDestination::Main,
        RoutingDestination::Monitor1,
        RoutingDestination::Monitor2,
        RoutingDestination::Fx1,
        RoutingDestination::Fx2,
    };
    routing.routeLevels.cells.reserve(
        conventionalMixerInputCount * static_cast<int>(destinations.size()));
    for (int input = 0; input < conventionalMixerInputCount; ++input) {
        for (const auto destination : destinations) {
            routing.routeLevels.cells.append(RouteLevelState {
                .sourceEndpoint = *inputEndpointForIndex(input),
                .destinationEndpoint = endpointForDestination(destination),
                .confirmed = {},
                .pending = std::nullopt,
                .error = {},
            });
        }
    }
    constexpr std::array fxDestinations {
        FxOutputDestination::Main,
        FxOutputDestination::Monitor1,
        FxOutputDestination::Monitor2,
    };
    routing.fxOutputRoutes.reserve(2 * static_cast<int>(fxDestinations.size()));
    for (int effect = 0; effect < 2; ++effect) {
        for (const auto destination : fxDestinations) {
            routing.fxOutputRoutes.append(FxOutputRouteState {
                .effectIndex = effect,
                .destination = destination,
                .enabled = {},
            });
        }
    }
    return routing;
}

} // namespace flow8::model
