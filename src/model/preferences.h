#pragma once

namespace flow8::model {

// These are desktop-application preferences. They describe presentation and
// interaction, not mixer observations and not FLOW 8 protocol fields.
enum class ControlGesture {
    Linear,
    Rotary,
};

// "Standard" is a desktop UI abstraction. Official materials establish that
// FLOW Mix exposes a parametric-EQ preference, but not a BLE representation.
enum class EqEditingMode {
    Standard,
    Parametric,
};

enum class FootswitchMode {
    Fx,
    Snapshot,
};

struct AppPreferences {
    bool showMuteButtons {true};
    bool showChannelIcons {true};
    ControlGesture controlGesture {ControlGesture::Linear};
    EqEditingMode eqEditingMode {EqEditingMode::Parametric};
    bool showOutputDelayIndicator {true};
    FootswitchMode footswitchMode {FootswitchMode::Fx};
};

} // namespace flow8::model
