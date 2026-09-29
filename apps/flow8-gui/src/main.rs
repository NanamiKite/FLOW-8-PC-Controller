use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};
use flow8_ble::{DeviceCommand, DeviceEvent, DeviceRuntime, NativeConnectionStage, SessionPhase};
use flow8_core::{Flow8State, Flow8Store, KnownSetting, MuteTarget, SemanticCommand, SessionState};
use flow8_model::{
    EvidenceStatus, FxId, HeadphoneSource, InputChannelState, InputId, MixBusId, MixDestination,
    MonitorRoutingSource, ParameterSpec, TapPoint, specs,
};

const BG: Color32 = Color32::from_rgb(17, 19, 23);
const SURFACE: Color32 = Color32::from_rgb(24, 27, 32);
const SURFACE_ALT: Color32 = Color32::from_rgb(27, 30, 36);
const BORDER: Color32 = Color32::from_rgb(43, 47, 54);
const TEXT: Color32 = Color32::from_rgb(230, 233, 238);
const SECONDARY: Color32 = Color32::from_rgb(142, 150, 162);
const YELLOW: Color32 = Color32::from_rgb(242, 201, 76);
const PURPLE: Color32 = Color32::from_rgb(155, 106, 214);
const GREEN: Color32 = Color32::from_rgb(85, 185, 121);
const BLUE: Color32 = Color32::from_rgb(79, 146, 232);
const RED: Color32 = Color32::from_rgb(215, 71, 82);
const UI_SCALE_MIN: f32 = 0.85;
const UI_SCALE_MAX: f32 = 1.40;

#[derive(Debug, Clone, Copy, PartialEq)]
struct UiMetrics {
    ui_scale: f32,
    body_font: f32,
    button_font: f32,
    secondary_font: f32,
    small_font: f32,
    heading_font: f32,
    control_height: f32,
    spacing: f32,
}

