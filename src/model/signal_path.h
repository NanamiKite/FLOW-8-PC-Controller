#pragma once

#include "model/capability.h"
#include "model/endpoint.h"
#include "model/state_value.h"

#include <QString>

#include <optional>

namespace flow8::model {

// These are the seven conventional signal sources shown by the Mixer. USB
// audio loopback endpoints are modelled independently in usb_audio.h.
enum class SignalSourceId {
    Input1,
    Input2,
    Input3,
    Input4,
    Input56,
    Input78,
    BluetoothUsbMixer,
};

enum class SignalSourceType {
    PhysicalInput,
    StereoPhysicalInput,
    BluetoothUsbMixer,
};

struct SignalSourceState {
    SignalSourceId id {SignalSourceId::Input1};
    SignalSourceType type {SignalSourceType::PhysicalInput};
    QString defaultLabel;
    // Present only for the seven sources shown as conventional Mixer strips.
    std::optional<int> mixerInputIndex;
    std::optional<EndpointId> mixerEndpoint;
    CapabilityEvidence evidence;
};

enum class PhysicalOutputId {
    MainOut,
    MonitorOut1,
    MonitorOut2,
    Headphones,
};

enum class PhysicalOutputSource {
    Main,
    Monitor1,
    Monitor2,
};

// The APK exposes the headphone selector as MAIN/MON, not as three separate
// MAIN/MON1/MON2 choices. Keep that semantic boundary until device behaviour
// provides a more detailed mapping.
enum class HeadphoneSource {
    Main,
    Monitor,
};

enum class RoutingTapPoint {
    PreFader,
    PostFader,
};

struct PhysicalOutputState {
    PhysicalOutputId id {PhysicalOutputId::MainOut};
    QString defaultLabel;
    bool sourceSelectable {};
    std::optional<PhysicalOutputSource> nominalSource;
    std::optional<StateValue<bool>> padMinus10Dbv;
    CapabilityEvidence evidence;
};

} // namespace flow8::model
