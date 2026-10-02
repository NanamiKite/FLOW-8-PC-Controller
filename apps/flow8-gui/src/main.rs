#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};
use flow8_ble::{DeviceCommand, DeviceEvent, DeviceRuntime, NativeConnectionStage, SessionPhase};
use flow8_core::{
    ChannelStateTarget, Flow8Store, KnownSetting, MuteTarget, SemanticCommand, SessionState,
};
use flow8_model::{
    EqState, EvidenceStatus, FX_PRESET_COUNT, FxId, HeadphoneSource, InputChannelState, InputId,
    MixBusId, MixDestination, MonitorRoutingSource, ParameterSpec, SnapshotSlotState, TapPoint,
    fx_preset_info, specs,
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
            control_height: (30.0 * scale).round_ui(),
            // add_space advances the cursor without rounding it. Keep scaled
            // spacing on egui's logical grid so trailing gaps stay aligned too.
            spacing: (8.0 * scale).round_ui(),
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
            .clamp(290.0 * ui.ui_scale, 430.0 * ui.ui_scale)
            .round_ui();
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
        let fader_height = (available.y - 290.0 * ui.ui_scale)
            .clamp(160.0 * ui.ui_scale, 320.0 * ui.ui_scale)
            .round_ui();
        Self {
            fader_height,
            channel_width: natural_channel_width
                .clamp(channel_min_width, channel_max_width)
                .round_ui(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeviceSlotStatus {
    Unknown,
    Free,
    Occupied(String),
}

fn device_slot_status(slot: &SnapshotSlotState) -> DeviceSlotStatus {
    match slot.name.confirmed.as_deref() {
        None => DeviceSlotStatus::Unknown,
        Some("") => DeviceSlotStatus::Free,
        Some(name) => DeviceSlotStatus::Occupied(name.to_owned()),
    }
}

#[derive(Debug, Clone, Copy)]
struct SnapshotGridLayout {
    columns: usize,
    card_inner_width: f32,
    button_width: f32,
    gap: f32,
}

impl SnapshotGridLayout {
    fn new(available_width: f32, scale: f32, gap: f32, slots: usize) -> Self {
        // Count a new column only when another complete card, including four
        // usable buttons and the card margins, fits in the visible width.
        let min_card_width = (4.0 * 90.0 * scale + 3.0 * gap + 28.0).max(410.0 * scale);
        let columns = (((available_width + gap) / (min_card_width + gap)).floor() as usize)
            .clamp(1, slots.max(1));
        let card_width =
            ((available_width - gap * (columns - 1) as f32) / columns as f32).min(560.0 * scale);
        // The card has 12-point side margins; leave extra rounding slack.
        let card_inner_width = (card_width - 28.0).max(0.0).floor();
        let button_width = ((card_inner_width - 3.0 * gap) / 4.0).max(0.0).floor();
        Self {
            columns,
            card_inner_width,
            button_width,
            gap,
        }
    }

    fn group_width(self) -> f32 {
        // Frame: two 12-point inner margins and two 1-point strokes.
        self.columns as f32 * (self.card_inner_width + 26.0) + (self.columns - 1) as f32 * self.gap
    }
}

fn ellipsize_snapshot_name(
    painter: &egui::Painter,
    name: &str,
    font: &FontId,
    max_width: f32,
) -> String {
    let width = |text: String| painter.layout_no_wrap(text, font.clone(), TEXT).size().x;
    if width(name.to_owned()) <= max_width {
        return name.to_owned();
    }
    let characters: Vec<char> = name.chars().collect();
    let mut low = 0;
    let mut high = characters.len();
    while low < high {
        let middle = (low + high + 1) / 2;
        let candidate = format!("{}…", characters[..middle].iter().collect::<String>());
        if width(candidate) <= max_width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    format!("{}…", characters[..low].iter().collect::<String>())
}

fn snapshot_status_badge(
    ui: &mut egui::Ui,
    status: &DeviceSlotStatus,
    language: Language,
) -> Response {
    let (label, fill, foreground) = match status {
        DeviceSlotStatus::Free => (
            language.tr("Free", "空闲"),
            GREEN.gamma_multiply(0.35),
            GREEN,
        ),
        DeviceSlotStatus::Occupied(_) => (
            language.tr("Occupied", "已占用"),
            RED.gamma_multiply(0.35),
            RED,
        ),
        DeviceSlotStatus::Unknown => (language.tr("Unknown", "未知"), SURFACE_ALT, SECONDARY),
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).color(foreground));
        })
        .response
}

fn snapshot_name_row(
    ui: &mut egui::Ui,
    name: Option<&str>,
    busy: bool,
    width: f32,
    font_size: f32,
    language: Language,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, (font_size * 1.4).round_ui()),
        Sense::hover(),
    );
    let text = if busy {
        language.tr("Waiting for device", "等待设备回报")
    } else {
        name.unwrap_or("")
    };
    if !text.is_empty() {
        let painter = ui.painter().with_clip_rect(rect);
        let font = FontId::proportional(font_size);
        let display = ellipsize_snapshot_name(&painter, text, &font, rect.width() - 4.0);
        painter.text(
            rect.left_center() + Vec2::new(2.0, 0.0),
            egui::Align2::LEFT_CENTER,
            display,
            font,
            if busy { SECONDARY } else { TEXT },
        );
    }
    if let Some(name) = name {
        response.on_hover_text(name)
    } else {
        response
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfirmationAction {
    EnablePhantom(InputId),
    SaveDeviceSnapshot {
        slot: u8,
        name: String,
        observed_name: Option<String>,
    },
    LoadDeviceSnapshot {
        slot: u8,
        observed_name: Option<String>,
    },
    DeleteDeviceSnapshot {
        slot: u8,
        observed_name: Option<String>,
    },
    RenameDeviceSnapshot {
        slot: u8,
        name: String,
        observed_name: Option<String>,
    },
}

fn valid_snapshot_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && trimmed.len() <= 20 && !trimmed.chars().any(char::is_control)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingConfirmation {
    action: ConfirmationAction,
    page: Page,
    selected_input: Option<InputId>,
    session: SessionState,
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

#[derive(Clone, Copy)]
struct MeterSmoother {
    display_db: f32,
}

impl Default for MeterSmoother {
    fn default() -> Self {
        Self {
            display_db: specs::METER_DISPLAY.min,
        }
    }
}

impl MeterSmoother {
    fn advance(&mut self, target_db: Option<f32>, elapsed_seconds: f32) {
        let Some(target_db) = target_db else {
            *self = Self::default();
            return;
        };
        let target_db = if target_db.is_nan() {
            specs::METER_DISPLAY.min
        } else {
            target_db.clamp(specs::METER_DISPLAY.min, specs::METER_DISPLAY.max)
        };
        // Fast attack, slower release. This changes only the painted bar.
        let time_constant = if target_db > self.display_db {
            0.08
        } else {
            0.35
        };
        let blend = 1.0 - (-elapsed_seconds / time_constant).exp();
        self.display_db += (target_db - self.display_db) * blend;
        if (target_db - self.display_db).abs() < 0.05 {
            self.display_db = target_db;
        }
    }
}

#[derive(Clone, Copy)]
struct EqBandSnapshot {
    frequency_hz: f32,
    gain_db: f32,
    q: f32,
}

fn confirmed_eq_snapshot(channel: &InputChannelState) -> Option<[EqBandSnapshot; 4]> {
    if !channel.capabilities.peq || channel.eq.bands.len() != 4 {
        return None;
    }
    let mut snapshot = [EqBandSnapshot {
        frequency_hz: 0.0,
        gain_db: 0.0,
        q: 0.0,
    }; 4];
    for (value, band) in snapshot.iter_mut().zip(&channel.eq.bands) {
        *value = EqBandSnapshot {
            frequency_hz: band.frequency_hz.confirmed?,
            gain_db: band.gain_db.confirmed?,
            q: band.q.confirmed?,
        };
    }
    Some(snapshot)
}

fn input_eq_modified(channel: &InputChannelState, initial: Option<&[EqBandSnapshot; 4]>) -> bool {
    let Some(current) = confirmed_eq_snapshot(channel) else {
        return false;
    };
    // A non-flat gain is meaningful even when it was already present at connect.
    if current.iter().any(|band| band.gain_db.abs() > 0.05) {
        return true;
    }
    // Factory frequency/Q defaults are not verified. Compare those parameters
    // only with the first complete state received in this connection.
    initial.is_some_and(|initial| {
        current.iter().zip(initial).any(|(now, before)| {
            (now.frequency_hz - before.frequency_hz).abs() > 0.5
                || (now.q - before.q).abs() > 0.01
                || (now.gain_db - before.gain_db).abs() > 0.05
        })
    })
}

fn format_delay_ms(ticks: u32) -> String {
    format!("{:.3}", ticks as f64 / 48.0)
}

fn parse_delay_ms(value: &str) -> Option<u32> {
    let milliseconds = value.trim().parse::<f64>().ok()?;
    let ticks = (milliseconds * 48.0).round();
    (milliseconds.is_finite() && milliseconds >= 0.0 && ticks <= u32::MAX as f64)
        .then_some(ticks as u32)
}

#[derive(Clone, Copy)]
struct SettingsGridLayout {
    two_columns: bool,
    column_width: f32,
    label_width: f32,
    gap: f32,
    scale: f32,
}

impl SettingsGridLayout {
    fn calculate(
        ui: &egui::Ui,
        content_width: f32,
        metrics: UiMetrics,
        language: Language,
    ) -> Self {
        let gap = (12.0 * metrics.ui_scale).round_ui();
        let label_width = [
            language.tr("Bluetooth / USB to Headphones Only", "蓝牙 / USB 仅送耳机"),
            language.tr("Device-linked Output Selection", "设备联动输出选择"),
            language.tr("Show Output Delay Indicator", "显示输出延迟指示"),
            language.tr("Connection progress", "连接进度"),
        ]
        .into_iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap(
                    label.to_owned(),
                    FontId::proportional(metrics.body_font),
                    TEXT,
                )
                .size()
                .x
        })
        .fold(0.0, f32::max)
            + gap;
        // The breakpoint follows actual form needs: measured labels, a usable
        // value/control area, card margins, and the gutter between columns.
        let min_control_width = 220.0 * metrics.ui_scale;
        let min_column_width = label_width + min_control_width + 26.0 + gap;
        let two_columns = content_width >= 2.0 * min_column_width + gap;
        let column_width = if two_columns {
            ((content_width - gap) / 2.0).floor()
        } else {
            content_width
        };
        Self {
            two_columns,
            column_width,
            label_width,
            gap,
            scale: metrics.ui_scale,
        }
    }
}

fn settings_grid_row(
    app: &mut Flow8App,
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    row_id: u8,
    left: impl FnOnce(&mut Flow8App, &mut egui::Ui),
    right: impl FnOnce(&mut Flow8App, &mut egui::Ui),
) {
    if layout.two_columns {
        // Both cards start at the same top edge. Advance the parent only once,
        // by the taller card's actual height; Grid centers zero-height child
        // Uis within its previous row height and leaves a large empty band.
        let available = ui.available_rect_before_wrap();
        let top = available.top();
        let left_x = available.left();
        let right_x = left_x + layout.column_width + layout.gap;
        let height = available.height().max(0.0);
        let mut left_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("settings-left", row_id))
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(left_x, top),
                    Vec2::new(layout.column_width, height),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        left(app, &mut left_ui);
        let left_bottom = left_ui.min_rect().bottom();

        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("settings-right", row_id))
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(right_x, top),
                    Vec2::new(layout.column_width, height),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        right(app, &mut right_ui);
        let right_bottom = right_ui.min_rect().bottom();

        ui.advance_cursor_after_rect(egui::Rect::from_min_max(
            egui::pos2(left_x, top),
            egui::pos2(right_x + layout.column_width, left_bottom.max(right_bottom)),
        ));
    } else {
        left(app, ui);
        ui.add_space(layout.gap);
        right(app, ui);
    }
}

fn settings_card<R>(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card(ui, |ui| {
        ui.set_width((layout.column_width - 26.0).max(0.0));
        add(ui)
    })
}

fn setting_switch_text(language: Language, enabled: bool) -> &'static str {
    if enabled {
        language.tr("On", "开启")
    } else {
        language.tr("Off", "关闭")
    }
}

fn settings_form_row(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    label: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    let min_value_width = 220.0 * layout.scale;
    if ui.available_width() < layout.label_width + min_value_width + layout.gap {
        ui.vertical(|ui| {
            ui.label(label);
            add(ui);
        });
    } else {
        ui.horizontal(|ui| {
            ui.add_sized(
                [layout.label_width, ui.spacing().interact_size.y],
                egui::Label::new(label),
            );
            add(ui);
        });
    }
}

struct Flow8App {
    store: Flow8Store,
    page: Page,
    language: Language,
    message: String,
    runtime: DeviceRuntime,
    discovered_devices: Vec<flow8_ble::DiscoveredDevice>,
    stage_layout_mode: bool,
    stage_positions: [Vec2; 7],
    pending_confirmation: Option<PendingConfirmation>,
    snapshot_names_requested: bool,
    channel_labels_requested: bool,
    meter_request_target: Option<MixDestination>,
    input_meter_display: [MeterSmoother; 7],
    output_meter_display: [MeterSmoother; 5],
    meter_frame_at: Option<Instant>,
    state_sync_applied: bool,
    eq_initial: [Option<[EqBandSnapshot; 4]>; 7],
    delay_ms_draft: [String; 3],
    delay_ms_dirty: [bool; 3],
    native_stage: Option<NativeConnectionStage>,
    last_ble_error: Option<String>,
    preferences: AppPreferences,
    metrics: UiMetrics,
    inspector_open: bool,
    inspector_input: bool,
}

