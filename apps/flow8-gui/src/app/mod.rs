use self::confirmation::PendingConfirmation;
use crate::fonts::configure_fonts;
use crate::i18n::{Language, native_stage_text, session_phase_text, session_state_text};
use crate::metrics::UiMetrics;
use crate::theme::{BG, BORDER, SECONDARY, SURFACE, TOOLBAR_SURFACE, configure_style};
use eframe::egui::{self, Align, Layout, Stroke, Vec2};
use flow8_ble::{DeviceCommand, DeviceEvent, DeviceRuntime, NativeConnectionStage, SessionPhase};
use flow8_core::{Flow8Store, SemanticCommand, SessionState};
use flow8_model::{EvidenceStatus, InputChannelState, MixBusId, MixDestination, specs};
use std::time::{Duration, Instant};

mod confirmation;
mod navigation;
mod pages;

#[cfg(test)]
mod tests;

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
    fn advance(&mut self, target_db: Option<f32>, elapsed_seconds: f32) -> bool {
        let Some(target_db) = target_db else {
            *self = Self::default();
            return false;
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
        self.display_db != target_db
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

pub(crate) struct Flow8App {
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
    session_generation: Option<u64>,
    connection_request_pending: bool,
    disconnect_request_pending: bool,
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
    pub(crate) fn new(context: &eframe::CreationContext<'_>, logging_error: Option<&str>) -> Self {
        let mut app = Self::from_context(&context.egui_ctx);
        if let Some(error) = logging_error {
            app.message = format!("Logging unavailable: {error}");
        }
        app
    }

    fn from_context(context: &egui::Context) -> Self {
        configure_fonts(context);
        let metrics =
            UiMetrics::calculate(Vec2::new(1440.0, 920.0), context.pixels_per_point(), 1.0);
        configure_style(context, metrics);
        let runtime = DeviceRuntime::spawn();
        let event_context = context.clone();
        runtime.set_event_waker(move || event_context.request_repaint());
        Self {
            store: Flow8Store::disconnected(),
            page: Page::Mixer,
            language: Language::English,
            message: "Disconnected · Connect to FLOW 8 to load the current mixer state".into(),
            runtime,
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
            session_generation: None,
            connection_request_pending: false,
            disconnect_request_pending: false,
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

    fn device_controls_ready(&self) -> bool {
        self.store.state.session == SessionState::Ready && !self.disconnect_request_pending
    }

    fn reset_device_requests(&mut self) {
        self.pending_confirmation = None;
        self.snapshot_names_requested = false;
        self.channel_labels_requested = false;
        self.meter_request_target = None;
    }

    fn request_disconnect(&mut self) -> Result<(), String> {
        self.reset_device_requests();
        match self.runtime.send(DeviceCommand::Disconnect) {
            Ok(()) => {
                self.disconnect_request_pending = true;
                self.connection_request_pending = false;
                self.store.clear_pending("Disconnect requested");
                self.message = self.language.tr("Disconnecting", "正在断开").into();
                Ok(())
            }
            Err(error) => {
                tracing::warn!(target: "flow8_gui", %error, "FLOW disconnect request could not reach the runtime");
                self.disconnect_request_pending = false;
                self.connection_request_pending = false;
                self.session_generation = None;
                self.state_sync_applied = false;
                self.store.apply_session_phase(SessionState::Error);
                self.last_ble_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn dispatch(&mut self, command: SemanticCommand) {
        if !self.device_controls_ready() {
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
                DeviceEvent::SessionStarted(generation) => {
                    self.session_generation = Some(generation);
                    self.state_sync_applied = false;
                    self.store.clear_pending("New device session");
                    self.reset_device_requests();
                }
                DeviceEvent::Phase(phase) => {
                    self.connection_request_pending = false;
                    if matches!(phase, SessionPhase::Disconnected | SessionPhase::Error) {
                        self.disconnect_request_pending = false;
                        self.session_generation = None;
                        self.state_sync_applied = false;
                        self.store.clear_pending("Device session ended");
                    }
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
                        self.reset_device_requests();
                    }
                    if matches!(phase, SessionPhase::Connecting | SessionPhase::StateSyncing) {
                        self.state_sync_applied = false;
                    }
                    if phase == SessionPhase::Ready {
                        self.last_ble_error = None;
                        self.native_stage = None;
                    }
                    if phase == SessionPhase::Ready && !self.state_sync_applied {
                        let disconnect_error = self.request_disconnect().err();
                        self.store.clear_pending("Invalid state synchronization");
                        self.store.apply_session_phase(SessionState::Error);
                        tracing::warn!(target: "flow8_gui",
                            "FLOW runtime reported Ready without an applied mixer state"
                        );
                        self.message = self
                            .language
                            .tr(
                                "Device synchronization failed. Please reconnect.",
                                "设备状态同步失败，请重新连接。",
                            )
                            .into();
                        if let Some(error) = disconnect_error {
                            self.message.push_str(&format!(" {error}"));
                        }
                    } else {
                        self.store.apply_session_phase(core_session_state(phase));
                        self.message = if self.disconnect_request_pending {
                            self.language.tr("Disconnecting", "正在断开").into()
                        } else {
                            format!(
                                "{}: {}",
                                self.language.tr("Bluetooth", "蓝牙"),
                                session_phase_text(phase, self.language)
                            )
                        };
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
                    self.apply_device_rx(command, None);
                }
                DeviceEvent::MixerState {
                    generation,
                    revision,
                    state,
                } => {
                    if self.session_generation == Some(generation) {
                        self.apply_device_rx(
                            flow8_protocol::RxCommand::MixerState(state),
                            Some((generation, revision)),
                        );
                    }
                }
                DeviceEvent::CommandFailed { id, error } => {
                    self.store.command_failed(id, &error);
                    self.message = format!(
                        "{}: {error}",
                        self.language.tr("Command failed", "命令失败")
                    );
                }
                DeviceEvent::CommandWriting { id } => self.store.command_started(id),
                // Raw transport details remain in tracing logs, not the user-facing status bar.
                DeviceEvent::RawRx(_)
                | DeviceEvent::RawTx(_)
                | DeviceEvent::FrameWritten { .. }
                | DeviceEvent::WriteMode(_)
                | DeviceEvent::Backend(_)
                | DeviceEvent::Mtu(_) => {}
                DeviceEvent::ProtocolWarning(warning) => {
                    tracing::debug!(target: "flow8_gui", %warning, "FLOW device communication warning");
                    self.message = self
                        .language
                        .tr(
                            "Some device data could not be processed. See the connection log.",
                            "部分设备数据未能处理，请查看连接日志。",
                        )
                        .into();
                }
                DeviceEvent::CommandError(error) => {
                    self.message = format!(
                        "{}: {error}",
                        self.language.tr("Command failed", "命令失败")
                    );
                }
                DeviceEvent::Error(error) => {
                    self.connection_request_pending = false;
                    self.disconnect_request_pending = false;
                    self.store.clear_pending(&error);
                    self.session_generation = None;
                    self.state_sync_applied = false;
                    self.reset_device_requests();
                    self.store.apply_session_phase(SessionState::Error);
                    self.last_ble_error = Some(error.clone());
                    self.message = format!("BLE: {error}");
                }
            }
        }
        if self.store.expire_pending(Instant::now()) {
            self.message = self
                .language
                .tr(
                    "Some changes were not confirmed by the device",
                    "部分调整尚未得到设备确认，已恢复设备数值",
                )
                .into();
        }
        self.flush_commands();
    }

    fn apply_device_rx(
        &mut self,
        command: flow8_protocol::RxCommand,
        acknowledgment: Option<(u64, u64)>,
    ) {
        if let Err(error) = self
            .store
            .apply_rx(command, EvidenceStatus::VerifiedFromDevice)
        {
            self.message = format!(
                "{}: {error}",
                self.language.tr("RX state error", "接收状态错误")
            );
        } else if let Some((generation, revision)) = acknowledgment {
            if self.eq_initial.iter().all(Option::is_none) {
                for (initial, channel) in self.eq_initial.iter_mut().zip(&self.store.state.channels)
                {
                    *initial = confirmed_eq_snapshot(channel);
                }
            }
            self.state_sync_applied = true;
            tracing::info!(target: "flow8_gui",
                evidence = "VERIFIED_FROM_DEVICE",
                "complete FLOW 8 state atomically applied to confirmed Store"
            );
            if let Err(error) = self.runtime.send(DeviceCommand::StateApplied {
                generation,
                revision,
            }) {
                let disconnect_error = self.request_disconnect().err();
                self.store
                    .clear_pending("State apply acknowledgment failed");
                self.store.apply_session_phase(SessionState::Error);
                self.message = error;
                if let Some(error) = disconnect_error {
                    self.message.push_str(&format!(" {error}"));
                }
            }
        }
    }

    fn sync_channel_labels_request(&mut self) {
        if !self.device_controls_ready() {
            self.channel_labels_requested = false;
        } else if !self.channel_labels_requested {
            self.dispatch(SemanticCommand::RequestChannelLabels);
            self.channel_labels_requested = true;
        }
    }

    fn sync_meter_request(&mut self) {
        if !self.device_controls_ready() {
            self.meter_request_target = None;
            return;
        }
        let destination = self.store.state.selected_destination;
        if self.meter_request_target != Some(destination) {
            self.dispatch(SemanticCommand::RequestMeters { destination });
            self.meter_request_target = Some(destination);
        }
    }

    fn advance_meter_display(&mut self) -> bool {
        if !self.device_controls_ready() {
            self.input_meter_display = [MeterSmoother::default(); 7];
            self.output_meter_display = [MeterSmoother::default(); 5];
            self.meter_frame_at = None;
            return false;
        }

        let now = Instant::now();
        let elapsed_seconds = self
            .meter_frame_at
            .replace(now)
            .map(|previous| now.saturating_duration_since(previous).as_secs_f32())
            .unwrap_or(1.0 / 30.0)
            .clamp(0.0, 0.25);

        let mut animating = false;
        for (index, smoother) in self.input_meter_display.iter_mut().enumerate() {
            animating |= smoother.advance(
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
            animating |=
                self.output_meter_display[destination.index()].advance(target, elapsed_seconds);
        }
        animating
    }

    fn flush_commands(&mut self) {
        if self.disconnect_request_pending {
            return;
        }
        while let Some((id, command)) = self.store.queue.front_protocol_tracked() {
            if let Err(error) = self.runtime.send(DeviceCommand::SendTracked {
                id,
                generation: self.session_generation.unwrap_or_default(),
                command,
            }) {
                if error.starts_with("BLE user command queue is full") {
                    self.message = error;
                    break;
                }
                let _ = self.store.queue.pop_tracked();
                self.store.command_failed(id, &error);
                self.message = error;
            } else {
                let _ = self.store.queue.pop_tracked();
            }
        }
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
        self.store.set_fader_pointer_down(
            ui.input(|input| input.pointer.primary_down()),
            Instant::now(),
        );
        self.poll_runtime();
        let meters_animating = self.advance_meter_display();
        self.clear_stale_confirmation();
        if meters_animating {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }

        ui.set_min_size(ui.available_size());
        ui.painter().rect_filled(ui.max_rect(), 0.0, BG);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            self.connection_bar(ui);
            self.layer_bar(ui);
            self.clear_stale_confirmation();
            if self.device_controls_ready() && self.page == Page::Snapshots {
                if !self.snapshot_names_requested {
                    self.dispatch(SemanticCommand::RequestSnapshotNames);
                    self.snapshot_names_requested = true;
                }
            } else {
                self.snapshot_names_requested = false;
            }
            ui.spacing_mut().item_spacing = Vec2::splat(self.metrics.spacing);
            let device_ready = self.device_controls_ready();
            if !device_ready && !self.disconnect_request_pending && self.page != Page::Settings {
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
                    .fill(TOOLBAR_SURFACE)
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
        if let Some(deadline) = self.store.next_pending_deadline() {
            ui.ctx()
                .request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
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