impl UiMetrics {
    fn calculate(available: Vec2, _pixels_per_point: f32, user_scale: f32) -> Self {
        let user_scale = user_scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX);
        let viewport_scale = (available.x / 1440.0).clamp(0.90, 1.10);
        let scale = (user_scale * viewport_scale).clamp(UI_SCALE_MIN, 1.45);
        Self {
            ui_scale: scale,
            body_font: 14.0 * scale,
            button_font: 13.0 * scale,
            secondary_font: 11.0 * scale,
            small_font: 10.0 * scale,
            heading_font: 22.0 * scale,
            control_height: 30.0 * scale,
            spacing: 8.0 * scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LayoutMetrics {
    fader_height: f32,
    channel_width: f32,
    inspector_width: f32,
    compact: bool,
    mixer_scrolls: bool,
}

impl LayoutMetrics {
    fn calculate(available: Vec2, ui: UiMetrics, visible_channels: usize) -> Self {
        let available_width = available.x.max(0.0);
        let compact = available_width < 1180.0 * ui.ui_scale;
        let wide = available_width >= 1760.0 * ui.ui_scale;
        let inspector_width = (available_width * if wide { 0.24 } else { 0.27 })
            .clamp(290.0 * ui.ui_scale, 430.0 * ui.ui_scale);
        let channel_area = if compact {
            available_width
        } else {
            (available_width - inspector_width - ui.spacing).max(0.0)
        };
        let channel_min_width = 100.0 * ui.ui_scale;
        let channel_max_width = 146.0 * ui.ui_scale;
        let channels = visible_channels.max(1) as f32;
        let gaps = ui.spacing * (channels - 1.0);
        let natural_channel_width = ((channel_area - gaps) / channels).max(0.0);
        let fader_height = 260.0 * ui.ui_scale;
        Self {
            fader_height,
            channel_width: natural_channel_width.clamp(channel_min_width, channel_max_width),
            inspector_width,
            compact,
            mixer_scrolls: natural_channel_width < channel_min_width,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    English,
    Chinese,
}

impl Language {
    fn tr(self, english: &'static str, chinese: &'static str) -> &'static str {
        match self {
            Self::English => english,
            Self::Chinese => chinese,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Mixer,
    Stage,
    Fx1,
    Fx2,
    Monitor1,
    Monitor2,
    Main,
    MainOut,
    Routing,
    Snapshots,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Simulator,
    Ble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlGesture {
    Linear,
    Rotary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EqEditingMode {
    Standard,
    Parametric,
}

#[derive(Debug, Clone)]
struct AppPreferences {
    show_mute_buttons: bool,
    show_channel_icons: bool,
    channel_visible: [bool; 7],
    control_gesture: ControlGesture,
    eq_editing_mode: EqEditingMode,
    show_output_delay_indicator: bool,
    ui_scale: f32,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            show_mute_buttons: true,
            show_channel_icons: true,
            channel_visible: [true; 7],
            control_gesture: ControlGesture::Linear,
            eq_editing_mode: EqEditingMode::Parametric,
            show_output_delay_indicator: true,
            ui_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct FontFallbackStatus {
    family: Option<&'static str>,
    path: Option<PathBuf>,
}

#[derive(Clone)]
struct LocalSnapshot {
    name: String,
    state: Flow8State,
}

struct Flow8App {
    store: Flow8Store,
    page: Page,
    language: Language,
    last_frame: Instant,
    message: String,
    mode: RunMode,
    runtime: DeviceRuntime,
    discovered_devices: Vec<flow8_ble::DiscoveredDevice>,
    stage_layout_mode: bool,
    stage_positions: [Vec2; 7],
    phantom_confirmation: Option<InputId>,
    snapshot_delete_confirmation: Option<u8>,
    local_snapshots: Vec<LocalSnapshot>,
    state_sync_applied: bool,
    native_stage: Option<NativeConnectionStage>,
    last_ble_error: Option<String>,
    preferences: AppPreferences,
    font_fallback: FontFallbackStatus,
    metrics: UiMetrics,
}

impl Flow8App {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        Self::from_context(&context.egui_ctx)
    }

    fn from_context(context: &egui::Context) -> Self {
        let font_fallback = configure_fonts(context);
        let metrics =
            UiMetrics::calculate(Vec2::new(1440.0, 920.0), context.pixels_per_point(), 1.0);
        configure_style(context, metrics);
        Self {
            store: Flow8Store::simulator(),
            page: Page::Mixer,
            language: Language::English,
            last_frame: Instant::now(),
            message: "Simulator mode · Synthetic mixer data · No FLOW 8 hardware connected".into(),
            mode: RunMode::Simulator,
            runtime: DeviceRuntime::spawn(*b"FLOW8-PC-RUST001"),
            discovered_devices: Vec::new(),
            stage_layout_mode: false,
            stage_positions: std::array::from_fn(|index| {
                Vec2::new(
                    34.0 + (index % 4) as f32 * 190.0,
                    42.0 + (index / 4) as f32 * 210.0,
                )
            }),
            phantom_confirmation: None,
            snapshot_delete_confirmation: None,
            local_snapshots: Vec::new(),
            state_sync_applied: false,
            native_stage: None,
            last_ble_error: None,
            preferences: AppPreferences::default(),
            font_fallback,
            metrics,
        }
    }

    fn dispatch(&mut self, command: SemanticCommand) {
        if self.mode == RunMode::Ble && self.store.state.session != SessionState::Ready {
            self.message = self
                .language
                .tr(
                    "Wait for a complete FLOW 8 state sync before editing.",
                    "请等待 FLOW 8 完成状态同步后再进行调整。",
                )
                .into();
            return;
        }
        if let Err(error) = self.store.dispatch(command) {
            self.message = error.to_string();
        }
    }

    fn poll_runtime(&mut self) {
        while let Some(event) = self.runtime.try_recv() {
            match event {
                DeviceEvent::Phase(phase) => {
                    if matches!(phase, SessionPhase::Connecting | SessionPhase::StateSyncing) {
                        self.state_sync_applied = false;
                    }
                    if phase == SessionPhase::Ready {
                        self.last_ble_error = None;
                    }
                    if phase == SessionPhase::Ready && !self.state_sync_applied {
                        self.store.state.session = SessionState::Error;
                        self.message = self
                            .language
                            .tr(
                                "BLE reported Ready before a complete mixer state was applied",
                                "BLE 在完整混音状态写入前错误地报告了就绪",
                            )
                            .into();
                    } else {
                        self.store.state.session = core_session_state(phase);
                        self.message = format!(
                            "{}: {}",
                            self.language.tr("Direct Bluetooth", "直连蓝牙"),
                            session_phase_text(phase, self.language)
                        );
                    }
                }
                DeviceEvent::ConnectionStage(stage) => {
                    self.native_stage = Some(stage);
                    if stage == NativeConnectionStage::NativeRxArmed {
                        self.last_ble_error = None;
                    }
                    self.message = native_stage_text(stage, self.language).into();
                }
                DeviceEvent::ScanResults(devices) => {
                    self.message = format!(
                        "{}: {}",
                        self.language.tr("BLE devices found", "扫描到的 BLE 设备"),
                        devices.len()
                    );
                    self.discovered_devices = devices;
                }
                DeviceEvent::Received(command) => {
                    let is_mixer_state =
                        matches!(&command, flow8_protocol::RxCommand::MixerState(_));
                    if let Err(error) = self
                        .store
                        .apply_rx(command, EvidenceStatus::VerifiedFromDevice)
                    {
                        self.message = format!(
                            "{}: {error}",
                            self.language.tr("RX state error", "接收状态错误")
                        );
                    } else if is_mixer_state {
                        self.state_sync_applied = true;
                        tracing::info!(
                            evidence = "VERIFIED_FROM_DEVICE",
                            "complete FLOW 8 state atomically applied to confirmed Store"
                        );
                        if let Err(error) = self.runtime.send(DeviceCommand::StateApplied) {
                            self.store.state.session = SessionState::Error;
                            self.message = error;
                        }
                    }
                }
                DeviceEvent::RawRx(bytes) => {
                    self.message = format!("RX {}", bytes_to_hex(&bytes));
                }
                DeviceEvent::RawTx(bytes) => {
                    self.message = format!("TX {}", bytes_to_hex(&bytes));
                }
                DeviceEvent::WriteMode(mode) => {
                    self.message = format!(
                        "{}: {mode:?}",
                        self.language.tr("GATT write mode", "GATT 写入模式")
                    );
                }
                DeviceEvent::Backend(backend) => {
                    self.message =
                        format!("{}: {backend}", self.language.tr("BLE backend", "BLE 后端"));
                }
                DeviceEvent::Mtu(mtu) => {
                    self.message = format!(
                        "{}: {mtu}",
                        self.language
                            .tr("BLE negotiated/reported PDU", "BLE 协商/报告的 PDU")
                    );
                }
                DeviceEvent::Error(error) => {
                    self.store.state.session = SessionState::Error;
                    self.last_ble_error = Some(error.clone());
                    self.message = format!("BLE: {error}");
                }
            }
        }

        while let Some(command) = self.store.queue.pop() {
            if self.mode == RunMode::Ble {
                if let Err(error) = self
                    .runtime
                    .send(DeviceCommand::Send(command.to_protocol()))
                {
                    self.message = error;
                    break;
                }
            }
        }
    }

    fn connection_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SURFACE)
            .inner_margin(egui::Margin::symmetric(18, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("FLOW 8")
                            .size(22.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        egui::RichText::new(self.language.tr("PC CONTROLLER", "PC 控制器"))
                            .size(10.0)
                            .color(SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(self.language.tr("Preferences", "偏好设置"))
                            .clicked()
                        {
                            self.page = Page::Settings;
                        }
                        if ui.button(self.language.tr("Setup", "设置")).clicked() {
                            self.page = Page::Routing;
                        }
                        let language_label = match self.language {
                            Language::English => "中文",
                            Language::Chinese => "English",
                        };
                        if ui.button(language_label).clicked() {
                            self.language = if self.language == Language::English {
                                Language::Chinese
                            } else {
                                Language::English
                            };
                            self.message = localized_status_message(
                                self.mode,
                                self.store.state.session,
                                self.language,
                            );
                            ui.ctx().request_repaint();
                        }
                        if ui.button(self.language.tr("Disconnect", "断开")).clicked() {
                            let _ = self.runtime.send(DeviceCommand::Disconnect);
                        }
                        let can_connect = self.mode != RunMode::Ble
                            || matches!(
                                self.store.state.session,
                                SessionState::Disconnected | SessionState::Error
                            );
                        if ui
                            .add_enabled(
                                can_connect,
                                egui::Button::new(self.language.tr("Connect", "连接")),
                            )
                            .clicked()
                        {
                            if self.mode != RunMode::Ble {
                                self.mode = RunMode::Ble;
                                self.store = Flow8Store::disconnected();
                            }
                            match self.runtime.send(DeviceCommand::Connect) {
                                Ok(()) => {
                                    self.store.state.session = SessionState::Connecting;
                                    self.native_stage = None;
                                    self.last_ble_error = None;
                                }
                                Err(error) => self.message = error,
                            }
                        }
                        let can_scan = self.mode != RunMode::Ble
                            || matches!(
                                self.store.state.session,
                                SessionState::Disconnected | SessionState::Error
                            );
                        let scan = ui
                            .add_enabled(
                                can_scan,
                                egui::Button::new(self.language.tr("Scan", "扫描")),
                            )
                            .on_disabled_hover_text(self.language.tr(
                                "Disconnect before scanning for devices.",
                                "请先断开当前设备，再扫描其他设备。",
                            ));
                        if scan.clicked() {
                            if self.mode != RunMode::Ble {
                                self.mode = RunMode::Ble;
                                self.store = Flow8Store::disconnected();
                            }
                            match self.runtime.send(DeviceCommand::Scan {
                                duration: Duration::from_secs(4),
                            }) {
                                Ok(()) => {
                                    self.store.state.session = SessionState::Scanning;
                                    self.native_stage = None;
                                    self.last_ble_error = None;
                                }
                                Err(error) => self.message = error,
                            }
                        }
                        if ui
                            .selectable_label(
                                self.mode == RunMode::Ble,
                                self.language.tr("Direct Bluetooth", "直连蓝牙"),
                            )
                            .clicked()
                            && self.mode != RunMode::Ble
                        {
                            self.mode = RunMode::Ble;
                            self.store = Flow8Store::disconnected();
                            self.message = localized_status_message(
                                self.mode,
                                self.store.state.session,
                                self.language,
                            );
                        }
                        if ui
                            .selectable_label(
                                self.mode == RunMode::Simulator,
                                self.language.tr("Simulator", "模拟器"),
                            )
                            .clicked()
                        {
                            let _ = self.runtime.send(DeviceCommand::Disconnect);
                            self.mode = RunMode::Simulator;
                            self.store = Flow8Store::simulator();
                            self.message = self
                                .language
                                .tr("Simulator · SYNTHETIC", "模拟器 · SYNTHETIC")
                                .into();
                        }
                        let status = match self.mode {
                            RunMode::Simulator => self
                                .language
                                .tr("● Ready (Simulator)", "● 已就绪（模拟器）")
                                .to_owned(),
                            RunMode::Ble => format!(
                                "● {}",
                                session_state_text(self.store.state.session, self.language)
                            ),
                        };
                        ui.label(egui::RichText::new(status).strong().color(
                            if self.store.state.session == SessionState::Error {
                                RED
                            } else {
                                BLUE
                            },
                        ));
                    });
                });
                if let Some(error) = &self.last_ble_error {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}：{}",
                            self.language.tr("Connection failed", "连接失败"),
                            error
                        ))
                        .color(RED),
                    );
                } else if let Some(stage) = self.native_stage {
                    ui.label(
                        egui::RichText::new(native_stage_text(stage, self.language))
                            .size(10.0)
                            .color(SECONDARY),
                    );
                }
            });
    }

    fn layer_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(Color32::from_rgb(20, 23, 27))
            .inner_margin(egui::Margin::symmetric(18, 7))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.layer_button(ui, Page::Mixer, self.language.tr("MIXER", "混音器"), YELLOW);
                    self.layer_button(ui, Page::Stage, self.language.tr("STAGE", "舞台"), YELLOW);
                    self.layer_button(ui, Page::Fx1, "FX1", PURPLE);
                    self.layer_button(ui, Page::Fx2, "FX2", PURPLE);
                    self.layer_button(ui, Page::Monitor1, "MON1", GREEN);
                    self.layer_button(ui, Page::Monitor2, "MON2", GREEN);
                    self.layer_button(ui, Page::Main, "MAIN", TEXT);
                    self.layer_button(
                        ui,
                        Page::MainOut,
                        self.language.tr("MAIN OUT", "主输出"),
                        TEXT,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_tab(
                            ui,
                            self.page == Page::Snapshots,
                            self.language.tr("Snapshots", "快照"),
                            TEXT,
                        )
                        .clicked()
                        {
                            self.page = Page::Snapshots;
                        }
                        if small_tab(
                            ui,
                            self.page == Page::Routing,
                            self.language.tr("Routing", "路由"),
                            TEXT,
                        )
                        .clicked()
                        {
                            self.page = Page::Routing;
                        }
                    });
                });
            });
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0, BORDER),
        );
    }

    fn layer_button(&mut self, ui: &mut egui::Ui, page: Page, label: &str, accent: Color32) {
        if small_tab(ui, self.page == page, label, accent).clicked() {
            self.page = page;
            let destination = match page {
                Page::Fx1 => Some(MixDestination::Fx1),
                Page::Fx2 => Some(MixDestination::Fx2),
                Page::Monitor1 => Some(MixDestination::Monitor1),
                Page::Monitor2 => Some(MixDestination::Monitor2),
                Page::Main | Page::Mixer => Some(MixDestination::Main),
                _ => None,
            };
            if let Some(destination) = destination {
                self.store.state.selected_destination = destination;
            }
        }
    }

    fn mixer(&mut self, ui: &mut egui::Ui) {
        let metrics = self.metrics;
        ui.add_space(metrics.spacing);
        egui::ScrollArea::horizontal()
            .id_salt("mixer-destination-scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(metrics.spacing * 2.0);
                    ui.label(
                        egui::RichText::new(self.language.tr("MIX DESTINATION", "混音目标"))
                            .size(metrics.small_font)
                            .strong()
                            .color(SECONDARY),
                    );
                    for destination in MixDestination::ALL {
                        let selected = self.store.state.selected_destination == destination;
                        if destination_button(ui, selected, destination_name(destination)).clicked()
                        {
                            self.store.state.selected_destination = destination;
                        }
                    }
                    ui.label(
                        egui::RichText::new(self.language.tr(
                            "7 conventional inputs · USB returns are configured in Routing",
                            "7 个常规输入 · USB 回放在路由中配置",
                        ))
                        .size(metrics.secondary_font)
                        .color(SECONDARY),
                    );
                });
            });
        ui.add_space(metrics.spacing);

        let available = ui.available_size();
        let visible_channels = InputId::ALL
            .iter()
            .filter(|id| self.preferences.channel_visible[id.index()])
            .count();
        let layout = LayoutMetrics::calculate(available, metrics, visible_channels);
        let inspector_width = layout.inspector_width;
        let stack_inspector = layout.compact;
        let fader_height = layout.fader_height;

        egui::ScrollArea::vertical()
            .id_salt("mixer-vertical")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if stack_inspector {
                    let channel_area = ui.available_width();
                    self.render_channel_strips(ui, channel_area, visible_channels, fader_height);
                    ui.add_space(metrics.spacing);
                    let inspector_width = ui
                        .available_width()
                        .min((760.0 * metrics.ui_scale).max(layout.inspector_width));
                    ui.allocate_ui_with_layout(
                        Vec2::new(inspector_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| self.master_and_inspector(ui, layout),
                    );
                } else {
                    ui.horizontal_top(|ui| {
                        let channel_area =
                            (ui.available_width() - inspector_width - metrics.spacing).max(0.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(channel_area, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                self.render_channel_strips(
                                    ui,
                                    channel_area,
                                    visible_channels,
                                    fader_height,
                                );
                            },
                        );
                        ui.add_space(metrics.spacing);
                        ui.allocate_ui_with_layout(
                            Vec2::new(inspector_width, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| self.master_and_inspector(ui, layout),
                        );
                    });
                }
            });
    }

    fn render_channel_strips(
        &mut self,
        ui: &mut egui::Ui,
        available_width: f32,
        visible_channels: usize,
        fader_height: f32,
    ) {
        let metrics = self.metrics;
        let layout = LayoutMetrics::calculate(
            Vec2::new(available_width, ui.available_height()),
            metrics,
            visible_channels,
        );
        let strip_width = layout.channel_width;
        egui::ScrollArea::horizontal()
            .id_salt("mixer-strips")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = metrics.spacing;
                    for id in InputId::ALL {
                        if !self.preferences.channel_visible[id.index()] {
                            continue;
                        }
                        let channel = self.store.state.channels[id.index()].clone();
                        let selected = self.store.state.selected_input == Some(id);
                        let action = channel_strip(
                            ui,
                            &channel,
                            self.store.state.selected_destination,
                            selected,
                            self.language,
                            self.preferences.show_channel_icons,
                            self.preferences.show_mute_buttons,
                            metrics,
                            strip_width,
                            fader_height,
                        );
                        if let Some(action) = action {
                            self.apply_strip_action(id, action);
                        }
                    }
                });
            });
    }

    fn apply_strip_action(&mut self, id: InputId, action: StripAction) {
        self.store.state.selected_input = Some(id);
        match action {
            StripAction::Select => {}
            StripAction::Route(value) => self.dispatch(SemanticCommand::SetRouteLevel {
                source: id,
                destination: self.store.state.selected_destination,
                normalized: value,
            }),
            StripAction::Pan(value) => self.dispatch(SemanticCommand::SetPan { input: id, value }),
            StripAction::Mute(value) => self.dispatch(SemanticCommand::SetMute {
                input: id,
                enabled: value,
            }),
            StripAction::Solo(value) => self.dispatch(SemanticCommand::SetSolo {
                input: id,
                enabled: value,
            }),
        }
    }

    fn master_and_inspector(&mut self, ui: &mut egui::Ui, _layout: LayoutMetrics) {
        let destination = self.store.state.selected_destination;
        card(ui, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    destination_name(destination),
                    self.language.tr("MASTER", "主控")
                ))
                .size(14.0)
                .strong()
                .color(TEXT),
            );
            ui.label(
                egui::RichText::new(self.language.tr("Destination master", "目标总线主控"))
                    .size(10.0)
                    .color(SECONDARY),
            );
            let (mut master, meter) = if let Some(bus) =
                self.store.state.bus_for_destination(destination)
            {
                (
                    *bus.master_level.effective().unwrap_or(&0.75),
                    *bus.meter.level_db.effective().unwrap_or(&-60.0),
                )
            } else {
                let fx = &self.store.state.effects[usize::from(destination == MixDestination::Fx2)];
                (*fx.master_level.effective().unwrap_or(&0.75), -18.0)
            };
            let master_height = 220.0 * self.metrics.ui_scale;
            ui.horizontal(|ui| {
                if fader_sized(
                    ui,
                    &mut master,
                    70.0 * self.metrics.ui_scale,
                    master_height,
                    0.75,
                )
                .changed()
                {
                    self.dispatch(SemanticCommand::SetDestinationMaster {
                        destination,
                        normalized: master,
                    });
                }
                meter_widget_sized(
                    ui,
                    meter,
                    50.0 * self.metrics.ui_scale,
                    master_height,
                    self.metrics,
                );
            });
            ui.label(
                egui::RichText::new(format!("{:.1} dB", normalized_to_display_db(master)))
                    .strong()
                    .color(TEXT),
            );
        });
        if let Some(bus_id) = bus_id_for_destination(destination)
            && let Some(bus) = self.store.state.bus_for_destination(destination).cloned()
        {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.language.tr("OUTPUT PROCESSING", "输出处理"))
                        .size(10.0)
                        .strong()
                        .color(SECONDARY),
                );
                ui.horizontal(|ui| {
                    let muted = bus.muted.effective().copied().unwrap_or(false);
                    if state_button(ui, muted, self.language.tr("Mute", "静音"), RED).clicked() {
                        self.dispatch(SemanticCommand::SetMuteTarget {
                            target: MuteTarget::Bus(bus_id),
                            enabled: !muted,
                        });
                    }
                    if matches!(bus_id, MixBusId::Monitor1 | MixBusId::Monitor2) {
                        let linked = self
                            .store
                            .state
                            .routing
                            .monitor_link
                            .stereo_linked
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        if state_button(ui, linked, "MON1 ↔ MON2", GREEN).clicked() {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::MonitorStereoLink(!linked),
                            ));
                        }
                    }
                });
                if let Some(balance) = bus.balance.as_ref() {
                    let mut value = balance.effective().copied().unwrap_or(0.0);
                    if parameter_row(
                        ui,
                        self.language.tr("Balance", "平衡"),
                        &mut value,
                        specs::PAN,
                    )
                    .changed()
                    {
                        self.dispatch(SemanticCommand::SetBusBalance { bus: bus_id, value });
                    }
                }
                let mut limiter = bus
                    .limiter
                    .threshold_db
                    .effective()
                    .copied()
                    .unwrap_or(specs::LIMITER.default);
                if parameter_row(
                    ui,
                    self.language.tr("Limiter", "限制器"),
                    &mut limiter,
                    specs::LIMITER,
                )
                .changed()
                {
                    self.dispatch(SemanticCommand::SetLimiter {
                        bus: bus_id,
                        threshold_db: limiter,
                    });
                }
                ui.collapsing(self.language.tr("9-band GEQ", "9 段图示均衡"), |ui| {
                    for (index, band) in bus.eq.bands.iter().enumerate() {
                        let mut gain = band.gain_db.effective().copied().unwrap_or(0.0);
                        if parameter_row(
                            ui,
                            &format!("{} Hz", band.frequency_hz.effective().unwrap_or(&0.0)),
                            &mut gain,
                            specs::EQ_GAIN,
                        )
                        .changed()
                        {
                            self.dispatch(SemanticCommand::SetGeqBand {
                                bus: bus_id,
                                band: index as u8,
                                frequency_hz: band.frequency_hz.effective().copied().unwrap_or(0.0)
                                    as u16,
                                q: band.q.effective().copied().unwrap_or(1.0),
                                gain_db: gain,
                            });
                        }
                    }
                });
                if self.preferences.show_output_delay_indicator {
                    let ticks = bus.delay_ticks.effective().copied().unwrap_or(0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {} ticks · {:.2} ms",
                            self.language.tr("Delay", "延迟"),
                            ticks,
                            ticks as f32 / 48.0
                        ))
                        .size(10.0)
                        .color(SECONDARY),
                    );
                }
            });
        } else if matches!(destination, MixDestination::Fx1 | MixDestination::Fx2) {
            let (fx_id, index) = if destination == MixDestination::Fx1 {
                (FxId::Fx1, 0)
            } else {
                (FxId::Fx2, 1)
            };
            let effect = self.store.state.effects[index].clone();
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.language.tr("FX ENGINE", "效果器"))
                        .size(10.0)
                        .strong()
                        .color(PURPLE),
                );
                ui.horizontal(|ui| {
                    let muted = effect.muted.effective().copied().unwrap_or(false);
                    if state_button(ui, muted, self.language.tr("Mute", "静音"), RED).clicked() {
                        self.dispatch(SemanticCommand::SetMuteTarget {
                            target: MuteTarget::Fx(fx_id),
                            enabled: !muted,
                        });
                    }
                    ui.label(format!(
                        "{}: {}",
                        self.language.tr("Preset", "预设"),
                        effect.preset.effective().copied().unwrap_or(0)
                    ));
                });
                let mut pan = effect.pan.effective().copied().unwrap_or(0.0);
                if parameter_row(ui, self.language.tr("Pan", "声像"), &mut pan, specs::PAN)
                    .changed()
                {
                    self.dispatch(SemanticCommand::SetFxPan {
                        fx: fx_id,
                        value: pan,
                    });
                }
                let mut values = effect
                    .parameters
                    .each_ref()
                    .map(|value| value.effective().copied().unwrap_or(0));
                let mut setup_changed = false;
                for (index, value) in values.iter_mut().enumerate() {
                    let mut display = *value as f32;
                    setup_changed |= parameter_row(
                        ui,
                        &format!("{} {}", self.language.tr("Parameter", "参数"), index + 1),
                        &mut display,
                        specs::FX_RAW,
                    )
                    .changed();
                    *value = display.round() as u8;
                }
                let mut routes = [
                    effect.return_to_main.effective().copied().unwrap_or(false),
                    effect.return_to_mon1.effective().copied().unwrap_or(false),
                    effect.return_to_mon2.effective().copied().unwrap_or(false),
                ];
                ui.horizontal_wrapped(|ui| {
                    for (index, label) in ["MAIN", "MON1", "MON2"].into_iter().enumerate() {
                        if state_button(ui, routes[index], label, PURPLE).clicked() {
                            routes[index] = !routes[index];
                            setup_changed = true;
                        }
                    }
                });
                if setup_changed {
                    self.dispatch(SemanticCommand::SetFxSetup {
                        fx: fx_id,
                        values,
                        return_to_main: routes[0],
                        return_to_mon1: routes[1],
                        return_to_mon2: routes[2],
                    });
                }
                let mut tempo = self
                    .store
                    .state
                    .global_tempo_bpm
                    .effective()
                    .copied()
                    .unwrap_or(120.0);
                if parameter_row(ui, "BPM", &mut tempo, specs::TEMPO_DISPLAY).changed() {
                    self.dispatch(SemanticCommand::SetTempo {
                        bpm: tempo.round() as u16,
                    });
                }
                ui.label(
                    egui::RichText::new(self.language.tr(
                        "Preset-specific parameter names remain UNKNOWN.",
                        "各预设的具体参数名称仍为 UNKNOWN。",
                    ))
                    .size(9.0)
                    .color(SECONDARY),
                );
            });
        }
        ui.add_space(8.0);
        if let Some(id) = self.store.state.selected_input {
            self.input_inspector(ui, id);
        }
    }

    fn input_inspector(&mut self, ui: &mut egui::Ui, id: InputId) {
        let channel = self.store.state.channels[id.index()].clone();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(self.language.tr("INPUT DETAIL", "输入详情"))
                        .size(10.0)
                        .strong()
                        .color(SECONDARY),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(display_name(id, self.language))
                            .strong()
                            .color(TEXT),
                    );
                });
            });
            ui.separator();
            let mut label = channel
                .name
                .effective()
                .cloned()
                .unwrap_or_else(|| display_name(id, self.language).into());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.language.tr("Label", "名称")).color(TEXT));
                if ui.text_edit_singleline(&mut label).changed() {
                    self.dispatch(SemanticCommand::SetLabel {
                        input: id,
                        icon: channel.icon.effective().copied().unwrap_or(0),
                        name: label.clone(),
                    });
                }
            });
            if channel.capabilities.gain {
                let mut gain = *channel.gain_db.effective().unwrap_or(&0.0);
                if parameter_row(
                    ui,
                    self.language.tr("Gain", "增益"),
                    &mut gain,
                    specs::INPUT_GAIN_DISPLAY,
                )
                .changed()
                {
                    self.dispatch(SemanticCommand::SetGain {
                        input: id,
                        db: gain,
                    });
                }
            }
            let mut pan = *channel.pan.effective().unwrap_or(&0.0);
            if parameter_row(
                ui,
                if id.is_stereo() {
                    self.language.tr("Balance", "平衡")
                } else {
                    self.language.tr("Pan", "声像")
                },
                &mut pan,
                specs::PAN,
            )
            .changed()
            {
                self.dispatch(SemanticCommand::SetPan {
                    input: id,
                    value: pan,
                });
            }
            let destination = self.store.state.selected_destination;
            let mut route = channel.route_levels[destination.index()]
                .effective()
                .copied()
                .unwrap_or(specs::ROUTE_LEVEL.default);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} → {}",
                        display_name(id, self.language),
                        destination_name(destination)
                    ))
                    .color(TEXT),
                );
                let response = ui.add(
                    egui::Slider::new(&mut route, specs::ROUTE_LEVEL.min..=specs::ROUTE_LEVEL.max)
                        .show_value(false),
                );
                ui.label(
                    egui::RichText::new(format!("{:+.1} dB", normalized_to_display_db(route)))
                        .monospace()
                        .color(TEXT),
                );
                if response.changed() {
                    self.dispatch(SemanticCommand::SetRouteLevel {
                        source: id,
                        destination,
                        normalized: route,
                    });
                }
            });
            ui.horizontal_wrapped(|ui| {
                if channel.capabilities.phantom {
                    let enabled = *channel.phantom_48v.effective().unwrap_or(&false);
                    let confirming = self.phantom_confirmation == Some(id);
                    let label = if confirming && !enabled {
                        self.language.tr("CONFIRM 48 V", "确认开启 48 V")
                    } else {
                        "48 V"
                    };
                    if state_button(ui, enabled || confirming, label, RED).clicked() {
                        if enabled {
                            self.phantom_confirmation = None;
                            self.dispatch(SemanticCommand::SetPhantom {
                                input: id,
                                enabled: false,
                            });
                        } else if confirming {
                            self.phantom_confirmation = None;
                            self.dispatch(SemanticCommand::SetPhantom {
                                input: id,
                                enabled: true,
                            });
                        } else {
                            self.phantom_confirmation = Some(id);
                            self.message = self
                                .language
                                .tr(
                                    "48 V can damage incompatible equipment. Click CONFIRM 48 V to enable it.",
                                    "48 V 可能损坏不兼容设备。请再次点击“确认开启 48 V”。",
                                )
                                .into();
                        }
                    }
                }
                if channel.capabilities.phase {
                    let phase = *channel.phase_inverted.effective().unwrap_or(&false);
                    if state_button(ui, phase, self.language.tr("Phase", "极性"), BLUE).clicked()
                    {
                        self.dispatch(SemanticCommand::SetPhase {
                            input: id,
                            inverted: !phase,
                        });
                    }
                }
                if channel.capabilities.high_pass {
                    let hpf = *channel.high_pass_enabled.effective().unwrap_or(&false);
                    if state_button(ui, hpf, self.language.tr("Low Cut", "低切"), YELLOW).clicked()
                    {
                        self.dispatch(SemanticCommand::SetHighPass {
                            input: id,
                            enabled: !hpf,
                            frequency_hz: channel.high_pass_hz.effective().copied().unwrap_or(80.0)
                                as u16,
                        });
                    }
                }
            });
            if channel.capabilities.high_pass {
                let mut frequency = channel
                    .high_pass_hz
                    .effective()
                    .copied()
                    .unwrap_or(specs::HIGH_PASS.default);
                if parameter_row(
                    ui,
                    self.language.tr("Low Cut Frequency", "低切频率"),
                    &mut frequency,
                    specs::HIGH_PASS,
                )
                .changed()
                {
                    self.dispatch(SemanticCommand::SetHighPass {
                        input: id,
                        enabled: channel
                            .high_pass_enabled
                            .effective()
                            .copied()
                            .unwrap_or(false),
                        frequency_hz: frequency.round() as u16,
                    });
                }
            }
            ui.separator();
            ui.label(
                egui::RichText::new(self.language.tr("4-BAND PEQ", "四段参数均衡"))
                    .size(10.0)
                    .strong()
                    .color(SECONDARY),
            );
            if let Some((band, gain)) = eq_gain_graph(ui, &channel) {
                let state = &channel.eq.bands[band];
                self.dispatch(SemanticCommand::SetPeqBand {
                    input: id,
                    band: band as u8,
                    frequency_hz: state.frequency_hz.effective().copied().unwrap_or(80.0) as u16,
                    q: state.q.effective().copied().unwrap_or(1.0),
                    gain_db: gain,
                });
            }
            for (index, band) in channel.eq.bands.iter().enumerate() {
                ui.collapsing(
                    egui::RichText::new(format!(
                        "{} {}  ·  {:.0} Hz  ·  {:+.1} dB",
                        self.language.tr("Band", "频段"),
                        index + 1,
                        band.frequency_hz.effective().unwrap_or(&0.0),
                        band.gain_db.effective().unwrap_or(&0.0)
                    ))
                    .color(TEXT),
                    |ui| {
                        let mut frequency = band.frequency_hz.effective().copied().unwrap_or(80.0);
                        let mut q = band.q.effective().copied().unwrap_or(1.0);
                        let mut gain = band.gain_db.effective().copied().unwrap_or(0.0);
                        let changed = parameter_row(
                            ui,
                            self.language.tr("Frequency", "频率"),
                            &mut frequency,
                            specs::EQ_FREQUENCY_WIRE,
                        )
                        .changed()
                            | parameter_row(ui, "Q", &mut q, specs::EQ_Q_WIRE).changed()
                            | parameter_row(
                                ui,
                                self.language.tr("Gain", "增益"),
                                &mut gain,
                                specs::EQ_GAIN,
                            )
                            .changed();
                        if changed {
                            self.dispatch(SemanticCommand::SetPeqBand {
                                input: id,
                                band: index as u8,
                                frequency_hz: frequency.round() as u16,
                                q,
                                gain_db: gain,
                            });
                        }
                    },
                );
            }
            ui.separator();
            if channel.capabilities.compressor {
                let mut compressor = channel
                    .compressor
                    .amount
                    .effective()
                    .copied()
                    .unwrap_or(0.0);
                if parameter_row(
                    ui,
                    self.language.tr("Compressor", "压缩器"),
                    &mut compressor,
                    specs::COMPRESSOR_AMOUNT,
                )
                .changed()
                {
                    self.dispatch(SemanticCommand::SetCompressorAmount {
                        input: id,
                        amount: compressor,
                    });
                }
            }
            ui.horizontal(|ui| {
                let muted = channel.muted.effective().copied().unwrap_or(false);
                if state_button(ui, muted, self.language.tr("Mute", "静音"), RED).clicked() {
                    self.dispatch(SemanticCommand::SetMute {
                        input: id,
                        enabled: !muted,
                    });
                }
                let soloed = channel.soloed.effective().copied().unwrap_or(false);
                if state_button(ui, soloed, self.language.tr("Solo", "独奏"), YELLOW).clicked() {
                    self.dispatch(SemanticCommand::SetSolo {
                        input: id,
                        enabled: !soloed,
                    });
                }
            });
            ui.label(
                egui::RichText::new(self.language.tr(
                    "Controls operate the shared Store; BLE bytes are never built in this widget.",
                    "控件只操作共享 Store；本组件不构造 BLE 字节。",
                ))
                .size(9.0)
                .color(SECONDARY),
            );
        });
    }

    fn stage_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.heading(egui::RichText::new(self.language.tr("Stage View", "舞台视图")).color(TEXT));
            ui.separator();
            if state_button(
                ui,
                !self.stage_layout_mode,
                self.language.tr("Control", "控制"),
                YELLOW,
            )
            .clicked()
            {
                self.stage_layout_mode = false;
            }
            if state_button(
                ui,
                self.stage_layout_mode,
                self.language.tr("Layout", "布局"),
                BLUE,
            )
            .clicked()
            {
                self.stage_layout_mode = true;
            }
            for destination in MixDestination::ALL {
                if destination_button(
                    ui,
                    self.store.state.selected_destination == destination,
                    destination_name(destination),
                )
                .clicked()
                {
                    self.store.state.selected_destination = destination;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(format!(
                    "{:.0} BPM",
                    self.store
                        .state
                        .global_tempo_bpm
                        .effective()
                        .unwrap_or(&120.0)
                ));
            });
        });
        ui.add_space(8.0);
        let (canvas, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), ui.available_height().max(460.0)),
            Sense::hover(),
        );
        ui.painter()
            .rect_filled(canvas, 8.0, Color32::from_rgb(14, 16, 19));
        for id in InputId::ALL {
            let index = id.index();
            let size = Vec2::new(170.0, 178.0);
            let max_x = (canvas.width() - size.x - 8.0).max(0.0);
            let max_y = (canvas.height() - size.y - 8.0).max(0.0);
            self.stage_positions[index].x = self.stage_positions[index].x.clamp(8.0, max_x);
            self.stage_positions[index].y = self.stage_positions[index].y.clamp(8.0, max_y);
            let rect = Rect::from_min_size(canvas.min + self.stage_positions[index], size);
            let response = ui.interact(
                rect,
                ui.id().with(("stage-node", index)),
                Sense::click_and_drag(),
            );
            if response.dragged() {
                let delta = ui.input(|input| input.pointer.delta());
                if self.stage_layout_mode {
                    self.stage_positions[index] += delta;
                } else {
                    let destination = self.store.state.selected_destination;
                    let current = self.store.state.channels[index].route_levels
                        [destination.index()]
                    .effective()
                    .copied()
                    .unwrap_or(0.0);
                    let next = (current - delta.y / 180.0).clamp(0.0, 1.0);
                    self.dispatch(SemanticCommand::SetRouteLevel {
                        source: id,
                        destination,
                        normalized: next,
                    });
                }
            }
            if response.clicked() {
                self.store.state.selected_input = Some(id);
            }
            let channel = self.store.state.channels[index].clone();
            let route = channel.route_levels[self.store.state.selected_destination.index()]
                .effective()
                .copied()
                .unwrap_or(0.0);
            let meter = channel.meter.level_db.effective().copied().unwrap_or(-60.0);
            let selected = self.store.state.selected_input == Some(id);
            ui.painter().rect_filled(
                rect,
                8.0,
                if selected {
                    Color32::from_rgb(38, 36, 27)
                } else {
                    SURFACE_ALT
                },
            );
            ui.painter().rect_stroke(
                rect,
                8.0,
                Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected { YELLOW } else { BORDER },
                ),
                StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center_top() + Vec2::new(0.0, 18.0),
                egui::Align2::CENTER_CENTER,
                display_name(id, self.language),
                FontId::proportional(15.0),
                TEXT,
            );
            let center = rect.center() + Vec2::new(0.0, -2.0);
            ui.painter().circle_stroke(
                center,
                39.0,
                Stroke::new(7.0, Color32::from_rgb(50, 55, 63)),
            );
            let filled = (route * 24.0).round() as usize;
            for segment in 0..filled {
                let angle = std::f32::consts::PI * (0.75 + segment as f32 / 18.0);
                let a = center + Vec2::angled(angle) * 35.0;
                let b = center + Vec2::angled(angle) * 42.0;
                ui.painter().line_segment([a, b], Stroke::new(3.0, YELLOW));
            }
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                format!("{:+.1}", normalized_to_display_db(route)),
                FontId::proportional(13.0),
                TEXT,
            );
            let meter_width = ((meter + 60.0) / 70.0).clamp(0.0, 1.0) * (rect.width() - 24.0);
            let meter_rect = Rect::from_min_size(
                rect.left_bottom() + Vec2::new(12.0, -41.0),
                Vec2::new(meter_width, 5.0),
            );
            ui.painter()
                .rect_filled(meter_rect, 2.0, if meter >= 0.0 { RED } else { GREEN });
            let mute_rect = Rect::from_min_size(
                rect.left_bottom() + Vec2::new(12.0, -30.0),
                Vec2::new(66.0, 23.0),
            );
            let solo_rect = Rect::from_min_size(
                rect.right_bottom() + Vec2::new(-78.0, -30.0),
                Vec2::new(66.0, 23.0),
            );
            let muted = channel.muted.effective().copied().unwrap_or(false);
            if ui
                .put(
                    mute_rect,
                    egui::Button::new(self.language.tr("MUTE", "静音")).fill(if muted {
                        RED
                    } else {
                        SURFACE
                    }),
                )
                .clicked()
            {
                self.dispatch(SemanticCommand::SetMute {
                    input: id,
                    enabled: !muted,
                });
            }
            let soloed = channel.soloed.effective().copied().unwrap_or(false);
            if ui
                .put(
                    solo_rect,
                    egui::Button::new(self.language.tr("SOLO", "独奏")).fill(if soloed {
                        YELLOW
                    } else {
                        SURFACE
                    }),
                )
                .clicked()
            {
                self.dispatch(SemanticCommand::SetSolo {
                    input: id,
                    enabled: !soloed,
                });
            }
        }
    }

    fn snapshots_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.heading(egui::RichText::new(self.language.tr("Device Snapshots", "设备快照")).color(TEXT));
                ui.label(egui::RichText::new(self.language.tr("Device slots are separate from future local PC snapshot files.", "设备槽位与未来的本地 PC 快照文件分离。")).color(SECONDARY));
                ui.add_space(10.0);
                let snapshots = self.store.state.snapshots.device_slots.clone();
                egui::Grid::new("device-snapshots").num_columns(5).spacing([9.0, 9.0]).show(ui, |ui| {
                    for (index, snapshot) in snapshots.into_iter().enumerate() {
                        card(ui, |ui| {
                            ui.set_min_width(150.0);
                            ui.label(egui::RichText::new(format!("{:02}", snapshot.slot + 1)).strong().color(YELLOW));
                            ui.label(snapshot.name.effective().cloned().unwrap_or_default());
                            ui.horizontal(|ui| {
                                if ui.small_button(self.language.tr("Load", "载入")).clicked() {
                                    self.dispatch(SemanticCommand::LoadSnapshot { slot: snapshot.slot });
                                }
                                if ui.small_button(self.language.tr("Save", "保存")).clicked() {
                                    self.dispatch(SemanticCommand::SaveSnapshot {
                                        slot: snapshot.slot,
                                        name: snapshot.name.effective().cloned().unwrap_or_else(|| format!("Snapshot {:02}", snapshot.slot + 1)),
                                    });
                                }
                                let confirming = self.snapshot_delete_confirmation == Some(snapshot.slot);
                                if ui.small_button(if confirming { self.language.tr("Confirm", "确认") } else { self.language.tr("Delete", "删除") }).clicked() {
                                    if confirming {
                                        self.dispatch(SemanticCommand::DeleteSnapshot { slot: snapshot.slot });
                                        self.snapshot_delete_confirmation = None;
                                    } else {
                                        self.snapshot_delete_confirmation = Some(snapshot.slot);
                                    }
                                }
                            });
                        });
                        if index % 5 == 4 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(10.0);
                ui.separator();
                ui.heading(
                    egui::RichText::new(self.language.tr("Local PC Snapshots", "本地 PC 快照"))
                        .color(TEXT),
                );
                ui.label(
                    egui::RichText::new(self.language.tr(
                        "Local snapshots are application data, never device snapshot slots or protocol evidence.",
                        "本地快照属于应用数据，不是设备快照槽位，也不是协议证据。",
                    ))
                    .color(SECONDARY),
                );
                if ui
                    .add_enabled(
                        self.mode == RunMode::Simulator,
                        egui::Button::new(self.language.tr(
                            "Save current simulator state locally",
                            "保存当前模拟器状态到本地",
                        )),
                    )
                    .clicked()
                {
                    self.local_snapshots.push(LocalSnapshot {
                        name: format!("Local {:02}", self.local_snapshots.len() + 1),
                        state: self.store.state.clone(),
                    });
                }
                let local_snapshots = self.local_snapshots.clone();
                for (index, snapshot) in local_snapshots.into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&snapshot.name).strong().color(TEXT));
                        if ui
                            .add_enabled(
                                self.mode == RunMode::Simulator,
                                egui::Button::new(self.language.tr("Load", "载入")),
                            )
                            .clicked()
                        {
                            self.store.state = snapshot.state;
                            self.store.state.session = SessionState::Ready;
                            self.message = self
                                .language
                                .tr(
                                    "Loaded local simulator snapshot (SYNTHETIC).",
                                    "已载入本地模拟器快照（SYNTHETIC）。",
                                )
                                .into();
                        }
                        if ui.small_button(self.language.tr("Delete", "删除")).clicked() {
                            self.local_snapshots.remove(index);
                        }
                    });
                }
                if self.mode == RunMode::Ble {
                    ui.label(
                        egui::RichText::new(self.language.tr(
                            "Local restore is disabled in BLE mode so an old PC state cannot overwrite the device.",
                            "BLE 模式下禁用本地恢复，避免旧 PC 状态覆盖设备。",
                        ))
                        .size(10.0)
                        .color(SECONDARY),
                    );
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new(self.language.tr("Factory Reset is intentionally not part of automatic or one-click UI flows.", "恢复出厂设置不会进入自动流程或单击操作。" )).size(10.0).color(SECONDARY));
            });
        });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(18.0);
                let content_width = (ui.available_width() - 36.0).clamp(0.0, 1180.0);
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(content_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                        ui.heading(
                            egui::RichText::new(
                                self.language.tr("Preferences", "偏好设置"),
                            )
                            .color(TEXT),
                        );
                        ui.label(
                            egui::RichText::new(self.language.tr(
                                "Application preferences and evidence-backed device settings",
                                "应用偏好与有证据支持的设备设置",
                            ))
                            .color(SECONDARY),
                        );
                        ui.add_space(10.0);
                        let settings = self.store.state.routing.settings.clone();

                        ui.columns(2, |columns| {
                            card(&mut columns[0], |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("APPLICATION", "应用"),
                                    )
                                    .strong()
                                    .color(BLUE),
                                );
                                ui.horizontal(|ui| {
                                    ui.label(self.language.tr("Language", "语言"));
                                    if ui
                                        .selectable_label(
                                            self.language == Language::English,
                                            "English",
                                        )
                                        .clicked()
                                    {
                                        self.language = Language::English;
                                        self.message = localized_status_message(
                                            self.mode,
                                            self.store.state.session,
                                            self.language,
                                        );
                                        ui.ctx().request_repaint();
                                    }
                                    if ui
                                        .selectable_label(
                                            self.language == Language::Chinese,
                                            "简体中文",
                                        )
                                        .clicked()
                                    {
                                        self.language = Language::Chinese;
                                        self.message = localized_status_message(
                                            self.mode,
                                            self.store.state.session,
                                            self.language,
                                        );
                                        ui.ctx().request_repaint();
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label(self.language.tr("UI Scale", "界面缩放"));
                                    let response = ui.add(
                                        egui::Slider::new(
                                            &mut self.preferences.ui_scale,
                                            UI_SCALE_MIN..=UI_SCALE_MAX,
                                        )
                                        .step_by(0.05)
                                        .custom_formatter(|value, _| {
                                            format!("{:.0}%", value * 100.0)
                                        })
                                        .show_value(true),
                                    );
                                    if response.changed() {
                                        ui.ctx().request_repaint();
                                    }
                                });
                                ui.label(format!(
                                    "{}: {}",
                                    self.language.tr("CJK font fallback", "中文字体回退"),
                                    self.font_fallback
                                        .family
                                        .unwrap_or_else(|| self.language.tr(
                                            "not found",
                                            "未找到"
                                        ))
                                ));
                                if let Some(path) = &self.font_fallback.path {
                                    ui.label(
                                        egui::RichText::new(path.display().to_string())
                                            .size(9.0)
                                            .color(SECONDARY),
                                    );
                                }
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("CONNECTION", "连接"),
                                    )
                                    .strong()
                                    .color(BLUE),
                                );
                                ui.label(format!(
                                    "{}: {}",
                                    self.language.tr("Mode", "模式"),
                                    match self.mode {
                                        RunMode::Simulator => {
                                            self.language.tr("Simulator", "模拟器")
                                        }
                                        RunMode::Ble => {
                                            self.language.tr("Direct Bluetooth", "直连蓝牙")
                                        },
                                    }
                                ));
                                ui.label(format!(
                                    "{}: {}",
                                    self.language.tr("Status", "状态"),
                                    session_state_text(
                                        self.store.state.session,
                                        self.language,
                                    )
                                ));
                                if let Some(stage) = self.native_stage {
                                    ui.label(format!(
                                        "{}: {}",
                                        self.language.tr("Native stage", "原生连接阶段"),
                                        native_stage_text(stage, self.language)
                                    ));
                                }
                                if let Some(error) = &self.last_ble_error {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}: {}",
                                            self.language.tr(
                                                "Last BLE error",
                                                "最近 BLE 错误"
                                            ),
                                            error
                                        ))
                                        .color(RED),
                                    );
                                }
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(self.language.tr(
                                        "Automatic reconnect · Not implemented",
                                        "自动重连 · 尚未实现",
                                    )),
                                );
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(self.language.tr(
                                        "MIDI transport · Not implemented",
                                        "MIDI 传输 · 尚未实现",
                                    )),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}: {}",
                                        self.language.tr("Logging", "日志"),
                                        std::env::var("RUST_LOG")
                                            .unwrap_or_else(|_| "flow8_ble=info".into())
                                    ))
                                    .size(10.0)
                                    .color(SECONDARY),
                                );
                            });

                            card(&mut columns[1], |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("MIXER DISPLAY", "混音器显示"),
                                    )
                                    .strong()
                                    .color(YELLOW),
                                );
                                ui.checkbox(
                                    &mut self.preferences.show_mute_buttons,
                                    self.language.tr(
                                        "Show Mute Buttons",
                                        "显示静音按钮",
                                    ),
                                );
                                ui.checkbox(
                                    &mut self.preferences.show_channel_icons,
                                    self.language.tr(
                                        "Show Channel Icons",
                                        "显示通道图标",
                                    ),
                                );
                                ui.checkbox(
                                    &mut self.preferences.show_output_delay_indicator,
                                    self.language.tr(
                                        "Show Output Delay Indicator",
                                        "显示输出延迟指示",
                                    ),
                                );
                                ui.separator();
                                ui.label(self.language.tr(
                                    "Visible mixer inputs",
                                    "可见混音输入",
                                ));
                                for input in InputId::ALL {
                                    ui.checkbox(
                                        &mut self.preferences.channel_visible[input.index()],
                                        display_name(input, self.language),
                                    );
                                }
                            });
                        });

                        ui.add_space(8.0);
                        ui.columns(2, |columns| {
                            card(&mut columns[0], |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("DEVICE / CONTROL", "设备 / 控制"),
                                    )
                                    .strong()
                                    .color(BLUE),
                                );
                                let mut name = settings
                                    .device_name
                                    .effective()
                                    .cloned()
                                    .unwrap_or_else(|| "FLOW 8".into());
                                ui.horizontal(|ui| {
                                    ui.label(self.language.tr(
                                        "Device Name",
                                        "设备名称",
                                    ));
                                    if ui.text_edit_singleline(&mut name).changed() {
                                        self.dispatch(SemanticCommand::SetSetting(
                                            KnownSetting::DeviceName(name.clone()),
                                        ));
                                    }
                                });
                                let footswitch = settings
                                    .footswitch_fx_mode
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                ui.horizontal(|ui| {
                                    ui.label(self.language.tr(
                                        "Footswitch Mode",
                                        "脚踏开关模式",
                                    ));
                                    for (fx_mode, label) in [
                                        (true, "FX"),
                                        (
                                            false,
                                            self.language.tr("Snapshot", "快照"),
                                        ),
                                    ] {
                                        if state_button(
                                            ui,
                                            footswitch == fx_mode,
                                            label,
                                            BLUE,
                                        )
                                        .clicked()
                                        {
                                            self.dispatch(
                                                SemanticCommand::SetSetting(
                                                    KnownSetting::FootswitchFxMode(
                                                        fx_mode,
                                                    ),
                                                ),
                                            );
                                        }
                                    }
                                });
                                let linked = settings
                                    .device_linked_selection
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                if state_button(
                                    ui,
                                    linked,
                                    self.language.tr(
                                        "Device-linked Output Selection",
                                        "设备联动输出选择",
                                    ),
                                    BLUE,
                                )
                                .clicked()
                                {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::DeviceLinkedSelection(!linked),
                                    ));
                                }
                                ui.add_enabled_ui(false, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(self.language.tr(
                                            "Control Gesture",
                                            "控制手势",
                                        ));
                                        for (mode, label) in [
                                            (
                                                ControlGesture::Linear,
                                                self.language.tr("Linear", "线性"),
                                            ),
                                            (
                                                ControlGesture::Rotary,
                                                self.language.tr("Rotary", "旋转"),
                                            ),
                                        ] {
                                            let _ = ui.selectable_label(
                                                self.preferences.control_gesture == mode,
                                                label,
                                            );
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(self.language.tr(
                                            "EQ Editing Mode",
                                            "EQ 编辑模式",
                                        ));
                                        for (mode, label) in [
                                            (
                                                EqEditingMode::Standard,
                                                self.language.tr("Standard", "标准"),
                                            ),
                                            (
                                                EqEditingMode::Parametric,
                                                self.language.tr("Parametric", "参数"),
                                            ),
                                        ] {
                                            let _ = ui.selectable_label(
                                                self.preferences.eq_editing_mode == mode,
                                                label,
                                            );
                                        }
                                    });
                                });
                                ui.label(
                                    egui::RichText::new(self.language.tr(
                                        "Gesture and alternate EQ modes remain visible for Qt parity, but are disabled until the egui interaction modes are implemented.",
                                        "为保持 Qt 界面一致性，控制手势和另一种 EQ 模式仍会显示；在 egui 交互模式实现前保持禁用。",
                                    ))
                                    .size(9.0)
                                    .color(SECONDARY),
                                );
                            });

                            card(&mut columns[1], |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("MONITOR ROUTING", "监听路由"),
                                    )
                                    .strong()
                                    .color(GREEN),
                                );
                                let current = settings
                                    .monitor_routing
                                    .effective()
                                    .copied()
                                    .unwrap_or(MonitorRoutingSource::MonitorMix);
                                for (candidate, label, raw) in [
                                    (MonitorRoutingSource::MonitorMix, "MON", 0),
                                    (MonitorRoutingSource::Usb12, "USB 1/2", 1),
                                    (MonitorRoutingSource::Usb34, "USB 3/4", 2),
                                ] {
                                    if state_button(
                                        ui,
                                        current == candidate,
                                        label,
                                        GREEN,
                                    )
                                    .clicked()
                                    {
                                        self.dispatch(SemanticCommand::SetSetting(
                                            KnownSetting::MonitorRouting(raw),
                                        ));
                                    }
                                }
                                let post = settings
                                    .monitor_post_fader
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                if state_button(
                                    ui,
                                    post,
                                    if post { "POST" } else { "PRE" },
                                    GREEN,
                                )
                                .clicked()
                                {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::MonitorPostFader(!post),
                                    ));
                                }
                                let linked = self
                                    .store
                                    .state
                                    .routing
                                    .monitor_link
                                    .stereo_linked
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                if state_button(
                                    ui,
                                    linked,
                                    self.language.tr(
                                        "MON1/2 Stereo Link",
                                        "MON1/2 立体声链接",
                                    ),
                                    GREEN,
                                )
                                .clicked()
                                {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::MonitorStereoLink(!linked),
                                    ));
                                }
                                ui.label(
                                    egui::RichText::new(self.language.tr(
                                        "Link propagation remains UNKNOWN until hardware validation.",
                                        "链接后的参数传播仍需真机验证。",
                                    ))
                                    .size(9.0)
                                    .color(SECONDARY),
                                );
                            });
                        });

                        ui.add_space(8.0);
                        ui.columns(2, |columns| {
                            card(&mut columns[0], |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("USB / HEADPHONES", "USB / 耳机"),
                                    )
                                    .strong()
                                    .color(GREEN),
                                );
                                for (active, label, command) in [
                                    (
                                        settings
                                            .usb_streaming
                                            .effective()
                                            .copied()
                                            .unwrap_or(false),
                                        self.language.tr(
                                            "USB Streaming",
                                            "USB 推流",
                                        ),
                                        0_u8,
                                    ),
                                    (
                                        settings
                                            .input56_from_usb12
                                            .effective()
                                            .copied()
                                            .unwrap_or(false),
                                        self.language.tr(
                                            "USB 1/2 → Input 5/6",
                                            "USB 1/2 → 输入 5/6",
                                        ),
                                        1_u8,
                                    ),
                                    (
                                        settings
                                            .input78_from_usb34
                                            .effective()
                                            .copied()
                                            .unwrap_or(false),
                                        self.language.tr(
                                            "USB 3/4 → Input 7/8",
                                            "USB 3/4 → 输入 7/8",
                                        ),
                                        2_u8,
                                    ),
                                    (
                                        settings
                                            .phones_only
                                            .effective()
                                            .copied()
                                            .unwrap_or(false),
                                        self.language.tr(
                                            "Bluetooth / USB to Headphones Only",
                                            "蓝牙 / USB 仅送耳机",
                                        ),
                                        3_u8,
                                    ),
                                ] {
                                    if state_button(ui, active, label, GREEN).clicked() {
                                        let setting = match command {
                                            0 => KnownSetting::UsbStreaming(!active),
                                            1 => {
                                                KnownSetting::Input56FromUsb12(!active)
                                            }
                                            2 => {
                                                KnownSetting::Input78FromUsb34(!active)
                                            }
                                            _ => KnownSetting::PhonesOnly(!active),
                                        };
                                        self.dispatch(SemanticCommand::SetSetting(setting));
                                    }
                                }
                                let bt_usb_switch = settings
                                    .bt_usb_switch
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                if state_button(
                                    ui,
                                    bt_usb_switch,
                                    self.language.tr(
                                        "Bluetooth / USB source switch",
                                        "蓝牙 / USB 信号源切换",
                                    ),
                                    GREEN,
                                )
                                .clicked()
                                {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::BtUsbSwitch(!bt_usb_switch),
                                    ));
                                }
                                let headphone_source = self
                                    .store
                                    .state
                                    .routing
                                    .headphones
                                    .source
                                    .effective()
                                    .copied()
                                    .unwrap_or(HeadphoneSource::Main);
                                ui.horizontal(|ui| {
                                    ui.label(self.language.tr(
                                        "Headphone Source",
                                        "耳机信号源",
                                    ));
                                    for (source, label) in [
                                        (HeadphoneSource::Main, "MAIN"),
                                        (HeadphoneSource::Monitor, "MON"),
                                    ] {
                                        if state_button(
                                            ui,
                                            headphone_source == source,
                                            label,
                                            GREEN,
                                        )
                                        .clicked()
                                        {
                                            self.dispatch(SemanticCommand::SetSetting(
                                                KnownSetting::HeadphonesUseMonitor(
                                                    source == HeadphoneSource::Monitor,
                                                ),
                                            ));
                                        }
                                    }
                                });
                                let headphone_tap = self
                                    .store
                                    .state
                                    .routing
                                    .headphones
                                    .tap_point
                                    .effective()
                                    .copied()
                                    .unwrap_or(TapPoint::PostFader);
                                ui.horizontal(|ui| {
                                    ui.label(self.language.tr(
                                        "Headphone Tap",
                                        "耳机取样点",
                                    ));
                                    for (tap, label) in [
                                        (TapPoint::PreFader, "PRE"),
                                        (TapPoint::PostFader, "POST"),
                                    ] {
                                        if state_button(
                                            ui,
                                            headphone_tap == tap,
                                            label,
                                            GREEN,
                                        )
                                        .clicked()
                                        {
                                            self.dispatch(SemanticCommand::SetSetting(
                                                KnownSetting::HeadphonesPostFader(
                                                    tap == TapPoint::PostFader,
                                                ),
                                            ));
                                        }
                                    }
                                });
                            });

                            card(&mut columns[1], |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("OUTPUT / DIAGNOSTICS", "输出 / 诊断"),
                                    )
                                    .strong()
                                    .color(TEXT),
                                );
                                let main_pad = settings
                                    .main_minus_10_dbv
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                if state_button(
                                    ui,
                                    main_pad,
                                    "MAIN -10 dBV",
                                    TEXT,
                                )
                                .clicked()
                                {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::MainMinus10Dbv(!main_pad),
                                    ));
                                }
                                let monitor_pad = settings
                                    .monitor_minus_10_dbv
                                    .effective()
                                    .copied()
                                    .unwrap_or(false);
                                if state_button(
                                    ui,
                                    monitor_pad,
                                    "MON -10 dBV",
                                    TEXT,
                                )
                                .clicked()
                                {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::MonitorMinus10Dbv(!monitor_pad),
                                    ));
                                }
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(self.language.tr(
                                        "Request / Save MIDI SysEx Dump · Unavailable",
                                        "请求 / 保存 MIDI SysEx 转储 · 不可用",
                                    )),
                                );
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(self.language.tr(
                                        "Unknown device settings · Unavailable",
                                        "未识别设备设置 · 不可用",
                                    )),
                                );
                                ui.label(
                                    egui::RichText::new(self.language.tr(
                                        "Setting IDs 0x04 and 0x06 remain UNKNOWN; no protocol write is attached.",
                                        "Setting ID 0x04 与 0x06 仍为 UNKNOWN；此处不会触发协议写入。",
                                    ))
                                    .size(9.0)
                                    .color(SECONDARY),
                                );
                            });
                        });

                        if !self.discovered_devices.is_empty() {
                            ui.add_space(8.0);
                            card(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(self.language.tr(
                                        "Last BLE Scan",
                                        "最近一次 BLE 扫描",
                                    ))
                                    .strong()
                                    .color(BLUE),
                                );
                                for device in &self.discovered_devices {
                                    ui.label(format!(
                                        "{} · {} · RSSI {:?}",
                                        device.name.as_deref().unwrap_or_else(|| {
                                            self.language.tr("Unknown", "未知")
                                        }),
                                        device.address,
                                        device.rssi
                                    ));
                                }
                            });
                        }
                        },
                    );
                });
            });
    }

    fn main_out_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.heading(egui::RichText::new(self.language.tr("MAIN OUT", "主输出")).color(TEXT));
                ui.label(
                    egui::RichText::new(self.language.tr(
                        "Mix buses and physical output sinks remain distinct.",
                        "混音总线与物理输出端保持分离。",
                    ))
                    .color(SECONDARY),
                );
                ui.add_space(12.0);
                card(ui, |ui| {
                    ui.set_min_width(620.0);
                    ui.label(
                        self.language
                            .tr("MAIN Mix Bus  →  MAIN OUT", "MAIN 混音总线  →  主输出"),
                    );
                    ui.label(self.language.tr(
                        "MON1 Mix Bus  →  MONITOR OUT 1 (6.5 mm mono)",
                        "MON1 混音总线  →  监听输出 1（6.5 mm 单声道）",
                    ));
                    ui.label(self.language.tr(
                        "MON2 Mix Bus  →  MONITOR OUT 2 (6.5 mm mono)",
                        "MON2 混音总线  →  监听输出 2（6.5 mm 单声道）",
                    ));
                    let source = self
                        .store
                        .state
                        .routing
                        .headphones
                        .source
                        .effective()
                        .copied()
                        .unwrap_or(HeadphoneSource::Main);
                    let tap = self
                        .store
                        .state
                        .routing
                        .headphones
                        .tap_point
                        .effective()
                        .copied()
                        .unwrap_or(TapPoint::PostFader);
                    ui.label(format!(
                        "{}  ←  {} / {}",
                        self.language.tr("HEADPHONES", "耳机"),
                        headphone_source_text(source, self.language),
                        tap_point_text(tap, self.language)
                    ));
                    ui.label(
                        egui::RichText::new(self.language.tr(
                            "USB 1/2 · USB 3/4 = independent USB Audio endpoints",
                            "USB 1/2 · USB 3/4 = 独立 USB 音频端点",
                        ))
                        .color(BLUE),
                    );
                });
            });
        });
    }

    fn routing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.heading(egui::RichText::new(self.language.tr("Routing", "路由")).color(TEXT));
                ui.label(
                    egui::RichText::new(self.language.tr(
                        "Signal sources, mix buses and physical outputs remain separate.",
                        "信号源、混音总线和物理输出保持分层。",
                    ))
                    .color(SECONDARY),
                );
                ui.add_space(12.0);
                ui.columns(2, |columns| {
                    card(&mut columns[0], |ui| {
                        ui.label(
                            egui::RichText::new(self.language.tr("USB AUDIO", "USB 音频"))
                                .strong()
                                .color(BLUE),
                        );
                        let settings = self.store.state.routing.settings.clone();
                        let streaming =
                            settings.usb_streaming.effective().copied().unwrap_or(false);
                        if state_button(
                            ui,
                            streaming,
                            self.language.tr("Recording / Streaming", "录音 / 推流"),
                            BLUE,
                        )
                        .clicked()
                        {
                            self.dispatch(SemanticCommand::SetSetting(KnownSetting::UsbStreaming(
                                !streaming,
                            )));
                        }
                        let input56 = settings
                            .input56_from_usb12
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        if state_button(
                            ui,
                            input56,
                            self.language
                                .tr("USB 1/2 → Input 5/6", "USB 1/2 → 输入 5/6"),
                            BLUE,
                        )
                        .clicked()
                        {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::Input56FromUsb12(!input56),
                            ));
                        }
                        let input78 = settings
                            .input78_from_usb34
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        if state_button(
                            ui,
                            input78,
                            self.language
                                .tr("USB 3/4 → Input 7/8", "USB 3/4 → 输入 7/8"),
                            BLUE,
                        )
                        .clicked()
                        {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::Input78FromUsb34(!input78),
                            ));
                        }
                        ui.label("USB 1/2 · USB 3/4");
                        ui.label(
                            egui::RichText::new(self.language.tr(
                                "Independent loopback endpoints; not MON buses",
                                "独立回环端点；不是 MON 总线",
                            ))
                            .size(10.0)
                            .color(SECONDARY),
                        );
                    });
                    card(&mut columns[1], |ui| {
                        ui.label(
                            egui::RichText::new(
                                self.language.tr("MONITOR / HEADPHONES", "监听 / 耳机"),
                            )
                            .strong()
                            .color(GREEN),
                        );
                        let linked = *self
                            .store
                            .state
                            .routing
                            .monitor_link
                            .stereo_linked
                            .effective()
                            .unwrap_or(&false);
                        if state_button(
                            ui,
                            linked,
                            self.language.tr("MON1/2 Stereo Link", "MON1/2 立体声链接"),
                            GREEN,
                        )
                        .clicked()
                        {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::MonitorStereoLink(!linked),
                            ));
                        }
                        let source = self
                            .store
                            .state
                            .routing
                            .headphones
                            .source
                            .effective()
                            .copied()
                            .unwrap_or(HeadphoneSource::Main);
                        ui.horizontal(|ui| {
                            for (candidate, label) in [
                                (HeadphoneSource::Main, "MAIN"),
                                (HeadphoneSource::Monitor, "MONITOR"),
                            ] {
                                if state_button(ui, source == candidate, label, GREEN).clicked() {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::HeadphonesUseMonitor(
                                            candidate == HeadphoneSource::Monitor,
                                        ),
                                    ));
                                }
                            }
                        });
                        let tap = self
                            .store
                            .state
                            .routing
                            .headphones
                            .tap_point
                            .effective()
                            .copied()
                            .unwrap_or(TapPoint::PostFader);
                        ui.horizontal(|ui| {
                            for (candidate, label) in
                                [(TapPoint::PreFader, "PRE"), (TapPoint::PostFader, "POST")]
                            {
                                if state_button(ui, tap == candidate, label, GREEN).clicked() {
                                    self.dispatch(SemanticCommand::SetSetting(
                                        KnownSetting::HeadphonesPostFader(
                                            candidate == TapPoint::PostFader,
                                        ),
                                    ));
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(self.language.tr(
                                "Link propagation remains UNKNOWN until hardware validation.",
                                "链接后的参数传播仍需真机验证。",
                            ))
                            .size(10.0)
                            .color(SECONDARY),
                        );
                    });
                });
                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    card(&mut columns[0], |ui| {
                        ui.label(
                            egui::RichText::new(self.language.tr("FX RETURNS", "FX 返回路由"))
                                .strong()
                                .color(PURPLE),
                        );
                        for (index, fx_id) in [FxId::Fx1, FxId::Fx2].into_iter().enumerate() {
                            let effect = self.store.state.effects[index].clone();
                            let mut routes = [
                                effect.return_to_main.effective().copied().unwrap_or(false),
                                effect.return_to_mon1.effective().copied().unwrap_or(false),
                                effect.return_to_mon2.effective().copied().unwrap_or(false),
                            ];
                            ui.label(format!("FX {}", index + 1));
                            let mut changed = false;
                            ui.horizontal(|ui| {
                                for (route, label) in
                                    ["MAIN", "MON1", "MON2"].into_iter().enumerate()
                                {
                                    if state_button(ui, routes[route], label, PURPLE).clicked() {
                                        routes[route] = !routes[route];
                                        changed = true;
                                    }
                                }
                            });
                            if changed {
                                self.dispatch(SemanticCommand::SetFxSetup {
                                    fx: fx_id,
                                    values: effect
                                        .parameters
                                        .each_ref()
                                        .map(|value| value.effective().copied().unwrap_or(0)),
                                    return_to_main: routes[0],
                                    return_to_mon1: routes[1],
                                    return_to_mon2: routes[2],
                                });
                            }
                        }
                    });
                    card(&mut columns[1], |ui| {
                        ui.label(
                            egui::RichText::new(self.language.tr("OUTPUTS", "物理输出"))
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(self.language.tr("MAIN → MAIN OUT", "MAIN → 主输出"));
                        ui.label(
                            self.language
                                .tr("MON1 → MONITOR OUT 1", "MON1 → 监听输出 1"),
                        );
                        ui.label(
                            self.language
                                .tr("MON2 → MONITOR OUT 2", "MON2 → 监听输出 2"),
                        );
                        let main_pad = self
                            .store
                            .state
                            .routing
                            .settings
                            .main_minus_10_dbv
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        if state_button(ui, main_pad, "MAIN -10 dBV", TEXT).clicked() {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::MainMinus10Dbv(!main_pad),
                            ));
                        }
                        let mon_pad = self
                            .store
                            .state
                            .routing
                            .settings
                            .monitor_minus_10_dbv
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        if state_button(ui, mon_pad, "MON -10 dBV", TEXT).clicked() {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::MonitorMinus10Dbv(!mon_pad),
                            ));
                        }
                    });
                });
            });
        });
    }
}