impl Flow8App {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        Self::from_context(&context.egui_ctx)
    }

    fn from_context(context: &egui::Context) -> Self {
        configure_fonts(context);
        let metrics =
            UiMetrics::calculate(Vec2::new(1440.0, 920.0), context.pixels_per_point(), 1.0);
        configure_style(context, metrics);
        Self {
            store: Flow8Store::disconnected(),
            page: Page::Mixer,
            language: Language::English,
            message: "Disconnected · Connect to FLOW 8 to load the current mixer state".into(),
            runtime: DeviceRuntime::spawn(),
            discovered_devices: Vec::new(),
            stage_layout_mode: false,
            stage_positions: std::array::from_fn(|index| {
                Vec2::new(
                    34.0 + (index % 4) as f32 * 190.0,
                    42.0 + (index / 4) as f32 * 210.0,
                )
            }),
            pending_confirmation: None,
            snapshot_names_requested: false,
            channel_labels_requested: false,
            meter_request_target: None,
            input_meter_display: [MeterSmoother::default(); 7],
            output_meter_display: [MeterSmoother::default(); 5],
            meter_frame_at: None,
            state_sync_applied: false,
            eq_initial: [None; 7],
            delay_ms_draft: std::array::from_fn(|_| String::new()),
            delay_ms_dirty: [false; 3],
            native_stage: None,
            last_ble_error: None,
            preferences: AppPreferences::default(),
            metrics,
            inspector_open: true,
            inspector_input: true,
        }
    }

    fn dispatch(&mut self, command: SemanticCommand) {
        if self.store.state.session != SessionState::Ready {
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

    fn request_confirmation(&mut self, action: ConfirmationAction) {
        if self.store.state.session != SessionState::Ready {
            return;
        }
        self.pending_confirmation = Some(PendingConfirmation {
            action,
            page: self.page,
            selected_input: self.store.state.selected_input,
            session: self.store.state.session,
        });
    }

    fn confirmation_is_valid(&self, pending: &PendingConfirmation) -> bool {
        if self.page != pending.page
            || self.store.state.selected_input != pending.selected_input
            || self.store.state.session != pending.session
            || pending.session != SessionState::Ready
        {
            return false;
        }
        match &pending.action {
            ConfirmationAction::EnablePhantom(input) => {
                self.store.state.selected_input == Some(*input)
                    && self.store.state.channels[input.index()]
                        .capabilities
                        .phantom
                    && self.store.state.channels[input.index()]
                        .phantom_48v
                        .effective()
                        == Some(&false)
            }
            ConfirmationAction::SaveDeviceSnapshot {
                slot,
                observed_name,
                ..
            }
            | ConfirmationAction::LoadDeviceSnapshot {
                slot,
                observed_name,
            }
            | ConfirmationAction::DeleteDeviceSnapshot {
                slot,
                observed_name,
            }
            | ConfirmationAction::RenameDeviceSnapshot {
                slot,
                observed_name,
                ..
            } => self
                .store
                .state
                .snapshots
                .device_slots
                .get(*slot as usize)
                .is_some_and(|current| current.name.confirmed == *observed_name),
        }
    }

    fn clear_stale_confirmation(&mut self) {
        if self
            .pending_confirmation
            .as_ref()
            .is_some_and(|pending| !self.confirmation_is_valid(pending))
        {
            self.pending_confirmation = None;
        }
    }

    fn confirm_pending_action(&mut self) {
        let Some(pending) = self.pending_confirmation.take() else {
            return;
        };
        if !self.confirmation_is_valid(&pending) {
            self.message = self
                .language
                .tr(
                    "The device or selected context changed. Choose the action again.",
                    "设备状态或选择已变化，请重新选择操作。",
                )
                .into();
            return;
        }
        let command = match pending.action {
            ConfirmationAction::EnablePhantom(input) => SemanticCommand::SetPhantom {
                input,
                enabled: true,
            },
            ConfirmationAction::SaveDeviceSnapshot { slot, name, .. } => {
                if !valid_snapshot_name(&name) {
                    return;
                }
                SemanticCommand::SaveSnapshot {
                    slot,
                    name: name.trim().to_owned(),
                }
            }
            ConfirmationAction::RenameDeviceSnapshot { slot, name, .. } => {
                if !valid_snapshot_name(&name) {
                    return;
                }
                SemanticCommand::RenameSnapshot {
                    slot,
                    name: name.trim().to_owned(),
                }
            }
            ConfirmationAction::LoadDeviceSnapshot { slot, .. } => {
                SemanticCommand::LoadSnapshot { slot }
            }
            ConfirmationAction::DeleteDeviceSnapshot { slot, .. } => {
                SemanticCommand::DeleteSnapshot { slot }
            }
        };
        self.dispatch(command);
    }

    fn confirmation_dialog(&mut self, context: &egui::Context) {
        self.clear_stale_confirmation();
        let Some(mut pending) = self.pending_confirmation.clone() else {
            return;
        };
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.pending_confirmation = None;
            return;
        }
        let (title, description) = match pending.action.clone() {
            ConfirmationAction::EnablePhantom(input) => (
                self.language.tr("Enable 48 V?", "开启 48 V？").to_owned(),
                format!(
                    "{} {}. {}",
                    self.language.tr("Input", "输入"),
                    input.index() + 1,
                    self.language.tr(
                        "48 V may damage incompatible microphones or connected equipment. Only this input will change.",
                        "48 V 可能损坏不兼容的麦克风或连接设备；仅此输入通道会改变。",
                    )
                ),
            ),
            ConfirmationAction::SaveDeviceSnapshot { slot, observed_name, .. } => (
                self.language.tr("Save device snapshot?", "保存设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    match observed_name.as_deref() {
                        Some("") => self.language.tr(
                            "Current mixer settings will be saved to this free device slot.",
                            "当前混音设置将保存到此空闲设备槽位。",
                        ),
                        Some(_) => self.language.tr(
                            "Current mixer settings will replace the snapshot stored in this slot.",
                            "当前混音设置会覆盖此槽位中的快照。",
                        ),
                        None => self.language.tr(
                            "Slot occupancy is unknown. Saving may overwrite a device snapshot.",
                            "槽位占用状态未知；保存可能覆盖设备快照。",
                        ),
                    }
                ),
            ),
            ConfirmationAction::LoadDeviceSnapshot { slot, .. } => (
                self.language.tr("Load device snapshot?", "载入设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    self.language.tr(
                        "This applies the saved snapshot to the live FLOW 8 mixer. Current settings may change according to the snapshot scope.",
                        "这会将快照应用到 FLOW 8 当前混音；具体设置可能按快照范围改变。",
                    )
                ),
            ),
            ConfirmationAction::DeleteDeviceSnapshot { slot, .. } => (
                self.language.tr("Delete device snapshot?", "删除设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    self.language.tr(
                        "The snapshot stored on FLOW 8 will be deleted. This cannot be undone here.",
                        "FLOW 8 上保存的快照将被删除，此处无法撤销。",
                    )
                ),
            ),
            ConfirmationAction::RenameDeviceSnapshot { slot, .. } => (
                self.language.tr("Rename device snapshot?", "重命名设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    self.language.tr(
                        "Only the name stored in this FLOW 8 slot will change.",
                        "只修改 FLOW 8 此槽位保存的名称。",
                    )
                ),
            ),
        };
        let mut open = true;
        let mut decision = None;
        egui::Window::new(title)
            .id(egui::Id::new("device-action-confirmation"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(context, |ui| {
                ui.set_max_width(420.0);
                ui.label(description);
                let mut can_confirm = true;
                if let ConfirmationAction::SaveDeviceSnapshot { name, .. }
                | ConfirmationAction::RenameDeviceSnapshot { name, .. } = &mut pending.action
                {
                    ui.add_space(8.0);
                    ui.label(self.language.tr("Snapshot name", "快照名称"));
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .desired_width(300.0)
                            .hint_text(self.language.tr("Name", "名称")),
                    );
                    can_confirm = valid_snapshot_name(name);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} / 20 {}",
                            name.trim().len(),
                            self.language.tr("UTF-8 bytes", "UTF-8 字节")
                        ))
                        .color(if can_confirm { SECONDARY } else { RED }),
                    );
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(self.language.tr("Cancel", "取消")).clicked() {
                        decision = Some(false);
                    }
                    if ui
                        .add_enabled(
                            can_confirm,
                            egui::Button::new(self.language.tr("Confirm", "确认")),
                        )
                        .clicked()
                    {
                        decision = Some(true);
                    }
                });
            });
        self.pending_confirmation = Some(pending);
        match decision {
            Some(true) => self.confirm_pending_action(),
            Some(false) => self.pending_confirmation = None,
            None if !open => self.pending_confirmation = None,
            None => {}
        }
    }

    fn poll_runtime(&mut self) {
        while let Some(event) = self.runtime.try_recv() {
            match event {
                DeviceEvent::Phase(phase) => {
                    if matches!(
                        phase,
                        SessionPhase::Disconnected
                            | SessionPhase::Scanning
                            | SessionPhase::Connecting
                    ) {
                        self.eq_initial = [None; 7];
                        self.delay_ms_draft = std::array::from_fn(|_| String::new());
                        self.delay_ms_dirty = [false; 3];
                    }
                    if phase != SessionPhase::Ready {
                        self.pending_confirmation = None;
                        self.snapshot_names_requested = false;
                        self.channel_labels_requested = false;
                        self.meter_request_target = None;
                    }
                    if matches!(phase, SessionPhase::Connecting | SessionPhase::StateSyncing) {
                        self.state_sync_applied = false;
                    }
                    if phase == SessionPhase::Ready {
                        self.last_ble_error = None;
                        self.native_stage = None;
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
                            self.language.tr("Bluetooth", "蓝牙"),
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
                        if self.eq_initial.iter().all(Option::is_none) {
                            for (initial, channel) in
                                self.eq_initial.iter_mut().zip(&self.store.state.channels)
                            {
                                *initial = confirmed_eq_snapshot(channel);
                            }
                        }
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
                // Raw transport details remain in tracing logs, not the user-facing status bar.
                DeviceEvent::RawRx(_)
                | DeviceEvent::RawTx(_)
                | DeviceEvent::WriteMode(_)
                | DeviceEvent::Backend(_)
                | DeviceEvent::Mtu(_) => {}
                DeviceEvent::ProtocolWarning(warning) => {
                    self.message = format!(
                        "{}: {warning}",
                        self.language.tr("Protocol warning", "协议告警")
                    );
                }
                DeviceEvent::CommandError(error) => {
                    self.message = format!(
                        "{}: {error}",
                        self.language.tr("Command failed", "命令失败")
                    );
                }
                DeviceEvent::Error(error) => {
                    self.pending_confirmation = None;
                    self.snapshot_names_requested = false;
                    self.channel_labels_requested = false;
                    self.store.state.session = SessionState::Error;
                    self.last_ble_error = Some(error.clone());
                    self.message = format!("BLE: {error}");
                }
            }
        }

        self.flush_commands();
    }

    fn sync_channel_labels_request(&mut self) {
        if self.store.state.session != SessionState::Ready {
            self.channel_labels_requested = false;
        } else if !self.channel_labels_requested {
            self.dispatch(SemanticCommand::RequestChannelLabels);
            self.channel_labels_requested = true;
        }
    }

    fn sync_meter_request(&mut self) {
        if self.store.state.session != SessionState::Ready {
            self.meter_request_target = None;
            return;
        }
        let destination = self.store.state.selected_destination;
        if self.meter_request_target != Some(destination) {
            self.dispatch(SemanticCommand::RequestMeters { destination });
            self.meter_request_target = Some(destination);
        }
    }

    fn advance_meter_display(&mut self) {
        if self.store.state.session != SessionState::Ready {
            self.input_meter_display = [MeterSmoother::default(); 7];
            self.output_meter_display = [MeterSmoother::default(); 5];
            self.meter_frame_at = None;
            return;
        }

        let now = Instant::now();
        let elapsed_seconds = self
            .meter_frame_at
            .replace(now)
            .map(|previous| now.saturating_duration_since(previous).as_secs_f32())
            .unwrap_or(1.0 / 30.0)
            .clamp(0.0, 0.25);

        for (index, smoother) in self.input_meter_display.iter_mut().enumerate() {
            smoother.advance(
                self.store.state.channels[index].meter.level_db.confirmed,
                elapsed_seconds,
            );
        }
        for destination in MixDestination::ALL {
            let target = if let Some(bus) = self.store.state.bus_for_destination(destination) {
                bus.meter.level_db.confirmed
            } else {
                self.store.state.effects[usize::from(destination == MixDestination::Fx2)]
                    .meter
                    .level_db
                    .confirmed
            };
            self.output_meter_display[destination.index()].advance(target, elapsed_seconds);
        }
    }

    fn flush_commands(&mut self) {
        while let Some(command) = self.store.queue.pop() {
            if let Err(error) = self
                .runtime
                .send(DeviceCommand::Send(command.to_protocol()))
            {
                self.store.queue.push_front(command);
                self.message = error;
                break;
            }
        }
    }

    fn connection_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SURFACE)
            .inner_margin(egui::Margin::symmetric(18, 10))
            .show(ui, |ui| {
                // The outer page uses zero spacing between bars; controls need their own gaps.
                ui.spacing_mut().item_spacing.x = self.metrics.spacing;
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
                        ui.menu_button(self.language.tr("Setup", "设置"), |ui| {
                            if ui
                                .button(self.language.tr("Preferences", "偏好设置"))
                                .clicked()
                            {
                                self.page = Page::Settings;
                                ui.close();
                            }
                            if small_tab(
                                ui,
                                self.page == Page::Snapshots,
                                self.language.tr("Snapshots", "快照"),
                                TEXT,
                            )
                            .clicked()
                            {
                                self.page = Page::Snapshots;
                                ui.close();
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
                                ui.close();
                            }
                        });
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
                            self.message =
                                localized_status_message(self.store.state.session, self.language);
                            ui.ctx().request_repaint();
                        }
                        if ui.button(self.language.tr("Disconnect", "断开")).clicked() {
                            self.pending_confirmation = None;
                            self.snapshot_names_requested = false;
                            let _ = self.runtime.send(DeviceCommand::Disconnect);
                        }
                        let can_connect = matches!(
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
                            // A fresh session must not expose stale values from the prior device.
                            self.store = Flow8Store::disconnected();
                            self.pending_confirmation = None;
                            self.snapshot_names_requested = false;
                            self.channel_labels_requested = false;
                            self.meter_request_target = None;
                            self.state_sync_applied = false;
                            self.eq_initial = [None; 7];
                            self.delay_ms_draft = std::array::from_fn(|_| String::new());
                            self.delay_ms_dirty = [false; 3];
                            match self.runtime.send(DeviceCommand::Connect) {
                                Ok(()) => {
                                    self.store.state.session = SessionState::Connecting;
                                    self.native_stage = None;
                                    self.last_ble_error = None;
                                }
                                Err(error) => self.message = error,
                            }
                        }
                        let can_scan = matches!(
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
                        ui.add_space(self.metrics.spacing);
                        ui.separator();
                        ui.add_space(self.metrics.spacing);
                        ui.label(
                            egui::RichText::new(self.language.tr("Bluetooth", "蓝牙"))
                                .color(SECONDARY),
                        );
                        let status = format!(
                            "● {}",
                            session_state_text(self.store.state.session, self.language)
                        );
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
                ui.horizontal_wrapped(|ui| {
                    self.layer_button(ui, Page::Mixer, self.language.tr("MIXER", "混音器"), YELLOW);
                    self.layer_button(ui, Page::Stage, self.language.tr("STAGE", "舞台"), YELLOW);
                    ui.separator();
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
                });
            });
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0, BORDER),
        );
    }

    fn layer_button(&mut self, ui: &mut egui::Ui, page: Page, label: &str, accent: Color32) {
        let destination = match page {
            Page::Fx1 => Some(MixDestination::Fx1),
            Page::Fx2 => Some(MixDestination::Fx2),
            Page::Monitor1 => Some(MixDestination::Monitor1),
            Page::Monitor2 => Some(MixDestination::Monitor2),
            Page::Main => Some(MixDestination::Main),
            _ => None,
        };
        let is_mix_view = matches!(
            self.page,
            Page::Mixer | Page::Main | Page::Monitor1 | Page::Monitor2 | Page::Fx1 | Page::Fx2
        );
        let selected = match destination {
            Some(destination) => {
                (is_mix_view || self.page == Page::Stage)
                    && self.store.state.selected_destination == destination
            }
            None if page == Page::Mixer => is_mix_view,
            None => self.page == page,
        };
        if small_tab(ui, selected, label, accent).clicked() {
            if destination.is_none() || self.page != Page::Stage {
                self.page = page;
            }
            if let Some(destination) = destination {
                if self.store.state.selected_destination != destination {
                    self.store.state.selected_destination = destination;
                    if self.store.state.session == SessionState::Ready {
                        self.dispatch(SemanticCommand::RequestChannelState {
                            target: ChannelStateTarget::Destination(destination),
                        });
                    }
                }
            }
        }
    }

    fn mixer(&mut self, ui: &mut egui::Ui) {
        // Keep frame strokes, meter labels and scroll bars away from the window edge.
        egui::Frame::new().inner_margin(12).show(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
            self.mixer_contents(ui);
        });
    }

    fn mixer_contents(&mut self, ui: &mut egui::Ui) {
        let metrics = self.metrics;
        ui.add_space(metrics.spacing);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} → {}",
                    self.language.tr("INPUTS", "输入"),
                    destination_name(self.store.state.selected_destination),
                ))
                .strong()
                .color(YELLOW),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.toggle_value(
                    &mut self.inspector_open,
                    self.language.tr("Details", "详情"),
                );
            });
        });
        ui.add_space(metrics.spacing);

        // Reserve space for the existing status line. Each column owns its scrolling.
        let available = Vec2::new(
            ui.available_width(),
            (ui.available_height() - 40.0).max(80.0),
        );
        let visible_channels = InputId::ALL
            .iter()
            .filter(|id| self.preferences.channel_visible[id.index()])
            .count();
        let layout = LayoutMetrics::calculate(available, metrics, visible_channels);
        let master_width = (166.0 * metrics.ui_scale).round_ui();
        let show_inspector = self.inspector_open;
        let inspector_width = if show_inspector {
            layout.inspector_width
        } else {
            0.0
        };
        let divider_width = (24.0 * metrics.ui_scale).round_ui();
        let gaps = divider_width * if show_inspector { 2.0 } else { 1.0 };
        let channel_area = (available.x - master_width - inspector_width - gaps)
            .max(80.0)
            .round_ui();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let strip_height = ui
                .allocate_ui_with_layout(
                    Vec2::new(channel_area, available.y),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = metrics.spacing;
                        self.render_channel_strips(
                            ui,
                            channel_area,
                            visible_channels,
                            layout.fader_height,
                        )
                    },
                )
                .inner;
            mixer_divider(ui, divider_width, available.y);
            ui.allocate_ui_with_layout(
                Vec2::new(master_width, available.y),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.x = metrics.spacing;
                    egui::ScrollArea::vertical()
                        .id_salt("master-scroll")
                        .max_width(master_width)
                        .max_height(available.y)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Frame::new().inner_margin(4).show(ui, |ui| {
                                self.destination_master(ui, layout.fader_height, strip_height);
                            });
                        });
                },
            );
            if show_inspector {
                mixer_divider(ui, divider_width, available.y);
                ui.allocate_ui_with_layout(
                    Vec2::new(inspector_width, available.y),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = metrics.spacing;
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut self.inspector_input,
                                true,
                                self.language.tr("Input", "输入详情"),
                            );
                            ui.selectable_value(
                                &mut self.inspector_input,
                                false,
                                self.language.tr("Output / FX", "输出 / FX"),
                            );
                        });
                        ui.separator();
                        let scroll_height = ui.available_height().max(40.0);
                        egui::ScrollArea::vertical()
                            .id_salt(("inspector-scroll", self.inspector_input))
                            .max_width(inspector_width)
                            .max_height(scroll_height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Frame::new().inner_margin(4).show(ui, |ui| {
                                    if self.inspector_input {
                                        if let Some(id) = self.store.state.selected_input {
                                            ui.push_id(id.index(), |ui| {
                                                self.input_inspector(ui, id)
                                            });
                                        } else {
                                            ui.label(self.language.tr(
                                                "Select an input channel to edit.",
                                                "选择输入通道以查看详情。",
                                            ));
                                        }
                                    } else {
                                        ui.push_id(
                                            (
                                                "output",
                                                self.store.state.selected_destination.index(),
                                            ),
                                            |ui| self.output_inspector(ui),
                                        );
                                    }
                                });
                            });
                    },
                );
            }
        });
    }

    fn render_channel_strips(
        &mut self,
        ui: &mut egui::Ui,
        available_width: f32,
        visible_channels: usize,
        fader_height: f32,
    ) -> f32 {
        let metrics = self.metrics;
        let mut strip_height: f32 = 0.0;
        let channels = visible_channels.max(1) as f32;
        let strip_width = ((available_width - 8.0 - metrics.spacing * (channels - 1.0)) / channels)
            .clamp(
                (144.0 * metrics.ui_scale).max(132.0),
                164.0 * metrics.ui_scale,
            )
            .round_ui();
        egui::ScrollArea::both()
            .id_salt("mixer-strips")
            .max_width(available_width)
            .max_height(ui.available_height())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new().inner_margin(4).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = metrics.spacing;
                        let settings = &self.store.state.routing.settings;
                        let usb12_on_input56 = settings
                            .input56_from_usb12
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        let usb34_on_input78 = settings
                            .input78_from_usb34
                            .effective()
                            .copied()
                            .unwrap_or(false);
                        for id in InputId::ALL {
                            if !self.preferences.channel_visible[id.index()] {
                                continue;
                            }
                            let channel = self.store.state.channels[id.index()].clone();
                            let meter_db = self.input_meter_display[id.index()].display_db;
                            let eq_modified =
                                input_eq_modified(&channel, self.eq_initial[id.index()].as_ref());
                            let selected = self.store.state.selected_input == Some(id);
                            let source_heading = match id {
                                InputId::Input56 if usb12_on_input56 => "USB 1/2",
                                InputId::Input78 if usb34_on_input78 => "USB 3/4",
                                _ => channel_number(id),
                            };
                            let strip = ui.push_id(
                                (id.index(), self.store.state.selected_destination.index()),
                                |ui| {
                                    channel_strip(
                                        ui,
                                        &channel,
                                        meter_db,
                                        eq_modified,
                                        self.store.state.selected_destination,
                                        selected,
                                        source_heading,
                                        self.language,
                                        self.preferences.show_channel_icons,
                                        self.preferences.show_mute_buttons,
                                        metrics,
                                        strip_width,
                                        fader_height,
                                    )
                                },
                            );
                            strip_height = strip_height.max(strip.response.rect.height());
                            if let Some(action) = strip.inner {
                                self.apply_strip_action(id, action);
                            }
                        }
                    });
                });
            });
        strip_height
    }

    fn apply_strip_action(&mut self, id: InputId, action: StripAction) {
        let selection_changed = self.store.state.selected_input != Some(id);
        self.store.state.selected_input = Some(id);
        match action {
            StripAction::Select => {
                self.inspector_input = true;
                self.inspector_open = true;
                if selection_changed && self.store.state.session == SessionState::Ready {
                    self.dispatch(SemanticCommand::RequestChannelState {
                        target: ChannelStateTarget::Input(id),
                    });
                }
            }
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

    fn destination_master(&mut self, ui: &mut egui::Ui, fader_height: f32, strip_height: f32) {
        let destination = self.store.state.selected_destination;
        egui::Frame::new()
            .fill(Color32::from_rgb(29, 35, 45))
            .stroke(Stroke::new(1.0, BLUE.gamma_multiply(0.7)))
            .corner_radius(8)
            .inner_margin(8)
            .show(ui, |ui| {
                // Match the input strip's actual outer height, including its footer.
                // Both frames have 8-point margins and a 1-point stroke on each edge.
                ui.set_min_height((strip_height - 18.0).max(0.0));
                strip_controls(
                    ui,
                    ui.available_width(),
                    self.metrics,
                    self.preferences.show_channel_icons,
                    |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                destination_name(destination),
                                self.language.tr("MASTER", "主控")
                            ))
                            .size(self.metrics.body_font)
                            .strong()
                            .color(BLUE),
                        );
                        ui.label(
                            egui::RichText::new(
                                self.language.tr("Destination master", "目标总线主控"),
                            )
                            .size(10.0)
                            .color(SECONDARY),
                        );
                        if ui
                            .button(self.language.tr("Output / FX", "输出 / FX"))
                            .clicked()
                        {
                            self.inspector_open = true;
                            self.inspector_input = false;
                        }
                    },
                );
                let (mut master, meter, confirmed_master) =
                    if let Some(bus) = self.store.state.bus_for_destination(destination) {
                        (
                            *bus.master_level.effective().unwrap_or(&0.75),
                            self.output_meter_display[destination.index()].display_db,
                            bus.master_level.confirmed,
                        )
                    } else {
                        let fx = &self.store.state.effects
                            [usize::from(destination == MixDestination::Fx2)];
                        (
                            *fx.master_level.effective().unwrap_or(&0.75),
                            self.output_meter_display[destination.index()].display_db,
                            fx.master_level.confirmed,
                        )
                    };
                ui.push_id(destination.index(), |ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), fader_height),
                        Layout::left_to_right(Align::Min),
                        |ui| {
                            if fader_sized(
                                ui,
                                &mut master,
                                72.0 * self.metrics.ui_scale,
                                fader_height,
                                0.75,
                                confirmed_master,
                            )
                            .on_hover_text(self.language.tr(
                                "Drag to adjust · Shift: fine adjustment · Double-click: 0 dB",
                                "拖动调节 · Shift 精调 · 双击回到 0 dB",
                            ))
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
                                42.0 * self.metrics.ui_scale,
                                fader_height,
                            );
                        },
                    )
                });
                ui.label(
                    egui::RichText::new(format!("{} dB", fader_level_text(master)))
                        .strong()
                        .color(TEXT),
                );
            });
    }

    fn output_inspector(&mut self, ui: &mut egui::Ui) {
        let destination = self.store.state.selected_destination;
        if ui
            .small_button(self.language.tr("Refresh channel", "刷新通道"))
            .clicked()
        {
            self.dispatch(SemanticCommand::RequestChannelState {
                target: ChannelStateTarget::Destination(destination),
            });
        }
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
                ui.separator();
                ui.label(
                    egui::RichText::new(self.language.tr("9-band GEQ", "9 段图示均衡")).strong(),
                );
                if let Some((index, gain_db)) = output_eq_graph(ui, &bus.eq, self.metrics) {
                    let band = &bus.eq.bands[index];
                    self.dispatch(SemanticCommand::SetGeqBand {
                        bus: bus_id,
                        band: index as u8,
                        frequency_hz: band.frequency_hz.effective().copied().unwrap_or(0.0) as u16,
                        q: band.q.effective().copied().unwrap_or(1.0),
                        gain_db,
                    });
                }
                ui.label(
                    egui::RichText::new(self.language.tr(
                        "Drag a point · Shift: fine · Double-click: 0 dB",
                        "拖动节点 · Shift 精调 · 双击归零",
                    ))
                    .size(self.metrics.small_font)
                    .color(SECONDARY),
                );
                ui.collapsing(self.language.tr("Band values", "各频段数值"), |ui| {
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
                });
                ui.add_space(6.0);
                let current_preset = effect.preset.effective().copied();
                let preset_label = current_preset
                    .and_then(|id| fx_preset_info(fx_id, id))
                    .map(|info| format!("{:02} · {}", info.id + 1, info.name))
                    .unwrap_or_else(|| match current_preset {
                        Some(id) => {
                            format!("{} {}", self.language.tr("Unknown preset", "未知预设"), id)
                        }
                        None => self
                            .language
                            .tr("No preset reported", "尚未收到预设")
                            .to_owned(),
                    });
                let mut selected_preset = None;
                ui.horizontal(|ui| {
                    ui.label(self.language.tr("Preset", "预设"));
                    ui.add_enabled_ui(self.store.state.session == SessionState::Ready, |ui| {
                        egui::ComboBox::from_id_salt(("fx-preset", index))
                            .selected_text(preset_label)
                            .width(190.0 * self.metrics.ui_scale)
                            .show_ui(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(360.0)
                                    .show(ui, |ui| {
                                        for id in 0..FX_PRESET_COUNT as u8 {
                                            let info = fx_preset_info(fx_id, id)
                                                .expect("preset ID in range");
                                            if ui
                                                .selectable_label(
                                                    current_preset == Some(id),
                                                    format!("{:02} · {}", id + 1, info.name),
                                                )
                                                .clicked()
                                            {
                                                if current_preset != Some(id) {
                                                    selected_preset = Some(id);
                                                }
                                                ui.close();
                                            }
                                        }
                                    });
                            });
                    });
                });
                if let Some(preset) = selected_preset {
                    self.dispatch(SemanticCommand::SetFxPreset { fx: fx_id, preset });
                }
                ui.label(
                    egui::RichText::new(self.language.tr(
                        "Preset labels may differ from the device.",
                        "预设名称可能与设备显示不同。",
                    ))
                    .size(9.0 * self.metrics.ui_scale)
                    .color(SECONDARY),
                );
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
            });
        }
    }

    fn input_inspector(&mut self, ui: &mut egui::Ui, id: InputId) {
        let channel = self.store.state.channels[id.index()].clone();
        card(ui, |ui| {
            if ui
                .small_button(self.language.tr("Refresh channel", "刷新通道"))
                .clicked()
            {
                self.dispatch(SemanticCommand::RequestChannelState {
                    target: ChannelStateTarget::Input(id),
                });
            }
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
                if ui
                    .add(egui::TextEdit::singleline(&mut label).desired_width(ui.available_width()))
                    .changed()
                {
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
            ui.label(
                egui::RichText::new(format!(
                    "{} → {}",
                    display_name(id, self.language),
                    destination_name(destination)
                ))
                .color(TEXT),
            );
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width =
                    (ui.available_width() - 80.0 * self.metrics.ui_scale).max(40.0);
                let response = ui.add(
                    egui::Slider::new(&mut route, specs::ROUTE_LEVEL.min..=specs::ROUTE_LEVEL.max)
                        .show_value(false),
                );
                ui.label(
                    egui::RichText::new(format!("{} dB", fader_level_text(route)))
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
                    if state_button(ui, enabled, "48 V", RED).clicked() {
                        if enabled {
                            self.pending_confirmation = None;
                            self.dispatch(SemanticCommand::SetPhantom {
                                input: id,
                                enabled: false,
                            });
                        } else {
                            self.request_confirmation(ConfirmationAction::EnablePhantom(id));
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
                    if self.store.state.selected_destination != destination {
                        self.store.state.selected_destination = destination;
                        if self.store.state.session == SessionState::Ready {
                            self.dispatch(SemanticCommand::RequestChannelState {
                                target: ChannelStateTarget::Destination(destination),
                            });
                        }
                    }
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
                let selection_changed = self.store.state.selected_input != Some(id);
                self.store.state.selected_input = Some(id);
                if selection_changed && self.store.state.session == SessionState::Ready {
                    self.dispatch(SemanticCommand::RequestChannelState {
                        target: ChannelStateTarget::Input(id),
                    });
                }
            }
            let channel = self.store.state.channels[index].clone();
            let route = channel.route_levels[self.store.state.selected_destination.index()]
                .effective()
                .copied()
                .unwrap_or(0.0);
            let meter = self.input_meter_display[index].display_db;
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
                fader_level_text(route),
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
        let snapshots = self.store.state.snapshots.device_slots.clone();
        let viewport_width = ui.available_width().min(ui.clip_rect().width()).max(0.0);
        let layout = SnapshotGridLayout::new(
            (viewport_width - 36.0).max(0.0),
            self.metrics.ui_scale,
            self.metrics.spacing,
            snapshots.len(),
        );
        let _ = centered_snapshot_content(ui, layout.group_width(), |ui| {
            ui.heading(
                egui::RichText::new(self.language.tr("Device Snapshots", "设备快照")).color(TEXT),
            );
            ui.label(
                egui::RichText::new(self.language.tr(
                    "These slots are stored on FLOW 8.",
                    "这些槽位保存在 FLOW 8 上。",
                ))
                .color(SECONDARY),
            );
            if ui
                .button(self.language.tr("Refresh slots", "刷新槽位"))
                .clicked()
            {
                self.dispatch(SemanticCommand::RequestSnapshotNames);
            }
            ui.add_space(10.0);
            for row in snapshots.chunks(layout.columns) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = layout.gap;
                    for snapshot in row {
                        let _ = snapshot_card(ui, layout.card_inner_width, |ui| {
                            let status = device_slot_status(snapshot);
                            let busy = snapshot.name.pending.is_some();
                            let occupied = matches!(status, DeviceSlotStatus::Occupied(_)) && !busy;
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{:02}", snapshot.slot + 1))
                                        .strong()
                                        .color(YELLOW),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    snapshot_status_badge(ui, &status, self.language);
                                });
                            });
                            let name = match &status {
                                DeviceSlotStatus::Occupied(name) => Some(name.as_str()),
                                _ => None,
                            };
                            snapshot_name_row(
                                ui,
                                name,
                                busy,
                                layout.card_inner_width,
                                self.metrics.body_font,
                                self.language,
                            );
                            let button_size =
                                Vec2::new(layout.button_width, self.metrics.control_height);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = layout.gap;
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Load", "载入"),
                                    occupied,
                                    button_size,
                                )
                                .clicked()
                                {
                                    self.request_confirmation(
                                        ConfirmationAction::LoadDeviceSnapshot {
                                            slot: snapshot.slot,
                                            observed_name: snapshot.name.confirmed.clone(),
                                        },
                                    );
                                }
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Save", "保存"),
                                    !busy,
                                    button_size,
                                )
                                .clicked()
                                {
                                    let name = match &status {
                                        DeviceSlotStatus::Occupied(name) => name.clone(),
                                        _ => format!("Snapshot {:02}", snapshot.slot + 1),
                                    };
                                    self.request_confirmation(
                                        ConfirmationAction::SaveDeviceSnapshot {
                                            slot: snapshot.slot,
                                            name,
                                            observed_name: snapshot.name.confirmed.clone(),
                                        },
                                    );
                                }
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Delete", "删除"),
                                    occupied,
                                    button_size,
                                )
                                .clicked()
                                {
                                    self.request_confirmation(
                                        ConfirmationAction::DeleteDeviceSnapshot {
                                            slot: snapshot.slot,
                                            observed_name: snapshot.name.confirmed.clone(),
                                        },
                                    );
                                }
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Rename", "重命名"),
                                    occupied,
                                    button_size,
                                )
                                .clicked()
                                {
                                    if let DeviceSlotStatus::Occupied(name) = &status {
                                        self.request_confirmation(
                                            ConfirmationAction::RenameDeviceSnapshot {
                                                slot: snapshot.slot,
                                                name: name.clone(),
                                                observed_name: snapshot.name.confirmed.clone(),
                                            },
                                        );
                                    }
                                }
                            });
                        });
                    }
                });
                ui.add_space(layout.gap);
            }
            ui.add_space(10.0);
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
                                egui::RichText::new(self.language.tr("Preferences", "偏好设置"))
                                    .color(TEXT),
                            );
                            ui.label(
                                egui::RichText::new(
                                    self.language
                                        .tr("Application and device settings", "应用与设备设置"),
                                )
                                .color(SECONDARY),
                            );
                            ui.add_space(10.0);
                            let settings = self.store.state.routing.settings.clone();
                            let grid = SettingsGridLayout::calculate(ui, content_width, self.metrics, self.language);

                            settings_grid_row(self, ui, grid, 0, |this, ui| {
                                settings_card(ui, grid, |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            this.language.tr("APPLICATION", "应用"),
                                        )
                                        .strong()
                                        .color(BLUE),
                                    );
                                    settings_form_row(ui, grid, this.language.tr("Language", "语言"), |ui| {
                                        if ui
                                            .selectable_label(
                                                this.language == Language::English,
                                                "English",
                                            )
                                            .clicked()
                                        {
                                            this.language = Language::English;
                                            this.message = localized_status_message(
                                                this.store.state.session,
                                                this.language,
                                            );
                                            ui.ctx().request_repaint();
                                        }
                                        if ui
                                            .selectable_label(
                                                this.language == Language::Chinese,
                                                "简体中文",
                                            )
                                            .clicked()
                                        {
                                            this.language = Language::Chinese;
                                            this.message = localized_status_message(
                                                this.store.state.session,
                                                this.language,
                                            );
                                            ui.ctx().request_repaint();
                                        }
                                    });
                                    settings_form_row(ui, grid, this.language.tr("UI Scale", "界面缩放"), |ui| {
                                        let response = ui.add(
                                            egui::Slider::new(
                                                &mut this.preferences.ui_scale,
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
                                    ui.add_enabled_ui(false, |ui| {
                                        settings_form_row(ui, grid, this.language.tr("Control Gesture", "控制手势"), |ui| {
                                            for (mode, label) in [
                                                (
                                                    ControlGesture::Linear,
                                                    this.language.tr("Linear", "线性"),
                                                ),
                                                (
                                                    ControlGesture::Rotary,
                                                    this.language.tr("Rotary", "旋转"),
                                                ),
                                            ] {
                                                let _ = ui.selectable_label(
                                                    this.preferences.control_gesture == mode,
                                                    label,
                                                );
                                            }
                                        });
                                        settings_form_row(ui, grid, this.language.tr("EQ Editing Mode", "EQ 编辑模式"), |ui| {
                                            for (mode, label) in [
                                                (
                                                    EqEditingMode::Standard,
                                                    this.language.tr("Standard", "标准"),
                                                ),
                                                (
                                                    EqEditingMode::Parametric,
                                                    this.language.tr("Parametric", "参数"),
                                                ),
                                            ] {
                                                let _ = ui.selectable_label(
                                                    this.preferences.eq_editing_mode == mode,
                                                    label,
                                                );
                                            }
                                        });
                                    });
                                    ui.separator();
                                    ui.label(
                                        egui::RichText::new(this.language.tr("CONNECTION", "连接"))
                                            .strong()
                                            .color(BLUE),
                                    );
                                    settings_form_row(ui, grid, this.language.tr("Mode", "模式"), |ui| {
                                        ui.label(this.language.tr("Bluetooth", "蓝牙"));
                                    });
                                    settings_form_row(ui, grid, this.language.tr("Status", "状态"), |ui| {
                                        ui.label(session_state_text(this.store.state.session, this.language));
                                    });
                                    if this.store.state.session != SessionState::Ready {
                                        ui.label(
                                            egui::RichText::new(this.language.tr(
                                                "Device settings are available when the connection is ready.",
                                                "连接就绪后可编辑设备设置。",
                                            ))
                                            .size(this.metrics.small_font)
                                            .color(SECONDARY),
                                        );
                                    }
                                    if let Some(stage) = this.native_stage {
                                        settings_form_row(ui, grid, this.language.tr("Connection progress", "连接进度"), |ui| {
                                            ui.label(native_stage_text(stage, this.language));
                                        });
                                    }
                                    if let Some(error) = &this.last_ble_error {
                                        settings_form_row(ui, grid, this.language.tr("Last connection error", "最近连接错误"), |ui| {
                                            ui.label(egui::RichText::new(error).color(RED));
                                        });
                                    }
                                });
                            }, |this, ui| {
                                settings_card(ui, grid, |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            this.language.tr("MIXER DISPLAY", "混音器显示"),
                                        )
                                        .strong()
                                        .color(YELLOW),
                                    );
                                    settings_form_row(ui, grid, this.language.tr("Show Mute Buttons", "显示静音按钮"), |ui| {
                                        ui.checkbox(&mut this.preferences.show_mute_buttons, "");
                                    });
                                    settings_form_row(ui, grid, this.language.tr("Show Channel Icons", "显示通道图标"), |ui| {
                                        ui.checkbox(&mut this.preferences.show_channel_icons, "");
                                    });
                                    settings_form_row(ui, grid, this.language.tr("Show Output Delay Indicator", "显示输出延迟指示"), |ui| {
                                        ui.checkbox(&mut this.preferences.show_output_delay_indicator, "");
                                    });
                                    ui.separator();
                                    ui.label(
                                        this.language.tr("Visible mixer inputs", "可见混音输入"),
                                    );
                                    egui::Grid::new("settings-visible-inputs")
                                        .num_columns(2)
                                        .spacing(Vec2::new(grid.gap, 4.0 * grid.scale))
                                        .show(ui, |ui| {
                                            for (index, input) in InputId::ALL.into_iter().enumerate() {
                                                ui.checkbox(
                                                    &mut this.preferences.channel_visible[input.index()],
                                                    display_name(input, this.language),
                                                );
                                                if index % 2 == 1 {
                                                    ui.end_row();
                                                }
                                            }
                                            if InputId::ALL.len() % 2 != 0 {
                                                ui.end_row();
                                            }
                                        });
                                });
                            });

                            let device_ready = self.store.state.session == SessionState::Ready;
                            ui.add_enabled_ui(device_ready, |ui| {
                                ui.add_space(grid.gap);
                                settings_grid_row(self, ui, grid, 1, |this, ui| {
                                    settings_card(ui, grid, |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                this.language.tr("DEVICE / CONTROL", "设备 / 控制"),
                                            )
                                            .strong()
                                            .color(BLUE),
                                        );
                                        let mut name = settings
                                            .device_name
                                            .effective()
                                            .cloned()
                                            .unwrap_or_else(|| "FLOW 8".into());
                                        settings_form_row(ui, grid, this.language.tr("Device Name", "设备名称"), |ui| {
                                            if ui.text_edit_singleline(&mut name).changed() {
                                                this.dispatch(SemanticCommand::SetSetting(
                                                    KnownSetting::DeviceName(name.clone()),
                                                ));
                                            }
                                        });
                                        let footswitch = settings
                                            .footswitch_fx_mode
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, this.language.tr("Footswitch Mode", "脚踏开关模式"), |ui| {
                                            for (fx_mode, label) in [
                                                (true, "FX"),
                                                (false, this.language.tr("Snapshot", "快照")),
                                            ] {
                                                if state_button(
                                                    ui,
                                                    footswitch == fx_mode,
                                                    label,
                                                    BLUE,
                                                )
                                                .clicked()
                                                {
                                                    this.dispatch(SemanticCommand::SetSetting(
                                                        KnownSetting::FootswitchFxMode(fx_mode),
                                                    ));
                                                }
                                            }
                                        });
                                        let linked = settings
                                            .device_linked_selection
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, this.language.tr("Device-linked Output Selection", "设备联动输出选择"), |ui| {
                                            if state_button(ui, linked, setting_switch_text(this.language, linked), BLUE).clicked() {
                                                this.dispatch(SemanticCommand::SetSetting(
                                                    KnownSetting::DeviceLinkedSelection(!linked),
                                                ));
                                            }
                                        });
                                    });
                                }, |this, ui| {
                                    settings_card(ui, grid, |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                this.language.tr("MONITOR ROUTING", "监听路由"),
                                            )
                                            .strong()
                                            .color(GREEN),
                                        );
                                        let current = settings
                                            .monitor_routing
                                            .effective()
                                            .copied()
                                            .unwrap_or(MonitorRoutingSource::MonitorMix);
                                        settings_form_row(ui, grid, this.language.tr("Monitor Source", "监听来源"), |ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                for (candidate, label, raw) in [
                                                    (MonitorRoutingSource::MonitorMix, "MON", 0),
                                                    (MonitorRoutingSource::Usb12, "USB 1/2", 1),
                                                    (MonitorRoutingSource::Usb34, "USB 3/4", 2),
                                                ] {
                                                    if state_button(ui, current == candidate, label, GREEN).clicked() {
                                                        this.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorRouting(raw)));
                                                    }
                                                }
                                            });
                                        });
                                        let post = settings
                                            .monitor_post_fader
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, this.language.tr("Monitor Tap", "监听取样点"), |ui| {
                                            if state_button(ui, post, if post { "POST" } else { "PRE" }, GREEN).clicked() {
                                                this.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorPostFader(!post)));
                                            }
                                        });
                                        let linked = this
                                            .store
                                            .state
                                            .routing
                                            .monitor_link
                                            .stereo_linked
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, this.language.tr("MON1/2 Stereo Link", "MON1/2 立体声链接"), |ui| {
                                            if state_button(ui, linked, setting_switch_text(this.language, linked), GREEN).clicked() {
                                                this.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorStereoLink(!linked)));
                                            }
                                        });
                                    });
                                });

                                ui.add_space(grid.gap);
                                settings_grid_row(self, ui, grid, 2, |this, ui| {
                                    settings_card(ui, grid, |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                this.language.tr("USB / HEADPHONES", "USB / 耳机"),
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
                                                this.language.tr("USB Streaming", "USB 推流"),
                                                0_u8,
                                            ),
                                            (
                                                settings
                                                    .input56_from_usb12
                                                    .effective()
                                                    .copied()
                                                    .unwrap_or(false),
                                                this.language.tr(
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
                                                this.language.tr(
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
                                                this.language.tr(
                                                    "Bluetooth / USB to Headphones Only",
                                                    "蓝牙 / USB 仅送耳机",
                                                ),
                                                3_u8,
                                            ),
                                        ] {
                                            settings_form_row(ui, grid, label, |ui| {
                                                if state_button(ui, active, setting_switch_text(this.language, active), GREEN).clicked() {
                                                    let setting = match command {
                                                        0 => KnownSetting::UsbStreaming(!active),
                                                        1 => KnownSetting::Input56FromUsb12(!active),
                                                        2 => KnownSetting::Input78FromUsb34(!active),
                                                        _ => KnownSetting::PhonesOnly(!active),
                                                    };
                                                    this.dispatch(SemanticCommand::SetSetting(setting));
                                                }
                                            });
                                        }
                                        let bt_usb_switch = settings
                                            .bt_usb_switch
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, this.language.tr("Bluetooth / USB source switch", "蓝牙 / USB 信号源切换"), |ui| {
                                            if state_button(ui, bt_usb_switch, setting_switch_text(this.language, bt_usb_switch), GREEN).clicked() {
                                                this.dispatch(SemanticCommand::SetSetting(KnownSetting::BtUsbSwitch(!bt_usb_switch)));
                                            }
                                        });
                                        let headphone_source = this
                                            .store
                                            .state
                                            .routing
                                            .headphones
                                            .source
                                            .effective()
                                            .copied()
                                            .unwrap_or(HeadphoneSource::Main);
                                        settings_form_row(ui, grid, this.language.tr("Headphone Source", "耳机信号源"), |ui| {
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
                                                    this.dispatch(SemanticCommand::SetSetting(
                                                        KnownSetting::HeadphonesUseMonitor(
                                                            source == HeadphoneSource::Monitor,
                                                        ),
                                                    ));
                                                }
                                            }
                                        });
                                        let headphone_tap = this
                                            .store
                                            .state
                                            .routing
                                            .headphones
                                            .tap_point
                                            .effective()
                                            .copied()
                                            .unwrap_or(TapPoint::PostFader);
                                        settings_form_row(ui, grid, this.language.tr("Headphone Tap", "耳机取样点"), |ui| {
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
                                                    this.dispatch(SemanticCommand::SetSetting(
                                                        KnownSetting::HeadphonesPostFader(
                                                            tap == TapPoint::PostFader,
                                                        ),
                                                    ));
                                                }
                                            }
                                        });
                                    });
                                }, |this, ui| {
                                    settings_card(ui, grid, |ui| {
                                        ui.label(
                                            egui::RichText::new(this.language.tr("OUTPUT", "输出"))
                                                .strong()
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            egui::RichText::new(this.language.tr("OUTPUT LEVEL", "输出电平"))
                                                .size(this.metrics.small_font)
                                                .color(SECONDARY),
                                        );
                                        let main_pad = settings
                                            .main_minus_10_dbv
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, "MAIN -10 dBV", |ui| {
                                            if state_button(ui, main_pad, setting_switch_text(this.language, main_pad), TEXT).clicked() {
                                                this.dispatch(SemanticCommand::SetSetting(KnownSetting::MainMinus10Dbv(!main_pad)));
                                            }
                                        });
                                        let monitor_pad = settings
                                            .monitor_minus_10_dbv
                                            .effective()
                                            .copied()
                                            .unwrap_or(false);
                                        settings_form_row(ui, grid, "MON -10 dBV", |ui| {
                                            if state_button(ui, monitor_pad, setting_switch_text(this.language, monitor_pad), TEXT).clicked() {
                                                this.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorMinus10Dbv(!monitor_pad)));
                                            }
                                        });
                                        ui.separator();
                                        this.output_delay_settings(ui, grid);
                                    });
                                });
                            });

                            if !self.discovered_devices.is_empty() {
                                ui.add_space(grid.gap);
                                card(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            self.language.tr("Last BLE Scan", "最近一次 BLE 扫描"),
                                        )
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

    fn output_delay_settings(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        ui.label(
            egui::RichText::new(self.language.tr("OUTPUT DELAY", "输出延迟"))
                .strong()
                .color(TEXT),
        );
        for (index, bus, destination) in [
            (0, MixBusId::Main, MixDestination::Main),
            (1, MixBusId::Monitor1, MixDestination::Monitor1),
            (2, MixBusId::Monitor2, MixDestination::Monitor2),
        ] {
            let delay = &self
                .store
                .state
                .bus_for_destination(destination)
                .expect("MAIN and MON outputs have bus state")
                .delay_ticks;
            let current_ticks = delay.effective().copied();
            let confirmed_ticks = delay.confirmed;
            if !self.delay_ms_dirty[index] {
                self.delay_ms_draft[index] = current_ticks.map(format_delay_ms).unwrap_or_default();
            }
            settings_form_row(ui, grid, destination_name(destination), |ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.delay_ms_draft[index])
                            .id_salt(("output-delay-ms", index))
                            .desired_width(100.0 * self.metrics.ui_scale),
                    )
                    .changed()
                {
                    self.delay_ms_dirty[index] = true;
                }
                ui.label("ms");
                if ui
                    .add_enabled(
                        self.delay_ms_dirty[index],
                        egui::Button::new(self.language.tr("Apply", "应用")),
                    )
                    .clicked()
                {
                    match parse_delay_ms(&self.delay_ms_draft[index]) {
                        Some(ticks) => {
                            if current_ticks != Some(ticks) {
                                self.dispatch(SemanticCommand::SetDelay { bus, ticks });
                            }
                            self.delay_ms_dirty[index] = false;
                        }
                        None => {
                            self.message = self
                                .language
                                .tr(
                                    "Enter a valid non-negative delay.",
                                    "请输入有效的非负延迟值。",
                                )
                                .into();
                        }
                    }
                }
            });
            if let Some(ticks) = confirmed_ticks {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {} ms · {} ticks",
                        self.language.tr("Device reported", "设备回报"),
                        format_delay_ms(ticks),
                        ticks,
                    ))
                    .size(self.metrics.small_font)
                    .color(SECONDARY),
                );
            }
        }
    }

    fn main_out_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("main-out-topology-scroll")
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
                                    self.language.tr("Audio routing", "音频路由拓扑"),
                                )
                                .color(TEXT),
                            );
                            ui.label(
                                egui::RichText::new(self.language.tr(
                                    "Input channels → mix buses → outputs",
                                    "输入通道 → 混音总线 → 输出",
                                ))
                                .color(SECONDARY),
                            );
                            ui.add_space(12.0);
                            card(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("CHANNEL SENDS", "通道发送"),
                                    )
                                    .strong()
                                    .color(BLUE),
                                );
                                ui.add_space(8.0);
                                egui::ScrollArea::horizontal()
                                    .id_salt("main-out-channel-sends")
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        let row_height = 30.0 * self.metrics.ui_scale;
                                        let input_width = 200.0 * self.metrics.ui_scale;
                                        let bus_width = 88.0 * self.metrics.ui_scale;
                                        egui::Grid::new("main-out-send-matrix")
                                            .num_columns(6)
                                            .spacing(Vec2::new(
                                                8.0 * self.metrics.ui_scale,
                                                4.0 * self.metrics.ui_scale,
                                            ))
                                            .show(ui, |ui| {
                                                ui.add_sized(
                                                    [input_width, row_height],
                                                    egui::Label::new(
                                                        self.language.tr("INPUT", "输入通道"),
                                                    ),
                                                );
                                                for destination in MixDestination::ALL {
                                                    ui.add_sized(
                                                        [bus_width, row_height],
                                                        egui::Label::new(
                                                            egui::RichText::new(destination_name(
                                                                destination,
                                                            ))
                                                            .strong()
                                                            .color(topology_bus_color(destination)),
                                                        ),
                                                    );
                                                }
                                                ui.end_row();
                                                let settings = &self.store.state.routing.settings;
                                                for channel in &self.store.state.channels {
                                                    let source = match channel.id {
                                                        InputId::Input56
                                                            if settings
                                                                .input56_from_usb12
                                                                .effective()
                                                                .copied()
                                                                .unwrap_or(false) =>
                                                        {
                                                            "USB 1/2 → CH 5/6"
                                                        }
                                                        InputId::Input78
                                                            if settings
                                                                .input78_from_usb34
                                                                .effective()
                                                                .copied()
                                                                .unwrap_or(false) =>
                                                        {
                                                            "USB 3/4 → CH 7/8"
                                                        }
                                                        _ => channel_number(channel.id),
                                                    };
                                                    let name = channel
                                                        .name
                                                        .effective()
                                                        .map(String::as_str)
                                                        .filter(|name| !name.is_empty())
                                                        .unwrap_or_else(|| {
                                                            display_name(channel.id, self.language)
                                                        });
                                                    let title = format!("{source} · {name}");
                                                    ui.add_sized(
                                                        [input_width, row_height],
                                                        egui::Label::new(
                                                            egui::RichText::new(&title).color(TEXT),
                                                        )
                                                        .truncate(),
                                                    )
                                                    .on_hover_text(title);
                                                    for destination in MixDestination::ALL {
                                                        let level = channel.route_levels
                                                            [destination.index()]
                                                        .effective()
                                                        .copied();
                                                        let (text, color) = match level {
                                                            Some(value) if value > 0.0 => (
                                                                format!(
                                                                    "{} dB",
                                                                    fader_level_text(value)
                                                                ),
                                                                topology_bus_color(destination),
                                                            ),
                                                            Some(_) => ("−∞".to_owned(), SECONDARY),
                                                            None => ("—".to_owned(), SECONDARY),
                                                        };
                                                        ui.add_sized(
                                                            [bus_width, row_height],
                                                            egui::Label::new(
                                                                egui::RichText::new(text)
                                                                    .color(color),
                                                            ),
                                                        );
                                                    }
                                                    ui.end_row();
                                                }
                                            });
                                    });
                            });
                            ui.add_space(12.0);
                            card(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.language.tr("BUS OUTPUTS", "总线输出"),
                                    )
                                    .strong()
                                    .color(GREEN),
                                );
                                egui::Grid::new("main-out-bus-sinks")
                                    .num_columns(3)
                                    .spacing(Vec2::new(
                                        14.0 * self.metrics.ui_scale,
                                        7.0 * self.metrics.ui_scale,
                                    ))
                                    .show(ui, |ui| {
                                        for (destination, sink) in [
                                            (MixDestination::Main, "MAIN OUT"),
                                            (MixDestination::Monitor1, "MONITOR OUT 1"),
                                            (MixDestination::Monitor2, "MONITOR OUT 2"),
                                        ] {
                                            ui.label(
                                                egui::RichText::new(destination_name(destination))
                                                    .strong()
                                                    .color(topology_bus_color(destination)),
                                            );
                                            ui.label("→");
                                            let bus = self
                                                .store
                                                .state
                                                .bus_for_destination(destination)
                                                .expect("MAIN and MON destinations have a bus");
                                            let level = bus
                                                .master_level
                                                .effective()
                                                .copied()
                                                .map(|value| {
                                                    format!("{} dB", fader_level_text(value))
                                                })
                                                .unwrap_or_else(|| "—".to_owned());
                                            let muted =
                                                bus.muted.effective().copied().unwrap_or(false);
                                            let status = if muted {
                                                format!(
                                                    "{sink} · {level} · {}",
                                                    self.language.tr("Muted", "已静音")
                                                )
                                            } else {
                                                format!("{sink} · {level}")
                                            };
                                            ui.label(status);
                                            ui.end_row();
                                        }
                                        for effect in &self.store.state.effects {
                                            let source = match effect.id {
                                                FxId::Fx1 => "FX1",
                                                FxId::Fx2 => "FX2",
                                            };
                                            ui.label(
                                                egui::RichText::new(source).strong().color(PURPLE),
                                            );
                                            ui.label("→");
                                            let returns = [
                                                (
                                                    effect.return_to_main.effective().copied(),
                                                    "MAIN",
                                                ),
                                                (
                                                    effect.return_to_mon1.effective().copied(),
                                                    "MON1",
                                                ),
                                                (
                                                    effect.return_to_mon2.effective().copied(),
                                                    "MON2",
                                                ),
                                            ];
                                            let destinations: Vec<_> = returns
                                                .iter()
                                                .filter_map(|(enabled, bus)| {
                                                    (*enabled == Some(true)).then_some(*bus)
                                                })
                                                .collect();
                                            let text = if !destinations.is_empty() {
                                                destinations.join(" · ")
                                            } else if returns
                                                .iter()
                                                .any(|(enabled, _)| enabled.is_none())
                                            {
                                                "—".to_owned()
                                            } else {
                                                self.language
                                                    .tr("No return", "未设置返回")
                                                    .to_owned()
                                            };
                                            ui.label(text);
                                            ui.end_row();
                                        }
                                        ui.label(
                                            egui::RichText::new(
                                                self.language.tr("HEADPHONES", "耳机"),
                                            )
                                            .strong()
                                            .color(GREEN),
                                        );
                                        ui.label("←");
                                        let headphones = &self.store.state.routing.headphones;
                                        let source = headphones
                                            .source
                                            .effective()
                                            .copied()
                                            .map(|source| {
                                                headphone_source_text(source, self.language)
                                            })
                                            .unwrap_or("—");
                                        let tap = headphones
                                            .tap_point
                                            .effective()
                                            .copied()
                                            .map(|tap| tap_point_text(tap, self.language))
                                            .unwrap_or("—");
                                        ui.label(format!("{source} · {tap}"));
                                        ui.end_row();
                                    });
                            });
                        },
                    );
                });
            });
    }

    fn routing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.heading(egui::RichText::new(self.language.tr("Routing", "路由")).color(TEXT));
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
        self.advance_meter_display();
        self.clear_stale_confirmation();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));

        ui.set_min_size(ui.available_size());
        ui.painter().rect_filled(ui.max_rect(), 0.0, BG);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            self.connection_bar(ui);
            self.layer_bar(ui);
            self.clear_stale_confirmation();
            if self.store.state.session == SessionState::Ready && self.page == Page::Snapshots {
                if !self.snapshot_names_requested {
                    self.dispatch(SemanticCommand::RequestSnapshotNames);
                    self.snapshot_names_requested = true;
                }
            } else {
                self.snapshot_names_requested = false;
            }
            ui.spacing_mut().item_spacing = Vec2::splat(self.metrics.spacing);
            let device_ready = self.store.state.session == SessionState::Ready;
            if !device_ready && self.page != Page::Settings {
                device_unsynced_notice(ui, self.store.state.session, self.language);
            }
            match self.page {
                Page::Mixer
                | Page::Fx1
                | Page::Fx2
                | Page::Monitor1
                | Page::Monitor2
                | Page::Main => {
                    ui.add_enabled_ui(device_ready, |ui| self.mixer(ui));
                }
                Page::Stage => {
                    ui.add_enabled_ui(device_ready, |ui| self.stage_page(ui));
                }
                Page::MainOut => {
                    egui::ScrollArea::vertical()
                        .id_salt("main-out-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_enabled_ui(device_ready, |ui| self.main_out_page(ui));
                        });
                }
                Page::Routing => {
                    egui::ScrollArea::vertical()
                        .id_salt("routing-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_enabled_ui(device_ready, |ui| self.routing_page(ui));
                        });
                }
                Page::Snapshots => {
                    egui::ScrollArea::vertical()
                        .id_salt("snapshots-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_enabled_ui(device_ready, |ui| self.snapshots_page(ui));
                        });
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
        self.confirmation_dialog(ui.ctx());
        // Request the APK-defined meter sources once after Ready and when the
        // displayed destination changes. 0x38 itself contains no meter values.
        self.sync_channel_labels_request();
        self.sync_meter_request();
        // Send this frame's intents now, rather than waiting for the next repaint.
        // The Store's semantic queue still coalesces continuous edits within the frame.
        self.flush_commands();
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

