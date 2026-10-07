use crate::app::Flow8App;
use crate::i18n::{destination_name, display_name};
use crate::theme::{
    BLUE, BORDER, GREEN, RED, STAGE_RING, STAGE_SELECTED_SURFACE, STAGE_SURFACE, SURFACE,
    SURFACE_ALT, TEXT, YELLOW,
};
use crate::widgets::{
    accessible_slider_input, accessible_slider_node, destination_button, fader_level_text,
    state_button,
};
use eframe::egui::{self, Align, FontId, Layout, Rect, Sense, Stroke, StrokeKind, Vec2};
use flow8_core::{ChannelStateTarget, SemanticCommand};
use flow8_model::specs;
use flow8_model::{InputId, MixDestination};

impl Flow8App {
    pub(in crate::app) fn stage_page(&mut self, ui: &mut egui::Ui) {
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
                        if self.device_controls_ready() {
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
                        .unwrap_or(&specs::TEMPO_DISPLAY.default)
                ));
            });
        });
        ui.add_space(8.0);
        let (canvas, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), ui.available_height().max(460.0)),
            Sense::hover(),
        );
        ui.painter().rect_filled(canvas, 8.0, STAGE_SURFACE);
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
            if response.clicked() || response.drag_started() {
                response.request_focus();
            }
            let keyboard_select = response.has_focus()
                && ui.input(|input| {
                    input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
                });
            if self.stage_layout_mode {
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        ui.is_enabled(),
                        self.store.state.selected_input == Some(id),
                        format!(
                            "{} · {}",
                            display_name(id, self.language),
                            self.language.tr("Stage position", "舞台位置")
                        ),
                    )
                });
                if response.dragged() {
                    self.stage_positions[index] += ui.input(|input| input.pointer.delta());
                }
                if response.has_focus() && ui.is_enabled() {
                    let delta = ui.input(|input| {
                        let step = if input.modifiers.shift { 1.0 } else { 10.0 };
                        Vec2::new(
                            if input.key_pressed(egui::Key::ArrowRight) {
                                step
                            } else if input.key_pressed(egui::Key::ArrowLeft) {
                                -step
                            } else {
                                0.0
                            },
                            if input.key_pressed(egui::Key::ArrowDown) {
                                step
                            } else if input.key_pressed(egui::Key::ArrowUp) {
                                -step
                            } else {
                                0.0
                            },
                        )
                    });
                    self.stage_positions[index] += delta;
                    if delta != Vec2::ZERO {
                        ui.ctx().request_repaint();
                    }
                }
            } else {
                let destination = self.store.state.selected_destination;
                let current = self.store.state.channels[index].route_levels[destination.index()]
                    .effective()
                    .copied()
                    .unwrap_or(0.0);
                let editable = self.device_controls_ready() && ui.is_enabled();
                response.widget_info(|| {
                    egui::WidgetInfo::slider(
                        editable,
                        f64::from(current),
                        format!(
                            "{} → {}",
                            display_name(id, self.language),
                            destination_name(destination)
                        ),
                    )
                });
                if editable {
                    let mut next = current;
                    if response.dragged() {
                        next -= ui.input(|input| input.pointer.delta().y) / 180.0;
                    }
                    if response.has_focus() {
                        next = ui.input(|input| {
                            let step = if input.modifiers.shift { 0.001 } else { 0.01 };
                            if input.key_pressed(egui::Key::ArrowUp) {
                                next + step
                            } else if input.key_pressed(egui::Key::ArrowDown) {
                                next - step
                            } else if input.key_pressed(egui::Key::Home) {
                                0.0
                            } else if input.key_pressed(egui::Key::End) {
                                1.0
                            } else {
                                next
                            }
                        });
                    }
                    accessible_slider_input(ui, &response, &mut next, 0.0, 1.0, 0.01);
                    accessible_slider_node(ui, &response, current, 0.0, 1.0, 0.01);
                    next = next.clamp(0.0, 1.0);
                    if (next - current).abs() > f32::EPSILON {
                        self.dispatch(SemanticCommand::SetRouteLevel {
                            source: id,
                            destination,
                            normalized: next,
                        });
                    }
                }
            }
            if response.clicked() || keyboard_select {
                let selection_changed = self.store.state.selected_input != Some(id);
                self.store.state.selected_input = Some(id);
                if selection_changed && self.device_controls_ready() {
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
                    STAGE_SELECTED_SURFACE
                } else {
                    SURFACE_ALT
                },
            );
            ui.painter().rect_stroke(
                rect,
                8.0,
                Stroke::new(
                    if selected || response.has_focus() {
                        2.0
                    } else {
                        1.0
                    },
                    if response.has_focus() {
                        BLUE
                    } else if selected {
                        YELLOW
                    } else {
                        BORDER
                    },
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
            ui.painter()
                .circle_stroke(center, 39.0, Stroke::new(7.0, STAGE_RING));
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
}
