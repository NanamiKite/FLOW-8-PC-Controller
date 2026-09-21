#pragma once

#include "model/capability.h"
#include "model/state_value.h"

#include <QString>

#include <array>

namespace flow8::model {

// USB 1/2 and USB 3/4 are USB-audio loopback endpoints. They are not mixer
// buses, conventional mixer inputs, or aliases for MON1/MON2.
enum class UsbAudioEndpointId {
    Usb12,
    Usb34,
};

struct UsbAudioEndpointState {
    UsbAudioEndpointId id {UsbAudioEndpointId::Usb12};
    QString defaultLabel;
    CapabilityEvidence evidence;
};

enum class UsbMode {
    Streaming,
    Recording,
};

// The two APK-visible playback assignments are pair-specific: Input 5/6 can
// select USB 1/2, and Input 7/8 can select USB 3/4. This enum describes the
// kind of assignment without pretending that USB is a conventional strip.
enum class UsbPlaybackAssignment {
    AnalogInput,
    UsbAudioLoopback,
};

// This is a physical-output feed selection exposed by the APK Routing page.
// It deliberately does not live on MON1/MON2 MixBusState: choosing USB audio
// for a monitor jack does not turn USB 1/2 or USB 3/4 into monitor mix buses.
enum class PhysicalMonitorOutputFeed {
    NominalMonitorMix,
    Usb12,
    Usb34,
};

struct UsbAudioRoutingState {
    StateValue<UsbMode> mode;
    StateValue<UsbPlaybackAssignment> input56Assignment;
    StateValue<UsbPlaybackAssignment> input78Assignment;
    std::array<StateValue<PhysicalMonitorOutputFeed>, 2> monitorOutputFeeds;
};

} // namespace flow8::model