fn mixer_divider(ui: &mut egui::Ui, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, Color32::from_rgb(78, 88, 103)),
    );
}

fn strip_controls_height(metrics: UiMetrics, show_icons: bool) -> f32 {
    ((if show_icons { 182.0 } else { 152.0 }) * metrics.ui_scale).round_ui()
}

fn strip_controls(
    ui: &mut egui::Ui,
    width: f32,
    metrics: UiMetrics,
    show_icons: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    // Allocate the same fixed header for inputs and master. Unlike a minimum
    // height, this cannot move the fader endpoints when header contents differ.
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, strip_controls_height(metrics, show_icons)).round_ui(),
        Sense::hover(),
    );
    // Use the same aligned bounds for child layout and clipping.
    let rect = rect.round_ui();
    let mut controls = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("strip-controls")
            .max_rect(rect)
            .layout(Layout::top_down(Align::Center)),
    );
    controls.set_clip_rect(ui.clip_rect().intersect(rect));
    add(&mut controls);
}

fn channel_strip(
    ui: &mut egui::Ui,
    channel: &InputChannelState,
    meter_db: f32,
    eq_modified: bool,
    destination: MixDestination,
    selected: bool,
    source_heading: &'static str,
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
        .stroke(Stroke::new(1.0, if selected { YELLOW } else { BORDER }))
        .corner_radius(8)
        .inner_margin(8)
        .show(ui, |ui| {
            // strip_width includes the frame margin and stroke.
            let content_width = (strip_width - 18.0).max(100.0).round_ui();
            ui.set_width(content_width);
            ui.vertical_centered(|ui| {
                strip_controls(ui, content_width, metrics, show_channel_icons, |ui| {
                    ui.label(
                        egui::RichText::new(source_heading)
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
                    let name = channel
                        .name
                        .effective()
                        .map(String::as_str)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| display_name(channel.id, language));
                    if ui
                        .add_sized(
                            [content_width, metrics.control_height],
                            egui::Button::new(
                                egui::RichText::new(name)
                                    .size(metrics.body_font)
                                    .strong()
                                    .color(TEXT),
                            )
                            .selected(selected)
                            .truncate(),
                        )
                        .on_hover_text(name)
                        .clicked()
                    {
                        action = Some(StripAction::Select);
                    }
                    let phantom = channel.capabilities.phantom
                        && *channel.phantom_48v.effective().unwrap_or(&false);
                    let status = if phantom {
                        "48 V"
                    } else if channel.id.is_stereo() {
                        language.tr("STEREO", "立体声")
                    } else {
                        " "
                    };
                    ui.label(
                        egui::RichText::new(status)
                            .size(metrics.small_font)
                            .color(if phantom { RED } else { BLUE }),
                    );
                    ui.add_space(4.0);
                    let mut pan = *channel.pan.effective().unwrap_or(&0.0);
                    let pan_response = pan_control(
                        ui,
                        &mut pan,
                        (content_width - 14.0 * metrics.ui_scale).max(72.0),
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
                });
                let route_state = &channel.route_levels[destination.index()];
                let mut route = *route_state.effective().unwrap_or(&0.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(content_width, fader_height),
                    Layout::left_to_right(Align::Min),
                    |ui| {
                        let fader_width = (content_width * 0.65)
                            .clamp(68.0, 82.0)
                            .min(content_width - metrics.spacing - 20.0)
                            .round_ui();
                        let meter_width = (content_width - fader_width - metrics.spacing).max(20.0);
                        let changed = fader_sized(
                            ui,
                            &mut route,
                            fader_width,
                            fader_height,
                            0.75,
                            route_state.confirmed,
                        )
                        .on_hover_text(language.tr(
                            "Drag to adjust · Shift: fine adjustment · Double-click: 0 dB",
                            "拖动调节 · Shift 精调 · 双击回到 0 dB",
                        ))
                        .changed();
                        meter_widget_sized(ui, meter_db, meter_width, fader_height);
                        if changed {
                            action = Some(StripAction::Route(route));
                        }
                    },
                );
                ui.label(
                    egui::RichText::new(format!("{} dB", fader_level_text(route)))
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
                    badge(ui, "EQ", eq_modified);
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

const FADER_CONFIRM_GRACE_SECONDS: f64 = 3.0;

// A gesture is editing intent, not another copy of device state. The confirmed
// Store remains authoritative once the device catches up or the grace expires.
#[derive(Clone, Copy)]
struct FaderGesture {
    target: f32,
    last_sent: f32,
    last_pointer_y: f32,
    grab_offset_y: f32,
    fine: bool,
    released_at: Option<f64>,
}

impl FaderGesture {
    fn begin(value: f32, pointer_y: f32, track: Rect, on_thumb: bool, fine: bool) -> Self {
        let grab_offset_y = if on_thumb {
            fader_thumb_y(track, value) - pointer_y
        } else {
            0.0
        };
        Self {
            target: if on_thumb {
                value
            } else {
                fader_value_at_y(track, pointer_y)
            },
            last_sent: value,
            last_pointer_y: pointer_y,
            grab_offset_y,
            fine,
            released_at: None,
        }
    }

    fn update(&mut self, pointer_y: f32, track: Rect, fine: bool) -> bool {
        if self.fine != fine {
            // Switching precision must not move the thumb on its own.
            self.grab_offset_y = fader_thumb_y(track, self.target) - pointer_y;
        }
        self.target = if fine {
            (self.target - (pointer_y - self.last_pointer_y) / track.height() * 0.1).clamp(0.0, 1.0)
        } else {
            fader_value_at_y(track, pointer_y + self.grab_offset_y)
        };
        self.last_pointer_y = pointer_y;
        self.fine = fine;
        if (self.target - self.last_sent).abs() > 1.0e-4 {
            self.last_sent = self.target;
            true
        } else {
            false
        }
    }

    fn released(value: f32, time: f64) -> Self {
        Self {
            target: value,
            last_sent: value,
            last_pointer_y: 0.0,
            grab_offset_y: 0.0,
            fine: false,
            released_at: Some(time),
        }
    }

    fn awaiting_confirmation(&self, confirmed: Option<f32>, now: f64) -> bool {
        let released_at = self.released_at.unwrap_or(now);
        let confirmed_target =
            confirmed.is_some_and(|device_value| (device_value - self.target).abs() <= 0.01);
        !confirmed_target && now - released_at < FADER_CONFIRM_GRACE_SECONDS
    }
}

// Reserve a small bottom detent for mute, separate from finite fader levels.
const FADER_MUTE_GAP: f32 = 0.07;
const FADER_SCALE: [(f32, &str); 5] = [
    (1.0, "10"),
    (0.75, "0"),
    (0.5, "-10"),
    (0.25, "-30"),
    (0.0, "-∞"),
];

fn fader_visual_position(value: f32) -> f32 {
    if value <= 0.0 {
        0.0
    } else {
        FADER_MUTE_GAP + value.clamp(0.0, 1.0) * (1.0 - FADER_MUTE_GAP)
    }
}

fn fader_thumb_y(track: Rect, value: f32) -> f32 {
    egui::lerp(track.bottom()..=track.top(), fader_visual_position(value))
}

fn fader_value_at_y(track: Rect, y: f32) -> f32 {
    let position = ((track.bottom() - y) / track.height()).clamp(0.0, 1.0);
    if position <= FADER_MUTE_GAP {
        0.0
    } else {
        ((position - FADER_MUTE_GAP) / (1.0 - FADER_MUTE_GAP)).clamp(0.0, 1.0)
    }
}

fn fader_sized(
    ui: &mut egui::Ui,
    value: &mut f32,
    width: f32,
    height: f32,
    default: f32,
    confirmed: Option<f32>,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, height).round_ui(), Sense::click_and_drag());
    let track = Rect::from_center_size(
        Pos2::new(rect.left() + 26.0, rect.center().y),
        Vec2::new(5.0, (rect.height() - 24.0).max(1.0)),
    );
    let pointer_held =
        response.is_pointer_button_down_on() && ui.input(|input| input.pointer.primary_down());
    if response.clicked() || pointer_held {
        response.request_focus();
    }
    let drag_id = response.id.with("fader-drag");
    let time = ui.input(|input| input.time);
    if !ui.is_enabled() {
        ui.ctx()
            .data_mut(|data| data.remove::<FaderGesture>(drag_id));
    } else if pointer_held {
        if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
            let started =
                ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary));
            let previous = if started {
                None
            } else {
                ui.ctx().data(|data| data.get_temp::<FaderGesture>(drag_id))
            };
            let fine = ui.input(|input| input.modifiers.shift);
            let thumb = Rect::from_center_size(
                Pos2::new(track.center().x, fader_thumb_y(track, *value)),
                Vec2::new(29.0, 15.0),
            );
            let mut gesture = previous.unwrap_or_else(|| {
                FaderGesture::begin(
                    *value,
                    pointer.y,
                    track,
                    thumb.expand(3.0).contains(pointer),
                    fine,
                )
            });
            if gesture.update(pointer.y, track, fine) {
                response.mark_changed();
            }
            *value = gesture.target;
            ui.ctx().data_mut(|data| data.insert_temp(drag_id, gesture));
        }
    } else if let Some(mut gesture) = ui.ctx().data(|data| data.get_temp::<FaderGesture>(drag_id)) {
        gesture.released_at.get_or_insert(time);
        if gesture.awaiting_confirmation(confirmed, time) {
            // Hide an older in-flight echo briefly after release; never emit TX from it.
            *value = gesture.target;
            ui.ctx().data_mut(|data| data.insert_temp(drag_id, gesture));
        } else {
            ui.ctx()
                .data_mut(|data| data.remove::<FaderGesture>(drag_id));
        }
    }
    if response.double_clicked() && (*value - default).abs() > f32::EPSILON {
        *value = default;
        response.mark_changed();
        ui.ctx()
            .data_mut(|data| data.insert_temp(drag_id, FaderGesture::released(default, time)));
    }
    // Wheel input belongs to the scroll area, never to a merely hovered fader.
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
        let sensitivity = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
        let next = (*value + delta * sensitivity).clamp(0.0, 1.0);
        if (*value - next).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
            if !pointer_held {
                ui.ctx()
                    .data_mut(|data| data.insert_temp(drag_id, FaderGesture::released(next, time)));
            }
        }
    }
    let cursor = if pointer_held {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response = response.on_hover_cursor(cursor);
    let painter = ui.painter();
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            4.0,
            Stroke::new(1.0, BLUE),
            StrokeKind::Inside,
        );
    }
    painter.rect_filled(track, 2.0, Color32::from_rgb(52, 57, 67));
    let fill_top = fader_thumb_y(track, *value);
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(track.left(), fill_top), track.right_bottom()),
        2.0,
        YELLOW,
    );
    for (level, label) in FADER_SCALE {
        let y = fader_thumb_y(track, level);
        painter.line_segment(
            [
                Pos2::new(track.right() + 3.0, y),
                Pos2::new(track.right() + 7.0, y),
            ],
            Stroke::new(1.0, SECONDARY),
        );
        painter.text(
            Pos2::new(track.right() + 12.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(10.0),
            SECONDARY,
        );
    }
    let thumb_y = fader_thumb_y(track, *value);
    let thumb = Rect::from_center_size(Pos2::new(track.center().x, thumb_y), Vec2::new(29.0, 15.0));
    painter.rect_filled(
        thumb,
        4.0,
        if pointer_held {
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

fn meter_widget_sized(ui: &mut egui::Ui, db: f32, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height).round_ui(), Sense::hover());
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
}

fn pan_control(ui: &mut egui::Ui, value: &mut f32, width: f32, metrics: UiMetrics) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(
        Vec2::new(width, 20.0 * metrics.ui_scale).round_ui(),
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

struct ParameterRowResponse {
    user_changed: bool,
}

impl ParameterRowResponse {
    fn changed(&self) -> bool {
        self.user_changed
    }
}

fn parameter_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    spec: ParameterSpec,
) -> ParameterRowResponse {
    let original = *value;
    let scale = ui.spacing().interact_size.y / 30.0;
    let stacked = ui.available_width() < 330.0 * scale;
    let response = ui
        .vertical(|ui| {
            if stacked {
                ui.label(egui::RichText::new(label).color(TEXT));
            }
            // horizontal() bounds vertical centering to one control row. A bare
            // with_layout() here would center the slider in the remaining viewport.
            ui.horizontal(|ui| {
                if !stacked {
                    ui.label(egui::RichText::new(label).color(TEXT));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - 100.0 * scale).clamp(36.0, 140.0 * scale);
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
        })
        .inner;
    // egui may mark Slider::changed when merely snapping a received device
    // value to a display step. That is a rendering detail, not user intent.
    let pointer_edit =
        response.dragged() || response.clicked() || response.is_pointer_button_down_on();
    let keyboard_edit = response.has_focus()
        && ui.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Key { pressed: true, .. }
                )
            })
        });
    let wheel_edit = response.hovered() && ui.input(|input| input.smooth_scroll_delta.y != 0.0);
    let user_changed = response.changed()
        && (pointer_edit || keyboard_edit || wheel_edit)
        && (*value - original).abs() > 1.0e-6;
    if !user_changed {
        *value = original;
    }
    ParameterRowResponse { user_changed }
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