impl eframe::App for Flow8App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let metrics = UiMetrics::calculate(
            ui.available_size(),
            ui.ctx().pixels_per_point(),
            self.preferences.ui_scale,
        );
        if metrics != self.metrics {
            self.metrics = metrics;
            configure_style(ui.ctx(), metrics);
        }
        self.poll_runtime();
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if self.mode == RunMode::Simulator {
            self.store.tick_simulator(dt);
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));

        ui.set_min_size(ui.available_size());
        ui.painter().rect_filled(ui.max_rect(), 0.0, BG);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            self.connection_bar(ui);
            self.layer_bar(ui);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            match self.page {
                Page::Mixer
                | Page::Fx1
                | Page::Fx2
                | Page::Monitor1
                | Page::Monitor2
                | Page::Main => self.mixer(ui),
                Page::Stage => self.stage_page(ui),
                Page::MainOut => {
                    egui::ScrollArea::vertical()
                        .id_salt("main-out-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.main_out_page(ui));
                }
                Page::Routing => {
                    egui::ScrollArea::vertical()
                        .id_salt("routing-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.routing_page(ui));
                }
                Page::Snapshots => {
                    egui::ScrollArea::both()
                        .id_salt("snapshots-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.snapshots_page(ui));
                }
                Page::Settings => self.settings_page(ui),
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 23, 27))
                    .inner_margin(egui::Margin::symmetric(18, 7))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&self.message)
                                .size(10.0)
                                .color(SECONDARY),
                        );
                    });
            });
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StripAction {
    Select,
    Route(f32),
    Pan(f32),
    Mute(bool),
    Solo(bool),
}

