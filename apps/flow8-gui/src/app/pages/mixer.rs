use crate::app::{Flow8App, input_eq_modified};
use crate::i18n::{channel_number, destination_name};
use crate::metrics::LayoutMetrics;
use crate::theme::{BLUE, MASTER_SURFACE, SECONDARY, TEXT, YELLOW};
use crate::widgets::{
    StripAction, channel_strip, fader_level_text, fader_sized, meter_widget_sized, mixer_divider,
    strip_controls,
};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Align, Layout, Stroke, Vec2};
use flow8_core::{ChannelStateTarget, SemanticCommand};
use flow8_model::{InputId, MixDestination};

impl Flow8App {
    pub(in crate::app) fn mixer(&mut self, ui: &mut egui::Ui) {
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
                if selection_changed && self.device_controls_ready() {
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
            .fill(MASTER_SURFACE)
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
}