fn output_eq_graph(ui: &mut egui::Ui, eq: &EqState, metrics: UiMetrics) -> Option<(usize, f32)> {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 176.0 * metrics.ui_scale).round_ui(),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 5.0, Color32::from_rgb(13, 15, 18));
    // Inset both end points and their labels so the scroll viewport never cuts them off.
    let plot = Rect::from_min_max(
        rect.min + Vec2::new(30.0, 14.0) * metrics.ui_scale,
        rect.max - Vec2::new(18.0, 34.0) * metrics.ui_scale,
    );
    if eq.bands.is_empty() || plot.width() <= 0.0 || plot.height() <= 0.0 {
        return None;
    }
    let gain_range = specs::EQ_GAIN.max - specs::EQ_GAIN.min;
    for gain in [specs::EQ_GAIN.min, 0.0, specs::EQ_GAIN.max] {
        let y = egui::lerp(
            plot.bottom()..=plot.top(),
            (gain - specs::EQ_GAIN.min) / gain_range,
        );
        ui.painter().hline(
            plot.x_range(),
            y,
            Stroke::new(1.0, if gain == 0.0 { SECONDARY } else { BORDER }),
        );
        ui.painter().text(
            Pos2::new(plot.left() - 5.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{gain:+.0}"),
            FontId::proportional(metrics.small_font),
            SECONDARY,
        );
    }
    let steps = eq.bands.len().saturating_sub(1).max(1) as f32;
    let points: Vec<Pos2> = eq
        .bands
        .iter()
        .enumerate()
        .map(|(index, band)| {
            let gain = band.gain_db.effective().copied().unwrap_or(0.0);
            Pos2::new(
                egui::lerp(plot.left()..=plot.right(), index as f32 / steps),
                egui::lerp(
                    plot.bottom()..=plot.top(),
                    ((gain - specs::EQ_GAIN.min) / gain_range).clamp(0.0, 1.0),
                ),
            )
        })
        .collect();
    // This is a band-gain line, not an invented DSP frequency response.
    ui.painter()
        .add(egui::Shape::line(points.clone(), Stroke::new(2.0, BLUE)));
    let mut edit = None;
    for (index, (point, band)) in points.iter().zip(&eq.bands).enumerate() {
        let frequency = band.frequency_hz.effective().copied();
        let gain = band.gain_db.effective().copied().unwrap_or(0.0);
        let frequency_text = match frequency {
            Some(hz) if hz >= 1000.0 => format!("{}k", hz / 1000.0),
            Some(hz) => format!("{hz:.0}"),
            None => "—".to_owned(),
        };
        ui.painter()
            .vline(point.x, plot.y_range(), Stroke::new(1.0, BORDER));
        let label_row = if index % 2 == 0 {
            0.0
        } else {
            metrics.small_font + 2.0
        };
        ui.painter().text(
            Pos2::new(point.x, plot.bottom() + 5.0 + label_row),
            egui::Align2::CENTER_TOP,
            frequency_text,
            FontId::proportional(metrics.small_font),
            SECONDARY,
        );
        let hit_size = Vec2::splat((plot.width() / (steps + 1.0)).min(20.0 * metrics.ui_scale));
        let editable = frequency.is_some()
            && band.gain_db.effective().is_some()
            && band.q.effective().is_some();
        let response = ui
            .interact(
                Rect::from_center_size(*point, hit_size),
                ui.id().with(("output-eq-band", index)),
                if editable {
                    Sense::click_and_drag()
                } else {
                    Sense::hover()
                },
            )
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
            .on_hover_text(format!(
                "{} Hz · {gain:+.1} dB",
                frequency
                    .map(|hz| format!("{hz:.0}"))
                    .unwrap_or_else(|| "—".into())
            ));
        ui.painter().circle_filled(
            *point,
            4.0 * metrics.ui_scale,
            if response.hovered() || response.dragged() {
                Color32::WHITE
            } else {
                BLUE
            },
        );
        if response.dragged() {
            let fine = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
            let next = (gain - response.drag_delta().y / plot.height() * gain_range * fine)
                .clamp(specs::EQ_GAIN.min, specs::EQ_GAIN.max);
            if (next - gain).abs() > f32::EPSILON {
                edit = Some((index, next));
            }
        } else if response.double_clicked() && gain != 0.0 {
            edit = Some((index, 0.0));
        }
    }
    edit
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(8)
        .inner_margin(12)
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_frame().show(ui, add).inner
}