fn channel_strip(
    ui: &mut egui::Ui,
    channel: &InputChannelState,
    destination: MixDestination,
    selected: bool,
    language: Language,
    show_channel_icons: bool,
    show_mute_buttons: bool,
    metrics: UiMetrics,
    strip_width: f32,
    fader_height: f32,
) -> Option<StripAction> {
    let mut action = None;
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(
            if selected { 1.5 } else { 1.0 },
            if selected { YELLOW } else { BORDER },
        ))
        .corner_radius(8)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_width(strip_width);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(channel_number(channel.id))
                        .size(metrics.small_font)
                        .strong()
                        .color(SECONDARY),
                );
                let icon = if channel.id == InputId::UsbBluetooth {
                    "▣"
                } else if channel.id.is_stereo() {
                    "◫"
                } else {
                    "●"
                };
                if show_channel_icons {
                    ui.label(
                        egui::RichText::new(icon)
                            .size(20.0 * metrics.ui_scale)
                            .color(if selected { YELLOW } else { TEXT }),
                    );
                }
                if ui
                    .selectable_label(
                        selected,
                        egui::RichText::new(display_name(channel.id, language))
                            .size(metrics.body_font)
                            .strong()
                            .color(TEXT),
                    )
                    .clicked()
                {
                    action = Some(StripAction::Select);
                }
                if channel.id.is_stereo() {
                    ui.label(
                        egui::RichText::new(language.tr("STEREO", "立体声"))
                            .size(metrics.small_font)
                            .color(BLUE),
                    );
                }
                if channel.capabilities.phantom
                    && *channel.phantom_48v.effective().unwrap_or(&false)
                {
                    ui.label(
                        egui::RichText::new("48 V")
                            .size(metrics.small_font)
                            .strong()
                            .color(RED),
                    );
                }
                ui.add_space(4.0);
                let mut pan = *channel.pan.effective().unwrap_or(&0.0);
                let pan_response = pan_control(
                    ui,
                    &mut pan,
                    (strip_width - 14.0 * metrics.ui_scale).max(72.0),
                    metrics,
                );
                if pan_response.changed() {
                    action = Some(StripAction::Pan(pan));
                }
                ui.label(
                    egui::RichText::new(pan_text(pan, channel.id.is_stereo()))
                        .size(metrics.small_font)
                        .color(SECONDARY),
                );
                ui.add_space(4.0);
                let mut route = *channel.route_levels[destination.index()]
                    .effective()
                    .unwrap_or(&0.0);
                let meter = *channel.meter.level_db.effective().unwrap_or(&-60.0);
                ui.horizontal(|ui| {
                    let fader_width = (strip_width * 0.49).clamp(54.0, 76.0);
                    let meter_width = (strip_width - fader_width - metrics.spacing).max(38.0);
                    let changed =
                        fader_sized(ui, &mut route, fader_width, fader_height, 0.75).changed();
                    meter_widget_sized(ui, meter, meter_width, fader_height, metrics);
                    if changed {
                        action = Some(StripAction::Route(route));
                    }
                });
                ui.label(
                    egui::RichText::new(format!("{:.1} dB", normalized_to_display_db(route)))
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    if show_mute_buttons {
                        let muted = *channel.muted.effective().unwrap_or(&false);
                        if state_button(ui, muted, language.tr("MUTE", "静音"), RED).clicked() {
                            action = Some(StripAction::Mute(!muted));
                        }
                    }
                    let soloed = *channel.soloed.effective().unwrap_or(&false);
                    if state_button(ui, soloed, language.tr("SOLO", "独奏"), YELLOW).clicked() {
                        action = Some(StripAction::Solo(!soloed));
                    }
                });
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    badge(
                        ui,
                        "EQ",
                        channel.eq.enabled.effective().copied().unwrap_or(false),
                    );
                    badge(
                        ui,
                        "COMP",
                        channel
                            .compressor
                            .amount
                            .effective()
                            .copied()
                            .unwrap_or(0.0)
                            > 0.01,
                    );
                });
            });
        });
    action
}

