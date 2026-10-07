use flow8_ble::{NativeConnectionStage, SessionPhase};
use flow8_core::SessionState;
use flow8_model::{HeadphoneSource, InputId, MixDestination, TapPoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Language {
    English,
    Chinese,
}

impl Language {
    pub(super) fn tr(self, english: &'static str, chinese: &'static str) -> &'static str {
        match self {
            Self::English => english,
            Self::Chinese => chinese,
        }
    }
}

pub(super) fn waiting_for_device_text(language: Language) -> &'static str {
    language.tr("Waiting for device", "等待设备数据")
}

pub(super) fn setting_switch_text(language: Language, enabled: bool) -> &'static str {
    if enabled {
        language.tr("On", "开启")
    } else {
        language.tr("Off", "关闭")
    }
}

pub(super) fn destination_name(destination: MixDestination) -> &'static str {
    match destination {
        MixDestination::Main => "MAIN",
        MixDestination::Monitor1 => "MON1",
        MixDestination::Monitor2 => "MON2",
        MixDestination::Fx1 => "FX1",
        MixDestination::Fx2 => "FX2",
    }
}
pub(super) fn channel_number(id: InputId) -> &'static str {
    match id {
        InputId::Input1 => "CH 1",
        InputId::Input2 => "CH 2",
        InputId::Input3 => "CH 3",
        InputId::Input4 => "CH 4",
        InputId::Input56 => "CH 5/6",
        InputId::Input78 => "CH 7/8",
        InputId::UsbBluetooth => "USB / BT",
    }
}
pub(super) fn display_name(id: InputId, language: Language) -> &'static str {
    match (id, language) {
        (InputId::Input1, Language::English) => "Input 1",
        (InputId::Input2, Language::English) => "Input 2",
        (InputId::Input3, Language::English) => "Input 3",
        (InputId::Input4, Language::English) => "Input 4",
        (InputId::Input56, Language::English) => "Input 5/6",
        (InputId::Input78, Language::English) => "Input 7/8",
        (InputId::UsbBluetooth, Language::English) => "USB / Bluetooth",
        (InputId::Input1, Language::Chinese) => "输入 1",
        (InputId::Input2, Language::Chinese) => "输入 2",
        (InputId::Input3, Language::Chinese) => "输入 3",
        (InputId::Input4, Language::Chinese) => "输入 4",
        (InputId::Input56, Language::Chinese) => "输入 5/6",
        (InputId::Input78, Language::Chinese) => "输入 7/8",
        (InputId::UsbBluetooth, Language::Chinese) => "USB / 蓝牙",
    }
}
pub(super) fn headphone_source_text(source: HeadphoneSource, language: Language) -> &'static str {
    match source {
        HeadphoneSource::Main => language.tr("MAIN mix", "MAIN 混音"),
        HeadphoneSource::Monitor => language.tr("Monitor mix", "监听混音"),
    }
}
pub(super) fn tap_point_text(tap: TapPoint, language: Language) -> &'static str {
    match tap {
        TapPoint::PreFader => language.tr("Pre-fader", "推子前"),
        TapPoint::PostFader => language.tr("Post-fader", "推子后"),
    }
}
pub(super) fn session_phase_text(phase: SessionPhase, language: Language) -> &'static str {
    match phase {
        SessionPhase::Disconnected => language.tr("Disconnected", "已断开"),
        SessionPhase::Scanning => language.tr("Scanning", "正在扫描"),
        SessionPhase::Connecting => language.tr("Connecting", "正在连接"),
        SessionPhase::GattReady => language.tr("Device connected", "设备已连接"),
        SessionPhase::RxArming => language.tr("Preparing device updates", "正在准备设备更新"),
        SessionPhase::Handshaking => language.tr("Completing connection", "正在完成连接"),
        SessionPhase::StateSyncing => language.tr("Synchronizing state", "正在同步状态"),
        SessionPhase::Ready => language.tr("Ready", "已就绪"),
        SessionPhase::Error => language.tr("Error", "错误"),
    }
}

pub(super) fn session_state_text(state: SessionState, language: Language) -> &'static str {
    match state {
        SessionState::Disconnected => language.tr("Disconnected", "已断开"),
        SessionState::Scanning => language.tr("Scanning", "正在扫描"),
        SessionState::Connecting => language.tr("Connecting", "正在连接"),
        SessionState::GattReady => language.tr("Device connected", "设备已连接"),
        SessionState::RxArming => language.tr("Preparing device updates", "正在准备设备更新"),
        SessionState::Handshaking => language.tr("Completing connection", "正在完成连接"),
        SessionState::StateSyncing => language.tr("Synchronizing state", "正在同步状态"),
        SessionState::Ready => language.tr("Ready", "已就绪"),
        SessionState::Error => language.tr("Error", "错误"),
    }
}

pub(super) fn native_stage_text(stage: NativeConnectionStage, language: Language) -> &'static str {
    match stage {
        NativeConnectionStage::Scanning => language.tr("Scanning", "正在扫描"),
        NativeConnectionStage::DeviceFound => {
            language.tr("FLOW 8 device found", "已找到 FLOW 8 设备")
        }
        NativeConnectionStage::DeviceObjectCreated => {
            language.tr("Connecting to FLOW 8", "正在连接 FLOW 8")
        }
        NativeConnectionStage::NativeInterfaceEnumerating => {
            language.tr("Connecting to FLOW 8", "正在连接 FLOW 8")
        }
        NativeConnectionStage::FlowServiceSelected => {
            language.tr("Preparing FLOW 8 controls", "正在准备 FLOW 8 控制")
        }
        NativeConnectionStage::NativeServiceHandleOpened => {
            language.tr("Preparing FLOW 8 controls", "正在准备 FLOW 8 控制")
        }
        NativeConnectionStage::CharacteristicsEnumerated => {
            language.tr("Preparing FLOW 8 controls", "正在准备 FLOW 8 控制")
        }
        NativeConnectionStage::TargetCharacteristicFound => {
            language.tr("FLOW 8 controls available", "FLOW 8 控制已就绪")
        }
        NativeConnectionStage::NativeRxRegistering => {
            language.tr("Preparing device updates", "正在准备设备更新")
        }
        NativeConnectionStage::NativeRxArmed => {
            language.tr("Waiting for FLOW 8", "正在等待 FLOW 8 响应")
        }
        NativeConnectionStage::Handshaking => language.tr("Completing connection", "正在完成连接"),
    }
}

pub(super) fn localized_status_message(state: SessionState, language: Language) -> String {
    format!(
        "{}: {}",
        language.tr("Bluetooth", "蓝牙"),
        session_state_text(state, language)
    )
}