fn centered_snapshot_content<R>(
    ui: &mut egui::Ui,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.allocate_ui_with_layout(Vec2::new(width, 0.0), Layout::top_down(Align::Min), add)
    })
    .inner
}

fn snapshot_card<R>(
    ui: &mut egui::Ui,
    inner_width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_frame().show(ui, |ui| {
        ui.set_width(inner_width);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.set_width(inner_width);
            add(ui)
        })
        .inner
    })
}

fn snapshot_action_button(ui: &mut egui::Ui, label: &str, enabled: bool, size: Vec2) -> Response {
    ui.add_enabled(enabled, egui::Button::new(label).small().min_size(size))
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
fn topology_bus_color(destination: MixDestination) -> Color32 {
    match destination {
        MixDestination::Main => BLUE,
        MixDestination::Monitor1 | MixDestination::Monitor2 => GREEN,
        MixDestination::Fx1 | MixDestination::Fx2 => PURPLE,
    }
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
fn fader_level_text(value: f32) -> String {
    if value <= 0.0 {
        "−∞".into()
    } else {
        format!("{:+.1}", normalized_to_display_db(value))
    }
}

fn normalized_to_display_db(value: f32) -> f32 {
    // Presentation floor observed on the physical fader; wire conversion stays in flow8-protocol.
    flow8_protocol::normalized_to_fader_db(value.clamp(0.0, 1.0))
        .unwrap_or(-89.5)
        .max(-89.5)
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
        SessionPhase::GattReady => language.tr("Device connected", "设备已连接"),
        SessionPhase::RxArming => language.tr("Preparing device updates", "正在准备设备更新"),
        SessionPhase::Handshaking => language.tr("Completing connection", "正在完成连接"),
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
        SessionState::GattReady => language.tr("Device connected", "设备已连接"),
        SessionState::RxArming => language.tr("Preparing device updates", "正在准备设备更新"),
        SessionState::Handshaking => language.tr("Completing connection", "正在完成连接"),
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

fn device_unsynced_notice(ui: &mut egui::Ui, state: SessionState, language: Language) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(18.0);
        egui::Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(language.tr(
                        "Connect to FLOW 8 to enable controls.",
                        "连接 FLOW 8 后即可使用控制功能。",
                    ))
                    .strong(),
                );
                ui.label(language.tr(
                    "Current settings will appear after synchronization.",
                    "同步完成后将显示当前设备设置。",
                ));
                ui.label(format!(
                    "{}: {}",
                    language.tr("Status", "状态"),
                    session_state_text(state, language)
                ));
            });
    });
}