fn fader_sized(
    ui: &mut egui::Ui,
    value: &mut f32,
    width: f32,
    height: f32,
    default: f32,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let next = ((rect.bottom() - pointer.y) / rect.height()).clamp(0.0, 1.0);
        if (*value - next).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
        }
    }
    if response.double_clicked() {
        *value = default;
        response.mark_changed();
    }
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0 {
            *value = (*value + scroll.signum() * 0.005).clamp(0.0, 1.0);
            response.mark_changed();
        }
    }
    if response.has_focus() {
        let delta = ui.input(|input| {
            if input.key_pressed(egui::Key::ArrowUp) {
                0.01
            } else if input.key_pressed(egui::Key::ArrowDown) {
                -0.01
            } else {
                0.0
            }
        });
        if delta != 0.0 {
            *value = (*value + delta).clamp(0.0, 1.0);
            response.mark_changed();
        }
    }
    let cursor = if response.dragged() {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response = response.on_hover_cursor(cursor);
    let painter = ui.painter();
    let track = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.center().y),
        Vec2::new(5.0, rect.height() - 24.0),
    );
    painter.rect_filled(track, 2.0, Color32::from_rgb(52, 57, 67));
    let fill_top = egui::lerp(track.bottom()..=track.top(), *value);
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(track.left(), fill_top), track.right_bottom()),
        2.0,
        YELLOW,
    );
    for index in 0..=10 {
        let y = egui::lerp(track.bottom()..=track.top(), index as f32 / 10.0);
        painter.line_segment(
            [
                Pos2::new(track.left() - 8.0, y),
                Pos2::new(track.left() - 3.0, y),
            ],
            Stroke::new(1.0, SECONDARY),
        );
    }
    let thumb_y = egui::lerp(track.bottom()..=track.top(), *value);
    let thumb = Rect::from_center_size(Pos2::new(track.center().x, thumb_y), Vec2::new(29.0, 15.0));
    painter.rect_filled(
        thumb,
        4.0,
        if response.dragged() {
            YELLOW
        } else if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(215, 220, 227)
        },
    );
    painter.rect_stroke(
        thumb,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(90, 96, 105)),
        StrokeKind::Inside,
    );
    response
}

