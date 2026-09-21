#include "ui/ui_text.h"

#include <QCoreApplication>

namespace flow8::ui {
namespace {

// QT_TRANSLATE_NOOP makes every hand-written UI string discoverable by
// lupdate even when it is selected through a runtime terminology table.
[[maybe_unused]] constexpr const char* translationCatalog[] {
    QT_TRANSLATE_NOOP("Flow8Ui", "10 dBV Output Level: Off"),
    QT_TRANSLATE_NOOP("Flow8Ui", "10 dBV Output Level: On"),
    QT_TRANSLATE_NOOP("Flow8Ui", "15 hardware slots · Recall is simulated; hardware command needs verification."),
    QT_TRANSLATE_NOOP("Flow8Ui", "App snapshots are stored separately from the 15 hardware slots."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel sends · Pre/Post-Fader · 9-band EQ · Limiter"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Configure Inputs"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connect to Simulator before opening Setup."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Delete"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Edit Channel"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Final output level and output state"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FLOW 8 PC Controller\nSimulator mode uses SYNTHETIC data. BLE, GATT and device commands still need hardware verification."),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX %1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Hardware Required"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Hardware-facing preferences and SysEx requests remain unavailable without a verified device."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Independent Monitor"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Info"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Load"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Main Out"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Main mix channel levels · Balance · 9-band EQ · Limiter"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Mix"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Mixer Snapshots"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Monitor %1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Output Delay: %1 ms"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Output Delay: Unknown · Hardware Required"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Preamp"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Reset"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Setup"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator Output · SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Snapshot Library"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Store"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Two editable parameters · Global Tap Tempo · SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Visible in Mixer and Stage"),
    QT_TRANSLATE_NOOP("Flow8Ui", "10 dBV Output Level"),
    QT_TRANSLATE_NOOP("Flow8Ui", "4 Sends"),
    QT_TRANSLATE_NOOP("Flow8Ui", "9-band EQ"),
    QT_TRANSLATE_NOOP("Flow8Ui", "9-band EQ · Limiter"),
    QT_TRANSLATE_NOOP("Flow8Ui", "App Library"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Apply in Simulator"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Assisted Setup"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Attack"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Authenticating"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Available"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Balance"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Back"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Cancel"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel Icon"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel Name"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel Visibility"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel Sends · Pre/Post-Fader"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Channel fader"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Close"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Compressor"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Compressor Amount"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connect"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connected"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connecting"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connection"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connect the instrument through the appropriate input path."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connect the line-level source to a compatible input."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connect the microphone with an XLR cable."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Connect with an XLR cable. Phantom Power applies only to Input 1 or Input 2."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Continue Session"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Control"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Control Gesture"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Digital Input"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Delay: %1 ms"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Delay: Unknown"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Diagnostics"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Dynamic Microphone"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Dynamic microphone starting preset"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Disconnect"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Disconnected"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Effect Type"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Empty Slot"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Engine"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Equalizer"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Error"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Fader"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Fader; double-click to reset to 0 dB"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FLOW 8 PC Controller — Simulator"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FLOW 8 Settings"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FLOW 8 Preferences"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX %1 · Independent Engine"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 1 → Monitor 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 1 → Monitor 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 2 → Monitor 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 2 → Monitor 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 1 → Main"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 2 → Main"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Frequency"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Gain"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Gain Reduction"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Gain: %1   48 V: %2   Low Cut: %3   EQ: 4-band parametric   Compressor: %4   Sends: MON1 / MON2 / FX1 / FX2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Guitar / Bass"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Guitar / Bass starting preset"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Hardware BLE and MIDI controls are not available in this build."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Hardware control for advanced parameters is unavailable in this build."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Hardware Slots · 15"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Hardware"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Headphone Source"),
    QT_TRANSLATE_NOOP("Flow8Ui", "High"),
    QT_TRANSLATE_NOOP("Flow8Ui", "High Mid"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Inferred"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input %1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input 3"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input 4"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input 5/6"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input 7/8"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input gain (simulator model)"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Instrument"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Large controls for fast live operation"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Language"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Level"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Limiter"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Linear"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Line Instrument"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Line instrument starting preset"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Load Snapshot"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Load in Simulator"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Low"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Low Cut"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Low Mid"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Main"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Main Mix"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Main mix sends · Master · Balance · 9-band EQ · Limiter"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Master"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Makeup Gain"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Microphone"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Microphone / Line"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Mixer"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Mono"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Monitor 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Monitor 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Monitor 1 Send"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Monitor 2 Send"),
    QT_TRANSLATE_NOOP("Flow8Ui", "MON1/2 Linked"),
    QT_TRANSLATE_NOOP("Flow8Ui", "MON1/2 LINKED"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Mute"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Needs Hardware Verification"),
    QT_TRANSLATE_NOOP("Flow8Ui", "No app-library snapshots yet.\nThis is separate from the 15 hardware slots."),
    QT_TRANSLATE_NOOP("Flow8Ui", "No Input Selected"),
    QT_TRANSLATE_NOOP("Flow8Ui", "No Icon"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Next"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Not supported"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Official mapping model ready; transport not implemented"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Official capability · Simulator only · BLE routing protocol UNKNOWN"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Output"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Output hardware values are unknown until a FLOW 8 is connected and verified."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Pan"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Pan / Balance"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Parameter %1 (UNKNOWN)"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Parametric"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Playback"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Post-Fader"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Pre-Fader"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Preferences"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Phantom Power"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Polarity"),
    QT_TRANSLATE_NOOP("Flow8Ui", "PC Controller"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Preset"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Preset %1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Preset-specific type (unavailable)"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Q"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Ratio"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Ready (Simulator)"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Recall in Simulator"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Request / Save MIDI SysEx Dump"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Rename"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Release"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Routing"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Routing · Source → Destination"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Rotary"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Scanning"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Send"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Sends"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Settings"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Share / Export"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Show %1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Show Channel Icons"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Show Mute Buttons"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Show Output Delay Indicator"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator (SYNTHETIC)"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator / BLE when available"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator connected locally"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator mode · Synthetic mixer data · No FLOW 8 hardware connected"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator · SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Simulator applies a SYNTHETIC starting point. Hardware commands remain unavailable."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Solo"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Snapshot"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Snapshot export is not implemented yet."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Snapshot Name"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Stage"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Stage View"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Standard"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Standard / Parametric is a PC interaction preference, not a protocol claim."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Start a FLOW 8 Session"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Start New"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Step 1 · Select Input"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Step 2 · Select Source Type"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Step 3 · Recommended Preset"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Step 4 · Connection and Apply"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Store in App Library"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Stereo Bus"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Stereo Pair"),
    QT_TRANSLATE_NOOP("Flow8Ui", "SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Input sends · Independent engine · Output routing · SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Stereo link relates MON1 and MON2 while preserving two independent bus states."),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 1 Send"),
    QT_TRANSLATE_NOOP("Flow8Ui", "FX 2 Send"),
    QT_TRANSLATE_NOOP("Flow8Ui", "SYNTHETIC data, deterministic state"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Synchronizing"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Tap Tempo"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Tap tempo is global in the official MIDI chart and applies only to compatible effects.\nEffect-specific parameter names remain unavailable."),
    QT_TRANSLATE_NOOP("Flow8Ui", "The simulator is deterministic test data and is not a FLOW 8 hardware claim."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Threshold"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Transport"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Type"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Unavailable"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Unavailable for hardware control"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Unknown"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Untitled Snapshot"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB Mode"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB Streaming"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB Recording"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB, FX and Headphone Routing"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Input 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Input 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Input 3"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Input 4"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Input 5/6"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Input 7/8"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → USB / Bluetooth"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Monitor 1"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB → Monitor 2"),
    QT_TRANSLATE_NOOP("Flow8Ui", "USB / Bluetooth"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Verified"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Visible in Mixer and Stage"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Choose how you want to begin"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Choose the input you want to prepare."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Choose the source connected to this input."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Review the synthetic starting point."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Check the connection before applying."),
    QT_TRANSLATE_NOOP("Flow8Ui", "Full"),
    QT_TRANSLATE_NOOP("Flow8Ui", "Footswitch Mode"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EQ Editing Mode"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EZ-GAIN Selected"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EZ-GAIN All Inputs"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EZ-GAIN running · %1 s · SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EZ-GAIN cancelled"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EZ-GAIN complete · SYNTHETIC"),
    QT_TRANSLATE_NOOP("Flow8Ui", "EZ-GAIN ready"),
    QT_TRANSLATE_NOOP("Flow8Ui", "MIDI SysEx dump is an official capability. Requesting a real dump needs hardware."),
};

} // namespace

QString uiText(const char* sourceText)
{
    return QCoreApplication::translate("Flow8Ui", sourceText);
}

QString inputDisplayName(const model::InputId input)
{
    switch (input) {
    case model::InputId::Input1: return uiText("Input 1");
    case model::InputId::Input2: return uiText("Input 2");
    case model::InputId::Input3: return uiText("Input 3");
    case model::InputId::Input4: return uiText("Input 4");
    case model::InputId::Input56: return uiText("Input 5/6");
    case model::InputId::Input78: return uiText("Input 7/8");
    case model::InputId::UsbBluetooth: return uiText("USB / Bluetooth");
    }
    return uiText("Unknown");
}

QString busDisplayName(const model::BusId bus)
{
    switch (bus) {
    case model::BusId::Main: return uiText("Main");
    case model::BusId::Monitor1: return uiText("Monitor 1");
    case model::BusId::Monitor2: return uiText("Monitor 2");
    case model::BusId::Fx1: return uiText("FX 1");
    case model::BusId::Fx2: return uiText("FX 2");
    }
    return uiText("Unknown");
}

QString inputTypeDisplayName(const model::InputType type)
{
    switch (type) {
    case model::InputType::Microphone: return uiText("Microphone");
    case model::InputType::MicrophoneLine: return uiText("Microphone / Line");
    case model::InputType::StereoLinePair: return uiText("Stereo Pair");
    case model::InputType::UsbBluetooth: return uiText("Digital Input");
    }
    return uiText("Input");
}

QString evidenceDisplayName(const model::EvidenceStatus evidence)
{
    switch (evidence) {
    case model::EvidenceStatus::Verified: return uiText("Verified");
    case model::EvidenceStatus::Inferred: return uiText("Inferred");
    case model::EvidenceStatus::Unknown: return uiText("Unknown");
    case model::EvidenceStatus::Blocked: return uiText("Needs Hardware Verification");
    }
    return uiText("Unknown");
}

} // namespace flow8::ui