fn localized_status_message(state: SessionState, language: Language) -> String {
    format!(
        "{}: {}",
        language.tr("Bluetooth", "蓝牙"),
        session_state_text(state, language)
    )
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
    // egui includes this stroke width in Button layout on the next frame.
    // Keep it constant so hover/focus cannot move neighboring top-bar buttons.
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(130, 145, 164));
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(72, 79, 91);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, YELLOW);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(49, 55, 65);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.selection.bg_fill = BLUE.gamma_multiply(0.55);
    style.visuals.selection.stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    style.spacing.item_spacing = Vec2::splat(metrics.spacing);
    style.spacing.button_padding =
        Vec2::new(12.0 * metrics.ui_scale, 7.0 * metrics.ui_scale).round_ui();
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

fn configure_fonts(context: &egui::Context) {
    let Some((candidate, path, bytes)) = load_system_cjk_font() else {
        tracing::warn!(
            "no supported system CJK font was found; Simplified Chinese glyphs may be missing"
        );
        return;
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
    fn headless_fader_ignores_delayed_rx_during_drag_and_after_release() {
        fn render(
            context: &egui::Context,
            events: Vec<egui::Event>,
            store_value: f32,
            time: f64,
        ) -> (f32, bool, Rect) {
            let mut displayed = store_value;
            let mut changed = false;
            let mut rect = Rect::NOTHING;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
                time: Some(time),
                events,
                ..Default::default()
            };
            let mut output = context.run_ui(raw, |ui| {
                let response =
                    fader_sized(ui, &mut displayed, 50.0, 200.0, 0.75, Some(store_value));
                changed = response.changed();
                rect = response.rect;
            });
            output.textures_delta.clear();
            (displayed, changed, rect)
        }

        let context = egui::Context::default();
        let (_, _, rect) = render(&context, vec![], 0.5, 0.0);
        let thumb = Pos2::new(rect.center().x, rect.center().y);
        let press = egui::Event::PointerButton {
            pos: thumb,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        };
        let (pressed, _, _) = render(
            &context,
            vec![egui::Event::PointerMoved(thumb), press],
            0.5,
            0.1,
        );
        assert!((pressed - 0.5).abs() < 1.0e-6);

        let moved = Pos2::new(thumb.x, thumb.y - 40.0);
        let (dragged, changed, _) =
            render(&context, vec![egui::Event::PointerMoved(moved)], 0.5, 0.2);
        assert!(changed);
        assert!(dragged > 0.6);

        let (held, repeated, _) = render(&context, vec![], 0.2, 0.3);
        assert!(
            !repeated,
            "stale RX must not enqueue another identical fader TX"
        );
        assert!((held - dragged).abs() < 1.0e-6);

        let release = egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        };
        let (released, _, _) = render(&context, vec![release], 0.2, 0.4);
        assert!((released - dragged).abs() < 1.0e-6);
        let (confirmed, _, _) = render(&context, vec![], dragged, 0.5);
        assert!((confirmed - dragged).abs() < 1.0e-6);
    }

    #[test]
    fn fader_drag_tracks_pointer_without_reusing_stale_device_values() {
        let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(5.0, 200.0));
        let mut gesture = FaderGesture::begin(0.5, 100.0, track, true, false);
        assert!(!gesture.update(100.0, track, false));
        assert!(gesture.update(60.0, track, false));
        let dragged_target = fader_value_at_y(track, fader_thumb_y(track, 0.5) - 40.0);
        assert!((gesture.target - dragged_target).abs() < 1.0e-6);
        // A delayed RX might replace the Store value with 0.2. The active
        // gesture still displays the dragged target and does not enqueue another TX.
        let stale_confirmed = 0.2;
        assert!((gesture.target - stale_confirmed).abs() > 0.4);
        assert!(!gesture.update(60.0, track, false));
        assert!((gesture.target - dragged_target).abs() < 1.0e-6);
    }

    #[test]
    fn fader_track_click_and_fine_drag_preserve_pointer_position() {
        let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(5.0, 200.0));
        let mut click = FaderGesture::begin(0.25, 20.0, track, false, false);
        assert!(click.update(20.0, track, false));
        assert!((click.target - fader_value_at_y(track, 20.0)).abs() < 1.0e-6);

        let mut fine = FaderGesture::begin(0.5, 100.0, track, true, false);
        assert!(!fine.update(100.0, track, false));
        assert!(fine.update(80.0, track, true));
        assert!((fine.target - 0.51).abs() < 1.0e-6);
        assert!(!fine.update(80.0, track, false));
        assert!((fine.target - 0.51).abs() < 1.0e-6);
    }

    #[test]
    fn fader_scale_matches_physical_labels_and_thumb_positions() {
        assert_eq!(
            FADER_SCALE.map(|(_, label)| label),
            ["10", "0", "-10", "-30", "-∞"]
        );
        let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(5.0, 200.0));
        for (level, _) in FADER_SCALE {
            let y = fader_thumb_y(track, level);
            assert!((fader_value_at_y(track, y) - level).abs() < 1.0e-6);
        }
        assert!(fader_thumb_y(track, 0.001) < fader_thumb_y(track, 0.0));
        assert_eq!(fader_value_at_y(track, track.bottom() - 5.0), 0.0);
        assert_eq!(fader_level_text(0.0), "−∞");
        assert_eq!(fader_level_text(0.0001), "-89.5");
        assert_eq!(fader_level_text(1.0), "+10.0");
    }

    #[test]
    fn fader_release_reconciles_to_device_or_expires() {
        let gesture = FaderGesture::released(0.7, 10.0);
        assert!(gesture.awaiting_confirmation(Some(0.2), 10.5));
        assert!(!gesture.awaiting_confirmation(Some(0.7), 10.5));
        assert!(!gesture.awaiting_confirmation(Some(0.2), 13.1));
    }

    #[test]
    fn snapshot_slots_use_only_confirmed_device_names_for_occupancy() {
        let mut slot = Flow8Store::disconnected().state.snapshots.device_slots[0].clone();
        assert_eq!(device_slot_status(&slot), DeviceSlotStatus::Unknown);
        slot.name.set_pending("Proposed name".into());
        assert_eq!(device_slot_status(&slot), DeviceSlotStatus::Unknown);
        slot.name
            .observe(String::new(), EvidenceStatus::VerifiedFromDevice);
        assert_eq!(device_slot_status(&slot), DeviceSlotStatus::Free);
        slot.name
            .observe("Vocal".into(), EvidenceStatus::VerifiedFromDevice);
        assert_eq!(
            device_slot_status(&slot),
            DeviceSlotStatus::Occupied("Vocal".into())
        );
    }

    #[test]
    fn snapshot_grid_reflows_and_four_actions_keep_equal_nonoverlapping_widths() {
        assert_eq!(SnapshotGridLayout::new(831.0, 1.0, 8.0, 10).columns, 1);
        assert_eq!(SnapshotGridLayout::new(832.0, 1.0, 8.0, 10).columns, 2);
        let mut previous_columns = 0;
        for window_width in [1040.0, 1280.0, 1440.0, 1600.0, 1920.0, 2560.0] {
            let metrics = UiMetrics::calculate(Vec2::new(window_width, 920.0), 1.0, 1.0);
            let layout =
                SnapshotGridLayout::new(window_width - 36.0, metrics.ui_scale, metrics.spacing, 10);
            assert!(layout.columns >= previous_columns);
            previous_columns = layout.columns;
            assert!(4.0 * layout.button_width + 3.0 * layout.gap <= layout.card_inner_width + 0.1);
            for language in [Language::English, Language::Chinese] {
                let context = egui::Context::default();
                configure_style(&context, metrics);
                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(window_width, 920.0),
                    )),
                    ..Default::default()
                };
                let mut rects = Vec::new();
                let mut output = context.run_ui(raw, |ui| {
                    ui.set_width(layout.card_inner_width);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = layout.gap;
                        for (english, chinese) in [
                            ("Load", "载入"),
                            ("Save", "保存"),
                            ("Delete", "删除"),
                            ("Rename", "重命名"),
                        ] {
                            rects.push(
                                snapshot_action_button(
                                    ui,
                                    language.tr(english, chinese),
                                    true,
                                    Vec2::new(layout.button_width, metrics.control_height),
                                )
                                .rect,
                            );
                        }
                    });
                });
                output.textures_delta.clear();
                assert_eq!(rects.len(), 4);
                for pair in rects.windows(2) {
                    assert!(pair[0].right() <= pair[1].left());
                    assert!((pair[0].width() - pair[1].width()).abs() < 0.1);
                }
                assert!(rects[3].right() - rects[0].left() <= layout.card_inner_width + 0.1);
            }
        }
        assert!(previous_columns > 1);
    }

    #[test]
    fn snapshot_card_group_has_balanced_window_margins() {
        for width in [1040.0, 1280.0, 1440.0, 1600.0, 1920.0, 2560.0] {
            let metrics = UiMetrics::calculate(Vec2::new(width, 920.0), 1.0, 1.0);
            let layout =
                SnapshotGridLayout::new(width - 36.0, metrics.ui_scale, metrics.spacing, 10);
            let context = egui::Context::default();
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 920.0))),
                ..Default::default()
            };
            let mut margins = (0.0, 0.0);
            let mut output = context.run_ui(raw, |ui| {
                let viewport = ui.max_rect();
                let group = centered_snapshot_content(ui, layout.group_width(), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = layout.gap;
                        for _ in 0..layout.columns {
                            let _ = snapshot_card(ui, layout.card_inner_width, |ui| {
                                ui.label("01");
                            });
                        }
                    });
                });
                margins = (
                    group.response.rect.left() - viewport.left(),
                    viewport.right() - group.response.rect.right(),
                );
            });
            output.textures_delta.clear();
            assert!(
                (margins.0 - margins.1).abs() <= 3.0,
                "width={width}, left={}, right={}",
                margins.0,
                margins.1
            );
            assert!(margins.0 >= 16.0 && margins.1 >= 16.0);
        }
    }

    #[test]
    fn snapshot_cards_keep_status_in_header_and_reserve_name_height() {
        for language in [Language::English, Language::Chinese] {
            let context = egui::Context::default();
            let metrics = UiMetrics::calculate(Vec2::new(1040.0, 700.0), 1.0, 1.0);
            configure_style(&context, metrics);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1040.0, 700.0))),
                ..Default::default()
            };
            let mut cards = Vec::new();
            let mut output = context.run_ui(raw, |ui| {
                ui.horizontal(|ui| {
                    for status in [
                        DeviceSlotStatus::Free,
                        DeviceSlotStatus::Occupied("Vocal".into()),
                    ] {
                        let card = snapshot_card(ui, 410.0, |ui| {
                            let header = ui.horizontal(|ui| {
                                ui.label("01");
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    snapshot_status_badge(ui, &status, language);
                                });
                            });
                            let name = match &status {
                                DeviceSlotStatus::Occupied(name) => Some(name.as_str()),
                                _ => None,
                            };
                            let name = snapshot_name_row(
                                ui,
                                name,
                                false,
                                410.0,
                                metrics.body_font,
                                language,
                            );
                            let actions = ui.horizontal(|ui| {
                                for (english, chinese) in [
                                    ("Load", "载入"),
                                    ("Save", "保存"),
                                    ("Delete", "删除"),
                                    ("Rename", "重命名"),
                                ] {
                                    snapshot_action_button(
                                        ui,
                                        language.tr(english, chinese),
                                        true,
                                        Vec2::new(90.0, metrics.control_height),
                                    );
                                }
                            });
                            (header.response.rect, name.rect, actions.response.rect)
                        });
                        cards.push((card.response.rect, card.inner));
                    }
                });
            });
            output.textures_delta.clear();
            assert_eq!(cards.len(), 2);
            for (_, (header, name, actions)) in &cards {
                assert!(header.bottom() <= name.top());
                assert!(name.bottom() <= actions.top());
            }
            assert!((cards[0].0.height() - cards[1].0.height()).abs() < 0.1);
            assert!(cards[0].0.right() <= cards[1].0.left());
        }
    }

    #[test]
    fn long_snapshot_name_does_not_expand_responsive_grid() {
        fn render_width(language: Language, name: &str) -> f32 {
            let context = egui::Context::default();
            let mut app = Flow8App::from_context(&context);
            app.language = language;
            app.store.state.snapshots.device_slots[0]
                .name
                .observe(name.into(), EvidenceStatus::VerifiedFromDevice);
            let mut width = 0.0;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
                ..Default::default()
            };
            let mut output = context.run_ui(raw, |ui| {
                width = ui.scope(|ui| app.snapshots_page(ui)).response.rect.width();
            });
            output.textures_delta.clear();
            width
        }

        for language in [Language::English, Language::Chinese] {
            let short = render_width(language, "A");
            let long = render_width(language, &"VeryLongSnapshotName".repeat(20));
            assert!(
                (short - long).abs() < 1.0,
                "snapshot grid changed width for {language:?}: {short} vs {long}"
            );
        }
    }

    #[test]
    fn device_snapshot_actions_require_confirmation_and_cancel_is_safe() {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        app.store.state.session = SessionState::Ready;
        app.page = Page::Snapshots;
        app.store.state.snapshots.device_slots[0]
            .name
            .observe("Vocal".into(), EvidenceStatus::VerifiedFromDevice);
        let observed_name = Some("Vocal".into());
        app.request_confirmation(ConfirmationAction::LoadDeviceSnapshot {
            slot: 0,
            observed_name: observed_name.clone(),
        });
        assert!(app.store.queue.is_empty());
        app.pending_confirmation = None; // Explicit Cancel.
        assert!(app.store.queue.is_empty());

        app.request_confirmation(ConfirmationAction::DeleteDeviceSnapshot {
            slot: 0,
            observed_name: observed_name.clone(),
        });
        app.page = Page::Settings;
        app.clear_stale_confirmation();
        assert!(app.pending_confirmation.is_none());
        app.confirm_pending_action();
        assert!(app.store.queue.is_empty());

        app.page = Page::Snapshots;
        app.request_confirmation(ConfirmationAction::SaveDeviceSnapshot {
            slot: 0,
            name: "Vocal".into(),
            observed_name,
        });
        assert!(app.store.queue.is_empty());
        app.confirm_pending_action();
        assert!(matches!(
            app.store.queue.pop(),
            Some(SemanticCommand::SaveSnapshot { slot: 0, .. })
        ));
    }

    #[test]
    fn snapshot_names_obey_protocol_byte_limit_and_submit_edited_name() {
        assert!(!valid_snapshot_name("  "));
        assert!(!valid_snapshot_name("line\nbreak"));
        assert!(valid_snapshot_name("中文中文中文")); // 18 UTF-8 bytes.
        assert!(!valid_snapshot_name("中文中文中文中")); // 21 UTF-8 bytes.
        assert!(valid_snapshot_name("  Vocal  "));

        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        app.store.state.session = SessionState::Ready;
        app.page = Page::Snapshots;
        app.store.state.snapshots.device_slots[0]
            .name
            .observe("Old".into(), EvidenceStatus::VerifiedFromDevice);

        app.request_confirmation(ConfirmationAction::SaveDeviceSnapshot {
            slot: 0,
            name: "  New name  ".into(),
            observed_name: Some("Old".into()),
        });
        assert!(app.store.queue.is_empty());
        app.confirm_pending_action();
        assert!(matches!(
            app.store.queue.pop(),
            Some(SemanticCommand::SaveSnapshot { slot: 0, name }) if name == "New name"
        ));

        app.request_confirmation(ConfirmationAction::RenameDeviceSnapshot {
            slot: 0,
            name: "Vocal".into(),
            observed_name: Some("Old".into()),
        });
        assert!(app.store.queue.is_empty());
        app.confirm_pending_action();
        assert!(matches!(
            app.store.queue.pop(),
            Some(SemanticCommand::RenameSnapshot { slot: 0, name }) if name == "Vocal"
        ));

        app.request_confirmation(ConfirmationAction::SaveDeviceSnapshot {
            slot: 0,
            name: "中文中文中文中".into(),
            observed_name: Some("Old".into()),
        });
        app.confirm_pending_action();
        assert!(app.store.queue.is_empty());
    }

    #[test]
    fn phantom_confirmation_expires_when_input_or_session_changes() {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        app.store.state.session = SessionState::Ready;
        app.store.state.channels[0]
            .phantom_48v
            .observe(false, EvidenceStatus::VerifiedFromDevice);
        app.request_confirmation(ConfirmationAction::EnablePhantom(InputId::Input1));
        assert!(app.store.queue.is_empty());
        app.store.state.selected_input = Some(InputId::Input2);
        app.clear_stale_confirmation();
        assert!(app.pending_confirmation.is_none());
        app.confirm_pending_action();
        assert!(app.store.queue.is_empty());

        app.store.state.selected_input = Some(InputId::Input1);
        app.request_confirmation(ConfirmationAction::EnablePhantom(InputId::Input1));
        app.store.state.session = SessionState::Disconnected;
        app.clear_stale_confirmation();
        assert!(app.pending_confirmation.is_none());
        assert!(app.store.queue.is_empty());
    }

    #[test]
    fn snapshot_name_refresh_uses_existing_read_only_protocol_request() {
        assert!(matches!(
            SemanticCommand::RequestSnapshotNames.to_protocol(),
            flow8_protocol::TxCommand::GetSnapshotNames
        ));
    }

    #[test]
    fn production_gui_starts_disconnected_without_simulator_source() {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        assert!(!app.store.is_simulator());
        assert_eq!(app.store.state.session, SessionState::Disconnected);
        assert!(app.store.queue.is_empty());
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
            ..Default::default()
        };
        let mut output = context.run_ui(raw, |ui| {
            device_unsynced_notice(ui, app.store.state.session, app.language);
            ui.add_enabled_ui(false, |ui| app.mixer(ui));
        });
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
        app.dispatch(SemanticCommand::SetMute {
            input: InputId::Input1,
            enabled: true,
        });
        assert!(app.store.queue.is_empty());
        assert_eq!(app.store.state.channels[0].muted.pending, None);
    }

    #[test]
    fn idle_mixer_never_echoes_received_pan_to_tx_queue() {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        app.store = Flow8Store::disconnected();
        app.store.state.session = SessionState::Ready;
        for (endpoint, value) in [(0, 1.0 / 127.0), (6, 0.0), (15, 0.0)] {
            app.store
                .apply_rx(
                    flow8_protocol::RxCommand::Pan { endpoint, value },
                    EvidenceStatus::VerifiedFromDevice,
                )
                .unwrap();
        }
        app.store
            .apply_rx(
                flow8_protocol::RxCommand::Compressor {
                    endpoint: 0,
                    amount: 101.0 / 255.0,
                },
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
                ..Default::default()
            };
            let mut output = context.run_ui(raw, |ui| app.mixer(ui));
            output.textures_delta.clear();
            let mut commands = Vec::new();
            while let Some(command) = app.store.queue.pop() {
                commands.push(command);
            }
            assert!(commands.is_empty(), "idle GUI sent commands: {commands:?}");
        }
    }

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
                    assert!(
                        app.store.queue.is_empty(),
                        "idle page {page:?} generated a device command for {language:?} at {size:?}"
                    );
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
        assert_eq!(
            style.visuals.widgets.inactive.bg_stroke.width,
            style.visuals.widgets.hovered.bg_stroke.width
        );
        assert_eq!(
            style.visuals.widgets.hovered.bg_stroke.width,
            style.visuals.widgets.active.bg_stroke.width
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
        configure_fonts(&context);
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

const MAX_PRODUCTION_LOG_BYTES: u64 = 16 * 1024 * 1024;

struct ProductionLogFile {
    file: File,
    written: u64,
    capped: bool,
}

#[derive(Clone)]
struct ProductionLogWriter {
    file: Arc<Mutex<ProductionLogFile>>,
    console: bool,
}

struct ProductionLogSink {
    file: Arc<Mutex<ProductionLogFile>>,
    stdout: Option<io::Stdout>,
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for ProductionLogWriter {
    type Writer = ProductionLogSink;

    fn make_writer(&'writer self) -> Self::Writer {
        ProductionLogSink {
            file: Arc::clone(&self.file),
            stdout: self.console.then(io::stdout),
        }
    }
}

impl Write for ProductionLogSink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(stdout) = &mut self.stdout {
            let _ = stdout.write_all(buffer);
        }
        let mut log = self.file.lock().unwrap_or_else(|error| error.into_inner());
        if !log.capped {
            if log.written.saturating_add(buffer.len() as u64) <= MAX_PRODUCTION_LOG_BYTES {
                log.file.write_all(buffer)?;
                log.written += buffer.len() as u64;
            } else {
                log.capped = true;
                let _ = log.file.write_all(
                    b"warning: FLOW 8 log reached its 16 MiB limit; further entries are suppressed until the next launch\n",
                );
                if self.stdout.is_some() {
                    eprintln!("warning: FLOW 8 log reached its 16 MiB limit");
                }
            }
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .file
            .flush()?;
        if let Some(stdout) = &mut self.stdout {
            let _ = stdout.flush();
        }
        Ok(())
    }
}

fn production_log_directory() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(root)
            .join("FLOW 8 PC Controller")
            .join("logs"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(root) = std::env::var_os("XDG_STATE_HOME") {
            return Ok(PathBuf::from(root).join("flow8-pc-controller"));
        }
        if let Some(root) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(root).join(".local/state/flow8-pc-controller"));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no private per-user log directory is available",
    ))
}