fn meter_widget_sized(ui: &mut egui::Ui, db: f32, width: f32, height: f32, metrics: UiMetrics) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + 3.0, rect.top() + 2.0),
        Pos2::new(rect.left() + 14.0, rect.bottom() - 2.0),
    );
    ui.painter()
        .rect_filled(bar, 2.0, Color32::from_rgb(12, 14, 17));
    let normalized = ((db - specs::METER_DISPLAY.min)
        / (specs::METER_DISPLAY.max - specs::METER_DISPLAY.min))
        .clamp(0.0, 1.0);
    let top = egui::lerp(bar.bottom()..=bar.top(), normalized);
    let color = if db >= 9.5 {
        RED
    } else if db >= -6.0 {
        YELLOW
    } else {
        GREEN
    };
    ui.painter().rect_filled(
        Rect::from_min_max(Pos2::new(bar.left(), top), bar.right_bottom()),
        2.0,
        color,
    );
    for (value, label) in [
        (10.0_f32, "+10"),
        (0.0, "0"),
        (-20.0, "-20"),
        (-40.0, "-40"),
        (-60.0, "-60"),
    ] {
        let y = egui::lerp(
            bar.bottom()..=bar.top(),
            ((value + 60.0) / 70.0).clamp(0.0, 1.0),
        );
        ui.painter().line_segment(
            [
                Pos2::new(bar.right() + 2.0, y),
                Pos2::new(bar.right() + 5.0, y),
            ],
            Stroke::new(1.0, SECONDARY),
        );
        ui.painter().text(
            Pos2::new(bar.right() + 7.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(metrics.small_font),
            SECONDARY,
        );
    }
}

