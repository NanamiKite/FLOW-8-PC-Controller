use crate::app::Flow8App;
use crate::i18n::{destination_name, display_name};
use crate::metrics::{DESTINATION_BUTTON_MIN_WIDTH, STATE_BUTTON_MIN_WIDTH};
use crate::theme::{
    BLUE, BORDER, GREEN, RED, SECONDARY, STAGE_RING, STAGE_SELECTED_SURFACE, STAGE_SURFACE,
    SURFACE, SURFACE_ALT, TEXT, YELLOW,
};
use crate::widgets::{
    accessible_slider_input, accessible_slider_node, destination_button, fader_level_text,
    state_button_sized,
};
use eframe::egui::{self, Align, FontId, Layout, Rect, Sense, Stroke, StrokeKind, Vec2};
use flow8_core::{ChannelStateTarget, SemanticCommand};
use flow8_model::{InputId, MixDestination};

impl Flow8App {
    pub(in crate::app) fn stage_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(14.0);
        self.stage_toolbar(ui);
        ui.add_space(8.0);
        let (canvas, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), ui.available_height().max(460.0)),
            Sense::hover(),
        );
        ui.painter().rect_filled(canvas, 8.0, STAGE_SURFACE);
        for id in InputId::ALL {
            let index = id.index();
            // Reserve the actual themed button height, rather than letting a
            // standard-height button overflow a 23-point slot at the bottom.
            let button_height = self.metrics.control_height;
            let size = Vec2::new(170.0, 155.0 + button_height);
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
                .copied();
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
                FontId::proportional(self.metrics.body_font),
                TEXT,
            );
            let center = rect.center_top() + Vec2::new(0.0, 87.0);
            ui.painter()
                .circle_stroke(center, 39.0, Stroke::new(7.0, STAGE_RING));
            let filled = (route.unwrap_or(0.0) * 24.0).round() as usize;
            for segment in 0..filled {
                let angle = std::f32::consts::PI * (0.75 + segment as f32 / 18.0);
                let a = center + Vec2::angled(angle) * 35.0;
                let b = center + Vec2::angled(angle) * 42.0;
                ui.painter().line_segment([a, b], Stroke::new(3.0, YELLOW));
            }
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                route
                    .map(fader_level_text)
                    .unwrap_or_else(|| "—".to_owned()),
                FontId::proportional(self.metrics.body_font),
                TEXT,
            );
            let meter_width = ((meter + 60.0) / 70.0).clamp(0.0, 1.0) * (rect.width() - 24.0);
            let meter_rect = Rect::from_min_size(
                rect.left_bottom() + Vec2::new(12.0, -button_height - 18.0),
                Vec2::new(meter_width, 5.0),
            );
            ui.painter()
                .rect_filled(meter_rect, 2.0, if meter >= 0.0 { RED } else { GREEN });
            let button_width = (rect.width() - 24.0 - 8.0) / 2.0;
            let mute_rect = Rect::from_min_size(
                rect.left_bottom() + Vec2::new(12.0, -button_height - 8.0),
                Vec2::new(button_width, button_height),
            );
            let solo_rect = Rect::from_min_size(
                rect.right_bottom() + Vec2::new(-12.0 - button_width, -button_height - 8.0),
                Vec2::new(button_width, button_height),
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

    fn stage_toolbar(&mut self, ui: &mut egui::Ui) {
        let control_label = self.language.tr("Control", "控制");
        let layout_label = self.language.tr("Layout", "布局");
        let bus_label = self.language.tr("Mix bus", "混音总线");
        let measure = |text: &str| {
            ui.painter()
                .layout_no_wrap(
                    text.to_owned(),
                    FontId::proportional(self.metrics.body_font),
                    TEXT,
                )
                .size()
                .x
        };
        let button_padding = 2.0 * ui.spacing().button_padding.x + 4.0;
        let mode_width = (measure(control_label).max(measure(layout_label)) + button_padding)
            .max(STATE_BUTTON_MIN_WIDTH)
            .ceil();
        let bus_width = (measure("MON1").max(measure("MAIN")) + button_padding)
            .max(DESTINATION_BUTTON_MIN_WIDTH)
            .ceil();
        let gap = self.metrics.spacing;
        let height = self.metrics.control_height;
        let bus_group_width = measure(bus_label) + 5.0 * (bus_width + gap);
        // Keep each control group together when the toolbar wraps. The margin
        // aligns the header with the initial stage cards, not the window edge.
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(34, 0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0 * gap;
                    ui.heading(
                        egui::RichText::new(self.language.tr("Stage View", "舞台视图")).color(TEXT),
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(2.0 * mode_width + gap, height),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (layout_mode, label, accent) in
                                [(false, control_label, YELLOW), (true, layout_label, BLUE)]
                            {
                                if state_button_sized(
                                    ui,
                                    self.stage_layout_mode == layout_mode,
                                    label,
                                    accent,
                                    Vec2::new(mode_width, height),
                                )
                                .clicked()
                                {
                                    self.stage_layout_mode = layout_mode;
                                }
                            }
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(bus_group_width, height),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.label(egui::RichText::new(bus_label).color(SECONDARY));
                            for destination in MixDestination::ALL {
                                let response = ui
                                    .allocate_ui_with_layout(
                                        Vec2::new(bus_width, height),
                                        Layout::centered_and_justified(
                                            egui::Direction::LeftToRight,
                                        ),
                                        |ui| {
                                            destination_button(
                                                ui,
                                                self.store.state.selected_destination
                                                    == destination,
                                                destination_name(destination),
                                            )
                                        },
                                    )
                                    .inner;
                                if response.clicked() {
                                    if self.store.state.selected_destination != destination {
                                        self.store.state.selected_destination = destination;
                                        if self.device_controls_ready() {
                                            self.dispatch(SemanticCommand::RequestChannelState {
                                                target: ChannelStateTarget::Destination(
                                                    destination,
                                                ),
                                            });
                                        }
                                    }
                                }
                            }
                        },
                    );
                    let tempo = self
                        .store
                        .state
                        .global_tempo_bpm
                        .effective()
                        .map(|tempo| format!("{tempo:.0} BPM"))
                        .unwrap_or_else(|| "— BPM".to_owned());
                    ui.add(egui::Label::new(egui::RichText::new(tempo).color(SECONDARY)).extend());
                });
            });
    }
}