fn open_production_log() -> io::Result<(File, PathBuf)> {
    let directory = production_log_directory()?;
    fs::create_dir_all(&directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "log directory is not a real directory",
        ));
    }
    #[cfg(unix)]
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;

    const MAX_RETAINED_PRODUCTION_LOGS: usize = 12;
    let retained = fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("windows-production-connection-") && name.ends_with(".log")
        })
        .count();
    if retained >= MAX_RETAINED_PRODUCTION_LOGS {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "12 FLOW 8 diagnostic logs are retained; archive or remove old logs before capturing another",
        ));
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16 {
        let path = directory.join(format!(
            "windows-production-connection-{timestamp}-{}-{attempt}.log",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique FLOW 8 log filename",
    ))
}

// Routine release logs rotate independently of hardware-evidence captures.
fn open_application_log() -> io::Result<(File, PathBuf)> {
    let directory = production_log_directory()?;
    fs::create_dir_all(&directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "log directory is not a real directory",
        ));
    }
    #[cfg(unix)]
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;

    const MAX_RETAINED_APPLICATION_LOGS: usize = 12;
    let mut old_logs = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("flow8-gui-") && name.ends_with(".log") && entry.file_type()?.is_file()
        {
            old_logs.push(entry.path());
        }
    }
    old_logs.sort();
    let remove_count = old_logs
        .len()
        .saturating_sub(MAX_RETAINED_APPLICATION_LOGS - 1);
    for path in old_logs.into_iter().take(remove_count) {
        fs::remove_file(path)?;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16 {
        let path = directory.join(format!(
            "flow8-gui-{timestamp}-{}-{attempt}.log",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique FLOW 8 application log filename",
    ))
}

fn flow8_ble_verbose_logging(filter: &str) -> bool {
    filter.split(',').any(|directive| {
        let directive = directive.trim().to_ascii_lowercase();
        directive == "flow8_ble=debug" || directive == "flow8_ble=trace"
    })
}

fn init_tracing() -> Option<String> {
    const DEFAULT_FILTER: &str = "flow8_ble=info,flow8_directhci=info,flow8_gui=info";
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_FILTER.into());
    let filter = tracing_subscriber::EnvFilter::try_new(&rust_log)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_FILTER));
    let verbose = flow8_ble_verbose_logging(&rust_log);
    let console = !cfg!(all(windows, not(debug_assertions)));
    let log_file = if verbose {
        Some(open_production_log())
    } else if !console {
        Some(open_application_log())
    } else {
        None
    };
    let mut startup_error = None;
    if let Some(log_file) = log_file {
        match log_file {
            Ok((file, path)) => {
                if console {
                    eprintln!("FLOW 8 log: {}", path.display());
                }
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_writer(ProductionLogWriter {
                        file: Arc::new(Mutex::new(ProductionLogFile {
                            file,
                            written: 0,
                            capped: false,
                        })),
                        console,
                    })
                    .try_init();
                tracing::info!(target: "flow8_ble", path = %path.display(), "FLOW 8 log file opened");
                if verbose {
                    tracing::warn!(
                        target: "flow8_ble",
                        "verbose logs contain raw device state and client identifiers; review before sharing"
                    );
                }
                return None;
            }
            Err(error) => {
                let message = format!("could not create private FLOW 8 log: {error}");
                if console {
                    eprintln!("warning: {message}");
                } else {
                    startup_error = Some(message);
                }
            }
        }
    }
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    startup_error
}

fn main() -> eframe::Result {
    let logging_error = init_tracing();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FLOW 8 PC Controller")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/flow8.png"))
                    .expect("bundled FLOW 8 window icon must be a valid PNG"),
            )
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1040.0, 700.0]),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "FLOW 8 PC Controller",
        options,
        Box::new(move |context| {
            let mut app = Flow8App::new(context);
            if let Some(error) = &logging_error {
                app.message = format!("Logging unavailable: {error}");
            }
            Ok(Box::new(app))
        }),
    )
}