fn pan_control(ui: &mut egui::Ui, value: &mut f32, width: f32, metrics: UiMetrics) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(
        Vec2::new(width, 20.0 * metrics.ui_scale),
        Sense::click_and_drag(),
    );
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let next = ((pointer.x - rect.left()) / rect.width() * 2.0 - 1.0).clamp(-1.0, 1.0);
        if (*value - next).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
        }
    }
    if response.double_clicked() {
        *value = 0.0;
        response.mark_changed();
    }
    let track_y = rect.center().y;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), track_y),
            Pos2::new(rect.right(), track_y),
        ],
        Stroke::new(3.0, Color32::from_rgb(52, 57, 67)),
    );
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, SECONDARY),
    );
    let x = egui::lerp(rect.left()..=rect.right(), (*value + 1.0) / 2.0);
    ui.painter().circle_filled(
        Pos2::new(x, track_y),
        6.0,
        if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(215, 220, 227)
        },
    );
    let cursor = if response.dragged() {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response.on_hover_cursor(cursor)
}

fn parameter_row(ui: &mut egui::Ui, label: &str, value: &mut f32, spec: ParameterSpec) -> Response {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let suffix = match spec.unit {
                flow8_model::Unit::Decibels => " dB",
                flow8_model::Unit::Hertz => " Hz",
                _ => "",
            };
            ui.add(
                egui::Slider::new(value, spec.min..=spec.max)
                    .step_by(spec.step.unwrap_or(0.01) as f64)
                    .suffix(suffix)
                    .show_value(true),
            )
        })
        .inner
    })
    .inner
}

fn eq_gain_graph(ui: &mut egui::Ui, channel: &InputChannelState) -> Option<(usize, f32)> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 96.0),
        Sense::click_and_drag(),
    );
    ui.painter()
        .rect_filled(rect, 4.0, Color32::from_rgb(13, 15, 18));
    for index in 1..4 {
        let x = egui::lerp(rect.left()..=rect.right(), index as f32 / 4.0);
        ui.painter()
            .vline(x, rect.y_range(), Stroke::new(1.0, BORDER));
    }
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, BORDER));
    let mut points = Vec::new();
    for (index, band) in channel.eq.bands.iter().enumerate() {
        let x = egui::lerp(rect.left()..=rect.right(), (index as f32 + 0.5) / 4.0);
        let gain = *band.gain_db.effective().unwrap_or(&0.0);
        let y = egui::lerp(rect.bottom()..=rect.top(), (gain + 15.0) / 30.0);
        points.push(Pos2::new(x, y));
        ui.painter().circle_filled(Pos2::new(x, y), 4.0, YELLOW);
    }
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.0, YELLOW)));
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let normalized_x = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 0.999);
        let band = (normalized_x * 4.0).floor() as usize;
        let normalized_y = ((rect.bottom() - pointer.y) / rect.height()).clamp(0.0, 1.0);
        let gain = specs::EQ_GAIN.min + normalized_y * (specs::EQ_GAIN.max - specs::EQ_GAIN.min);
        return Some((band.min(3), gain));
    }
    None
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, add)
        .inner
}

fn small_tab(ui: &mut egui::Ui, selected: bool, label: &str, accent: Color32) -> Response {
    let fill = if selected {
        accent.gamma_multiply(0.17)
    } else {
        Color32::TRANSPARENT
    };
    let control_height = ui.spacing().interact_size.y;
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).strong().color(if selected {
            accent
        } else {
            SECONDARY
        }))
        .fill(fill)
        .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
        .corner_radius(5)
        .min_size(Vec2::new(72.0, control_height))
        .selected(selected),
    );
    enhanced_click_feedback(ui, response, selected, accent)
}

fn destination_button(ui: &mut egui::Ui, selected: bool, label: &str) -> Response {
    let control_height = ui.spacing().interact_size.y;
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).strong().color(if selected {
            Color32::from_rgb(24, 25, 27)
        } else {
            SECONDARY
        }))
        .fill(if selected {
            Color32::from_rgb(225, 185, 65)
        } else {
            SURFACE_ALT
        })
        .stroke(Stroke::new(1.0, if selected { YELLOW } else { BORDER }))
        .corner_radius(6)
        .min_size(Vec2::new(68.0, control_height))
        .selected(selected),
    );
    enhanced_click_feedback(ui, response, selected, YELLOW)
}

fn state_button(ui: &mut egui::Ui, selected: bool, label: &str, accent: Color32) -> Response {
    let control_height = ui.spacing().interact_size.y;
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).strong().color(if selected {
            Color32::WHITE
        } else {
            TEXT
        }))
        .fill(if selected {
            accent.gamma_multiply(0.72)
        } else {
            Color32::from_rgb(37, 41, 48)
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                accent
            } else {
                Color32::from_rgb(53, 58, 67)
            },
        ))
        .corner_radius(5)
        .min_size(Vec2::new(52.0, control_height))
        .selected(selected),
    );
    enhanced_click_feedback(ui, response, selected, accent)
}

fn enhanced_click_feedback(
    ui: &egui::Ui,
    response: Response,
    selected: bool,
    accent: Color32,
) -> Response {
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(response.rect.shrink(1.0), 5.0, accent.gamma_multiply(0.28));
    }
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            5.0,
            Stroke::new(if selected { 2.0 } else { 1.5 }, accent),
            StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn badge(ui: &mut egui::Ui, label: &str, active: bool) {
    ui.label(
        egui::RichText::new(label)
            .size(8.0)
            .color(if active {
                Color32::from_rgb(200, 240, 213)
            } else {
                SECONDARY
            })
            .background_color(if active {
                Color32::from_rgb(27, 48, 35)
            } else {
                Color32::from_rgb(37, 41, 48)
            }),
    );
}
fn destination_name(destination: MixDestination) -> &'static str {
    match destination {
        MixDestination::Main => "MAIN",
        MixDestination::Monitor1 => "MON1",
        MixDestination::Monitor2 => "MON2",
        MixDestination::Fx1 => "FX1",
        MixDestination::Fx2 => "FX2",
    }
}
fn bus_id_for_destination(destination: MixDestination) -> Option<MixBusId> {
    match destination {
        MixDestination::Main => Some(MixBusId::Main),
        MixDestination::Monitor1 => Some(MixBusId::Monitor1),
        MixDestination::Monitor2 => Some(MixBusId::Monitor2),
        MixDestination::Fx1 | MixDestination::Fx2 => None,
    }
}
fn channel_number(id: InputId) -> &'static str {
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
fn display_name(id: InputId, language: Language) -> &'static str {
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
fn headphone_source_text(source: HeadphoneSource, language: Language) -> &'static str {
    match source {
        HeadphoneSource::Main => language.tr("MAIN mix", "MAIN 混音"),
        HeadphoneSource::Monitor => language.tr("Monitor mix", "监听混音"),
    }
}
fn tap_point_text(tap: TapPoint, language: Language) -> &'static str {
    match tap {
        TapPoint::PreFader => language.tr("Pre-fader", "推子前"),
        TapPoint::PostFader => language.tr("Post-fader", "推子后"),
    }
}
fn pan_text(value: f32, balance: bool) -> String {
    if value.abs() < 0.005 {
        if balance { "BAL C" } else { "PAN C" }.into()
    } else if value < 0.0 {
        format!("L {:.0}", value.abs() * 100.0)
    } else {
        format!("R {:.0}", value * 100.0)
    }
}
fn normalized_to_display_db(value: f32) -> f32 {
    flow8_protocol_curve(value).max(-60.0)
}
fn flow8_protocol_curve(value: f32) -> f32 {
    if value >= 1.0 {
        10.0
    } else if value >= 0.5 {
        40.0 * value - 30.0
    } else if value >= 0.25 {
        80.0 * value - 50.0
    } else if value >= 0.0625 {
        160.0 * value - 70.0
    } else if value >= 1.0 / 1024.0 {
        480.0 * value - 90.0
    } else {
        -144.0
    }
}

fn core_session_state(phase: SessionPhase) -> SessionState {
    match phase {
        SessionPhase::Disconnected => SessionState::Disconnected,
        SessionPhase::Scanning => SessionState::Scanning,
        SessionPhase::Connecting => SessionState::Connecting,
        SessionPhase::GattReady => SessionState::GattReady,
        SessionPhase::RxArming => SessionState::RxArming,
        SessionPhase::Handshaking => SessionState::Handshaking,
        SessionPhase::StateSyncing => SessionState::StateSyncing,
        SessionPhase::Ready => SessionState::Ready,
        SessionPhase::Error => SessionState::Error,
    }
}

fn session_phase_text(phase: SessionPhase, language: Language) -> &'static str {
    match phase {
        SessionPhase::Disconnected => language.tr("Disconnected", "已断开"),
        SessionPhase::Scanning => language.tr("Scanning", "正在扫描"),
        SessionPhase::Connecting => language.tr("Connecting", "正在连接"),
        SessionPhase::GattReady => language.tr("GATT ready", "GATT 已就绪"),
        SessionPhase::RxArming => language.tr("Arming receive path", "正在启用接收通道"),
        SessionPhase::Handshaking => language.tr("Handshaking", "正在握手"),
        SessionPhase::StateSyncing => language.tr("Synchronizing state", "正在同步状态"),
        SessionPhase::Ready => language.tr("Ready", "已就绪"),
        SessionPhase::Error => language.tr("Error", "错误"),
    }
}

fn session_state_text(state: SessionState, language: Language) -> &'static str {
    match state {
        SessionState::Disconnected => language.tr("Disconnected", "已断开"),
        SessionState::Scanning => language.tr("Scanning", "正在扫描"),
        SessionState::Connecting => language.tr("Connecting", "正在连接"),
        SessionState::GattReady => language.tr("GATT ready", "GATT 已就绪"),
        SessionState::RxArming => language.tr("Arming receive path", "正在启用接收通道"),
        SessionState::Handshaking => language.tr("Handshaking", "正在握手"),
        SessionState::StateSyncing => language.tr("Synchronizing state", "正在同步状态"),
        SessionState::Ready => language.tr("Ready", "已就绪"),
        SessionState::Error => language.tr("Error", "错误"),
    }
}

