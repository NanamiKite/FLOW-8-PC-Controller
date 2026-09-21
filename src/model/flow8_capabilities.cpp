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
        bool lowCut;
        bool compressor;
        bool phantom;
    };

    constexpr std::array definitions {
        Definition {InputId::Input1, InputType::Microphone, "Input 1", false, true, true, true, true},
        Definition {InputId::Input2, InputType::Microphone, "Input 2", false, true, true, true, true},
        Definition {InputId::Input3, InputType::MicrophoneLine, "Input 3", false, true, true, true, false},
        Definition {InputId::Input4, InputType::MicrophoneLine, "Input 4", false, true, true, true, false},
        Definition {InputId::Input56, InputType::StereoLinePair, "Input 5/6", true, true, true, true, false},
        Definition {InputId::Input78, InputType::StereoLinePair, "Input 7/8", true, true, true, true, false},
        Definition {InputId::UsbBluetooth, InputType::UsbBluetooth, "USB/BT", true, false, false, false, false},
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
        bool fxEngine;
        bool outputDelay;
    };
    constexpr std::array definitions {
        Definition {BusId::Main, "MAIN", true, true, true, true, false, true},
        Definition {BusId::Monitor1, "MON 1", true, false, true, true, false, true},
        Definition {BusId::Monitor2, "MON 2", true, false, true, true, false, true},
        Definition {BusId::Fx1, "FX 1", false, false, false, false, true, false},
        Definition {BusId::Fx2, "FX 2", false, false, false, false, true, false},
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
            .fxEngine = definition.fxEngine,
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
    routing.routes.reserve(inputStripCount * static_cast<int>(destinations.size()));
    for (int input = 0; input < inputStripCount; ++input) {
        for (const auto destination : destinations) {
            routing.routes.append(RouteState {
                .inputIndex = input,
                .destination = destination,
                .enabled = {},
            });
        }
    }

    constexpr std::array usbDestinations {
        UsbRouteDestination::Input1,
        UsbRouteDestination::Input2,
        UsbRouteDestination::Input3,
        UsbRouteDestination::Input4,
        UsbRouteDestination::Input56,
        UsbRouteDestination::Input78,
        UsbRouteDestination::UsbBluetooth,
        UsbRouteDestination::Monitor1,
        UsbRouteDestination::Monitor2,
    };
    routing.usbRoutes.reserve(static_cast<qsizetype>(usbDestinations.size()));
    for (const auto destination : usbDestinations) {
        routing.usbRoutes.append(UsbRouteState {
            .destination = destination,
            .enabled = {},
        });
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
