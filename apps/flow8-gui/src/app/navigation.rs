use crate::app::{Flow8App, Page};
use crate::i18n::{Language, localized_status_message, native_stage_text, session_state_text};
use crate::theme::{
    BLUE, BORDER, GREEN, PURPLE, RED, SECONDARY, SURFACE, TEXT, TOOLBAR_SURFACE, YELLOW,
};
use crate::widgets::small_tab;
use eframe::egui::{self, Align, Color32, Layout, Stroke};
use flow8_ble::DeviceCommand;
use flow8_core::{ChannelStateTarget, Flow8Store, SemanticCommand, SessionState};
use flow8_model::MixDestination;
use std::time::Duration;

impl Flow8App {
    pub(super) fn connection_bar(&mut self, ui: &mut egui::Ui) {
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
                        if ui
                            .add_enabled(
                                !self.disconnect_request_pending,
                                egui::Button::new(self.language.tr("Disconnect", "断开")),
                            )
                            .clicked()
                        {
                            if let Err(error) = self.request_disconnect() {
                                self.message = format!(
                                    "{}: {error}",
                                    self.language.tr("Could not disconnect", "无法断开连接")
                                );
                            }
                        }
                        let can_connect = !self.connection_request_pending
                            && !self.disconnect_request_pending
                            && matches!(
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
                            self.reset_device_requests();
                            self.state_sync_applied = false;
                            self.eq_initial = [None; 7];
                            self.delay_ms_draft = std::array::from_fn(|_| String::new());
                            self.delay_ms_dirty = [false; 3];
                            match self.runtime.send(DeviceCommand::Connect) {
                                Ok(()) => {
                                    self.connection_request_pending = true;
                                    self.native_stage = None;
                                    self.last_ble_error = None;
                                }
                                Err(error) => self.message = error,
                            }
                        }
                        let can_scan = !self.connection_request_pending
                            && !self.disconnect_request_pending
                            && matches!(
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
                                    self.connection_request_pending = true;
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

    pub(super) fn layer_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(TOOLBAR_SURFACE)
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
                    if self.device_controls_ready() {
                        self.dispatch(SemanticCommand::RequestChannelState {
                            target: ChannelStateTarget::Destination(destination),
                        });
                    }
                }
            }
        }
    }
}