fn native_stage_text(stage: NativeConnectionStage, language: Language) -> &'static str {
    match stage {
        NativeConnectionStage::Scanning => language.tr("Scanning", "正在扫描"),
        NativeConnectionStage::DeviceFound => {
            language.tr("FLOW 8 device found", "已找到 FLOW 8 设备")
        }
        NativeConnectionStage::DeviceObjectCreated => {
            language.tr("FLOW 8 device object created", "已创建 FLOW 8 设备对象")
        }
        NativeConnectionStage::NativeInterfaceEnumerating => {
            language.tr("Enumerating native BLE interfaces", "正在枚举原生 BLE 接口")
        }
        NativeConnectionStage::FlowServiceSelected => {
            language.tr("FLOW service selected", "已选择 FLOW 服务")
        }
        NativeConnectionStage::NativeServiceHandleOpened => {
            language.tr("Native service handle opened", "已打开原生服务句柄")
        }
        NativeConnectionStage::CharacteristicsEnumerated => {
            language.tr("GATT characteristics enumerated", "已枚举 GATT 特征")
        }
        NativeConnectionStage::TargetCharacteristicFound => language.tr(
            "FLOW transport characteristic found",
            "已找到 FLOW 传输特征",
        ),
        NativeConnectionStage::NativeRxRegistering => {
            language.tr("Registering native receive path", "正在注册原生接收通道")
        }
        NativeConnectionStage::NativeRxArmed => {
            language.tr("Native receive path armed", "原生接收通道已启用")
        }
        NativeConnectionStage::Handshaking => {
            language.tr("Waiting for FLOW 8 handshake", "正在等待 FLOW 8 握手")
        }
    }
}

fn localized_status_message(mode: RunMode, state: SessionState, language: Language) -> String {
    match mode {
        RunMode::Simulator => language
            .tr(
                "Simulator mode · Synthetic mixer data · No FLOW 8 hardware connected",
                "模拟器模式 · 合成混音数据 · 未连接 FLOW 8 硬件",
            )
            .into(),
        RunMode::Ble => format!(
            "{}: {}",
            language.tr("Direct Bluetooth", "直连蓝牙"),
            session_state_text(state, language)
        ),
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn configure_style(context: &egui::Context, metrics: UiMetrics) {
    context.set_theme(egui::Theme::Dark);
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::proportional(metrics.heading_font),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::proportional(metrics.body_font),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::proportional(metrics.button_font),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::proportional(metrics.small_font),
    );
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SURFACE;
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(37, 41, 48);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(53, 58, 67));
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 62, 73);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::from_rgb(130, 145, 164));
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(72, 79, 91);
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, YELLOW);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(49, 55, 65);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.selection.bg_fill = BLUE.gamma_multiply(0.55);
    style.visuals.selection.stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    style.spacing.item_spacing = Vec2::splat(metrics.spacing);
    style.spacing.button_padding = Vec2::new(12.0 * metrics.ui_scale, 7.0 * metrics.ui_scale);
    style.spacing.interact_size.y = metrics.control_height;
    style.animation_time = 0.14;
    context.set_style_of(egui::Theme::Dark, style);
}

#[derive(Clone, Copy)]
struct SystemFontCandidate {
    family: &'static str,
    file_name: &'static str,
    collection_index: u32,
}

fn configure_fonts(context: &egui::Context) -> FontFallbackStatus {
    let Some((candidate, path, bytes)) = load_system_cjk_font() else {
        tracing::warn!(
            "no supported system CJK font was found; Simplified Chinese glyphs may be missing"
        );
        return FontFallbackStatus::default();
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut font_data = egui::FontData::from_owned(bytes);
    font_data.index = candidate.collection_index;
    fonts
        .font_data
        .insert("system-cjk".into(), font_data.into());

    // Keep egui's current Latin fonts first. The system CJK face is appended
    // only as a missing-glyph fallback, which keeps English metrics stable.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let names = fonts.families.entry(family).or_default();
        if !names.iter().any(|name| name == "system-cjk") {
            names.push("system-cjk".into());
        }
    }
    context.set_fonts(fonts);
    tracing::info!(
        family = candidate.family,
        path = %path.display(),
        "registered system CJK font fallback"
    );
    FontFallbackStatus {
        family: Some(candidate.family),
        path: Some(path),
    }
}

fn load_system_cjk_font() -> Option<(SystemFontCandidate, PathBuf, Vec<u8>)> {
    let candidates = cjk_font_candidates();
    for (root, recursive) in cjk_font_roots() {
        for candidate in &candidates {
            let exact = root.join(candidate.file_name);
            if let Ok(bytes) = std::fs::read(&exact) {
                return Some((*candidate, exact, bytes));
            }
        }
        if recursive
            && let Some((candidate, path)) = find_font_by_file_name(&root, &candidates, 4)
            && let Ok(bytes) = std::fs::read(&path)
        {
            return Some((candidate, path, bytes));
        }
    }
    None
}

fn cjk_font_candidates() -> Vec<SystemFontCandidate> {
    vec![
        SystemFontCandidate {
            family: "Microsoft YaHei UI",
            file_name: "msyh.ttc",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Microsoft YaHei",
            file_name: "msyh.ttf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "DengXian",
            file_name: "Deng.ttf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "SimSun",
            file_name: "simsun.ttc",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Noto Sans CJK SC",
            file_name: "NotoSansCJKsc-Regular.otf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Noto Sans CJK SC",
            file_name: "NotoSansCJK-Regular.ttc",
            collection_index: 2,
        },
        SystemFontCandidate {
            family: "Noto Sans SC",
            file_name: "NotoSansSC-Regular.ttf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "Source Han Sans SC",
            file_name: "SourceHanSansSC-Regular.otf",
            collection_index: 0,
        },
        SystemFontCandidate {
            family: "WenQuanYi Micro Hei",
            file_name: "wqy-microhei.ttc",
            collection_index: 0,
        },
    ]
}

fn cjk_font_roots() -> Vec<(PathBuf, bool)> {
    let mut roots = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(windows_dir) = std::env::var_os("WINDIR") {
            roots.push((PathBuf::from(windows_dir).join("Fonts"), false));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        for root in [
            "/usr/share/fonts",
            "/usr/local/share/fonts",
            "/usr/share/fonts/opentype/noto",
            "/usr/share/fonts/truetype/wqy",
        ] {
            roots.push((PathBuf::from(root), true));
        }
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            roots.push((PathBuf::from(data_home).join("fonts"), true));
        }
    }
    roots
}

fn find_font_by_file_name(
    root: &Path,
    candidates: &[SystemFontCandidate],
    remaining_depth: usize,
) -> Option<(SystemFontCandidate, PathBuf)> {
    if remaining_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_font_by_file_name(&path, candidates, remaining_depth - 1) {
                return Some(found);
            }
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(candidate) = candidates
            .iter()
            .find(|candidate| file_name.eq_ignore_ascii_case(candidate.file_name))
        {
            return Some((*candidate, path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_eleven_pages_render_in_english_and_simplified_chinese() {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);

        for size in [
            Vec2::new(1040.0, 700.0),
            Vec2::new(1280.0, 720.0),
            Vec2::new(1440.0, 920.0),
            Vec2::new(1920.0, 1080.0),
            Vec2::new(2560.0, 1080.0),
        ] {
            app.metrics = UiMetrics::calculate(size, 1.0, app.preferences.ui_scale);
            configure_style(&context, app.metrics);
            for language in [Language::English, Language::Chinese] {
                app.language = language;
                for page in [
                    Page::Mixer,
                    Page::Stage,
                    Page::Fx1,
                    Page::Fx2,
                    Page::Monitor1,
                    Page::Monitor2,
                    Page::Main,
                    Page::MainOut,
                    Page::Routing,
                    Page::Snapshots,
                    Page::Settings,
                ] {
                    app.page = page;
                    let raw = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    };
                    let mut output = context.run_ui(raw, |ui| {
                        app.connection_bar(ui);
                        app.layer_bar(ui);
                        match page {
                            Page::Mixer
                            | Page::Fx1
                            | Page::Fx2
                            | Page::Monitor1
                            | Page::Monitor2
                            | Page::Main => app.mixer(ui),
                            Page::Stage => app.stage_page(ui),
                            Page::MainOut => app.main_out_page(ui),
                            Page::Routing => app.routing_page(ui),
                            Page::Snapshots => app.snapshots_page(ui),
                            Page::Settings => app.settings_page(ui),
                        }
                    });
                    assert!(
                        !output.shapes.is_empty(),
                        "page {page:?} rendered no shapes for {language:?} at {size:?}"
                    );
                    output.textures_delta.clear();
                }
            }
        }
    }

    #[test]
    fn responsive_metrics_scale_fonts_controls_and_mixer_widths() {
        let compact = UiMetrics::calculate(Vec2::new(1040.0, 700.0), 1.0, 1.0);
        let enlarged = UiMetrics::calculate(Vec2::new(1040.0, 700.0), 1.0, 1.40);
        assert!(enlarged.body_font > compact.body_font);
        assert!(enlarged.control_height > compact.control_height);
        let compact_layout = LayoutMetrics::calculate(Vec2::new(650.0, 700.0), compact, 7);
        let wide_layout = LayoutMetrics::calculate(Vec2::new(1600.0, 900.0), enlarged, 7);
        assert!(compact_layout.mixer_scrolls);
        assert!(!wide_layout.mixer_scrolls);
        assert!(wide_layout.channel_width <= 146.0 * enlarged.ui_scale);
    }

    #[test]
    fn interactive_visual_states_are_distinct() {
        let context = egui::Context::default();
        let metrics = UiMetrics::calculate(Vec2::new(1440.0, 920.0), 1.0, 1.0);
        configure_style(&context, metrics);
        let style = context.style_of(egui::Theme::Dark);
        assert_ne!(
            style.visuals.widgets.inactive.bg_fill,
            style.visuals.widgets.hovered.bg_fill
        );
        assert_ne!(
            style.visuals.widgets.hovered.bg_fill,
            style.visuals.widgets.active.bg_fill
        );
        assert_ne!(
            style.visuals.widgets.inactive.fg_stroke.color,
            style.visuals.widgets.noninteractive.fg_stroke.color
        );
        assert!((style.animation_time - 0.14).abs() < f32::EPSILON);
    }

    #[test]
    fn language_switch_changes_all_representative_visible_strings() {
        let keys = [
            ("Connect", "连接"),
            ("MIXER", "混音器"),
            ("INPUT DETAIL", "输入详情"),
            ("MAIN OUT", "主输出"),
            ("FX ENGINE", "效果器"),
            ("Routing", "路由"),
            ("Stage View", "舞台视图"),
            ("Device Snapshots", "设备快照"),
            ("Preferences", "偏好设置"),
            ("Connection failed", "连接失败"),
            (
                "Wait for a complete FLOW 8 state sync before editing.",
                "请等待 FLOW 8 完成状态同步后再进行调整。",
            ),
        ];
        for (english, chinese) in keys {
            assert!(!english.is_empty());
            assert!(!chinese.is_empty());
            assert_ne!(
                Language::English.tr(english, chinese),
                Language::Chinese.tr(english, chinese)
            );
        }
    }

    #[test]
    fn system_cjk_fallback_initialization_never_panics() {
        let context = egui::Context::default();
        let status = configure_fonts(&context);
        if let Some(path) = status.path {
            assert!(path.is_file());
            assert!(status.family.is_some());
        }
    }

    #[test]
    fn settings_render_does_not_enqueue_unknown_or_disabled_writes() {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        app.language = Language::Chinese;
        assert!(app.store.queue.is_empty());
        let mut output = context.run_ui(egui::RawInput::default(), |ui| app.settings_page(ui));
        assert!(!output.shapes.is_empty());
        assert!(app.store.queue.is_empty());
        output.textures_delta.clear();
    }

    #[test]
    fn verbose_flow8_ble_filter_enables_production_log_capture() {
        assert!(flow8_ble_verbose_logging("flow8_ble=debug,flow8_gui=info"));
        assert!(flow8_ble_verbose_logging("flow8_ble=trace"));
        assert!(!flow8_ble_verbose_logging("flow8_ble=info"));
        assert!(!flow8_ble_verbose_logging("flow8_gui=debug"));
    }
}

#[derive(Clone)]
struct ProductionLogWriter {
    file: Arc<Mutex<File>>,
}

struct ProductionLogSink {
    file: Arc<Mutex<File>>,
    stdout: io::Stdout,
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for ProductionLogWriter {
    type Writer = ProductionLogSink;

    fn make_writer(&'writer self) -> Self::Writer {
        ProductionLogSink {
            file: Arc::clone(&self.file),
            stdout: io::stdout(),
        }
    }
}

impl Write for ProductionLogSink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.file
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .write_all(buffer)?;
        let _ = self.stdout.write_all(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .flush()?;
        let _ = self.stdout.flush();
        Ok(())
    }
}

fn flow8_ble_verbose_logging(filter: &str) -> bool {
    filter.split(',').any(|directive| {
        let directive = directive.trim().to_ascii_lowercase();
        directive == "flow8_ble=debug" || directive == "flow8_ble=trace"
    })
}

fn init_tracing() {
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "flow8_ble=info".into());
    let filter = tracing_subscriber::EnvFilter::try_new(&rust_log)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("flow8_ble=info"));
    if flow8_ble_verbose_logging(&rust_log) {
        let path = Path::new("captures/hardware/windows-production-connection.log");
        let writer = fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
            .and_then(|()| File::create(path));
        match writer {
            Ok(file) => {
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_writer(ProductionLogWriter {
                        file: Arc::new(Mutex::new(file)),
                    })
                    .try_init();
                return;
            }
            Err(error) => eprintln!("warning: could not create {}: {error}", path.display()),
        }
    }
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn main() -> eframe::Result {
    init_tracing();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FLOW 8 PC Controller")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1040.0, 700.0]),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "FLOW 8 PC Controller",
        options,
        Box::new(|context| Ok(Box::new(Flow8App::new(context)))),
    )
}
