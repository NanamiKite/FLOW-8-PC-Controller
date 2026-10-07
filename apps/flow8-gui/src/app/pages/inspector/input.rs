use crate::app::Flow8App;
use crate::app::confirmation::ConfirmationAction;
use crate::i18n::{destination_name, display_name, waiting_for_device_text};
use crate::theme::{BLUE, RED, SECONDARY, TEXT, YELLOW, card, inspector_heading};
use crate::widgets::mute_button;
use crate::widgets::{
    committed_text, eq_gain_graph, eq_gain_row, fader_level_text, parameter_row,
    parameter_value_row, state_button,
};
use eframe::egui::{self, Align, Layout};
use flow8_core::{ChannelStateTarget, SemanticCommand};
use flow8_model::CHANNEL_LABEL_MAX_BYTES;
use flow8_model::{InputChannelState, InputId, specs};

impl Flow8App {
    pub(in crate::app) fn input_inspector(&mut self, ui: &mut egui::Ui, id: InputId) {
        let channel = self.store.state.channels[id.index()].clone();
        card(ui, |ui| {
            self.input_header_controls(ui, id, &channel);
            self.input_gain_pan_controls(ui, id, &channel);
            self.input_route_controls(ui, id, &channel);
            self.input_filter_controls(ui, id, &channel);
            self.input_eq_controls(ui, id, &channel);
            self.input_dynamics_controls(ui, id, &channel);
        });
    }

    fn input_header_controls(
        &mut self,
        ui: &mut egui::Ui,
        id: InputId,
        channel: &InputChannelState,
    ) {
        if ui
            .small_button(self.language.tr("Refresh channel", "刷新通道"))
            .clicked()
        {
            self.dispatch(SemanticCommand::RequestChannelState {
                target: ChannelStateTarget::Input(id),
            });
        }
        ui.horizontal(|ui| {
            inspector_heading(ui, self.language.tr("INPUT DETAIL", "输入详情"), SECONDARY);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(display_name(id, self.language))
                        .strong()
                        .color(TEXT),
                );
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(self.language.tr("Label", "名称")).color(TEXT));
            let edited = ui
                .add_enabled_ui(channel.icon.effective().is_some(), |ui| {
                    committed_text(
                        ui,
                        ("channel-name", self.session_generation, id),
                        channel.name.effective().map(String::as_str),
                        CHANNEL_LABEL_MAX_BYTES,
                        self.language,
                    )
                })
                .inner;
            if let Some(icon) = channel.icon.effective().copied()
                && let Some(label) = edited
            {
                self.dispatch(SemanticCommand::SetLabel {
                    input: id,
                    icon,
                    name: label,
                });
            }
        });
    }

    fn input_gain_pan_controls(
        &mut self,
        ui: &mut egui::Ui,
        id: InputId,
        channel: &InputChannelState,
    ) {
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
    }

    fn input_route_controls(
        &mut self,
        ui: &mut egui::Ui,
        id: InputId,
        channel: &InputChannelState,
    ) {
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
    }

    fn input_filter_controls(
        &mut self,
        ui: &mut egui::Ui,
        id: InputId,
        channel: &InputChannelState,
    ) {
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
                if state_button(ui, phase, self.language.tr("Phase", "极性"), BLUE).clicked() {
                    self.dispatch(SemanticCommand::SetPhase {
                        input: id,
                        inverted: !phase,
                    });
                }
            }
            if channel.capabilities.high_pass {
                let setting = channel
                    .high_pass_enabled
                    .effective()
                    .copied()
                    .zip(channel.high_pass_hz.effective().copied());
                let clicked = ui
                    .add_enabled_ui(setting.is_some(), |ui| {
                        state_button(
                            ui,
                            setting.is_some_and(|(enabled, _)| enabled),
                            self.language.tr("Low Cut", "低切"),
                            YELLOW,
                        )
                        .clicked()
                    })
                    .inner;
                if clicked && let Some((hpf, frequency)) = setting {
                    self.dispatch(SemanticCommand::SetHighPass {
                        input: id,
                        enabled: !hpf,
                        frequency_hz: frequency as u16,
                    });
                }
            }
        });
        if channel.capabilities.high_pass {
            if let Some((mut frequency, enabled)) = channel
                .high_pass_hz
                .effective()
                .copied()
                .zip(channel.high_pass_enabled.effective().copied())
            {
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
                        enabled,
                        frequency_hz: frequency.round() as u16,
                    });
                }
            } else {
                ui.weak(waiting_for_device_text(self.language));
            }
        }
    }

    fn input_eq_controls(&mut self, ui: &mut egui::Ui, id: InputId, channel: &InputChannelState) {
        if channel.capabilities.peq {
            ui.separator();
            inspector_heading(
                ui,
                self.language.tr("4-BAND PEQ", "四段参数均衡"),
                SECONDARY,
            );
            if let Some((band, gain)) = eq_gain_graph(ui, channel, self.metrics, self.language) {
                let state = &channel.eq.bands[band];
                if let (Some(frequency), Some(q)) =
                    (state.frequency_hz.effective(), state.q.effective())
                {
                    self.dispatch(SemanticCommand::SetPeqBand {
                        input: id,
                        band: band as u8,
                        frequency_hz: *frequency as u16,
                        q: *q,
                        gain_db: gain,
                    });
                }
            }
            ui.label(
                egui::RichText::new(self.language.tr(
                    "Drag a band · Shift: fine · Double-click band or gain slider: 0 dB",
                    "拖动频段 · Shift 精调 · 双击频段或增益滑块归零",
                ))
                .size(self.metrics.small_font)
                .color(SECONDARY),
            );
            for (index, band) in channel.eq.bands.iter().enumerate() {
                // The live values are presentation only: hashing them into
                // the widget ID resets expansion and child edits as they change.
                egui::CollapsingHeader::new(
                    egui::RichText::new(format!(
                        "{} {}  ·  {:.0} Hz  ·  {:+.1} dB",
                        self.language.tr("Band", "频段"),
                        index + 1,
                        band.frequency_hz.effective().unwrap_or(&0.0),
                        band.gain_db.effective().unwrap_or(&0.0)
                    ))
                    .color(TEXT),
                )
                .id_salt(("input-peq-band", self.session_generation, id, index))
                .show(ui, |ui| {
                    let (Some(mut frequency), Some(mut q), Some(mut gain)) = (
                        band.frequency_hz.effective().copied(),
                        band.q.effective().copied(),
                        band.gain_db.effective().copied(),
                    ) else {
                        ui.label(waiting_for_device_text(self.language));
                        return;
                    };
                    let changed = parameter_value_row(
                        ui,
                        self.language.tr("Frequency", "频率"),
                        &mut frequency,
                        specs::EQ_FREQUENCY_WIRE,
                    )
                    .changed()
                        | parameter_value_row(ui, "Q", &mut q, specs::EQ_Q_WIRE).changed()
                        | eq_gain_row(
                            ui,
                            self.language.tr("Gain", "增益"),
                            &mut gain,
                            self.language,
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
                });
            }
        }
    }

    fn input_dynamics_controls(
        &mut self,
        ui: &mut egui::Ui,
        id: InputId,
        channel: &InputChannelState,
    ) {
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
            if mute_button(ui, muted, self.language.tr("Mute", "静音")).clicked() {
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
    }
}
