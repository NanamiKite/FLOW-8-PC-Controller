use super::fx_setup_values;
use crate::app::Flow8App;
use crate::theme::{PURPLE, SECONDARY, card, inspector_heading};
use crate::widgets::mute_button;
use crate::widgets::{parameter_row, state_button};
use eframe::egui;
use flow8_core::{MuteTarget, SemanticCommand};
use flow8_model::{FX_PRESET_COUNT, FxId, FxState, fx_preset_info, specs};

impl Flow8App {
    pub(in crate::app) fn fx_output_inspector(
        &mut self,
        ui: &mut egui::Ui,
        fx_id: FxId,
        index: usize,
        effect: &FxState,
    ) {
        ui.add_space(8.0);
        card(ui, |ui| {
            self.fx_header_controls(ui, fx_id, effect);
            self.fx_preset_controls(ui, fx_id, index, effect);
            self.fx_pan_controls(ui, fx_id, effect);
            self.fx_setup_controls(ui, fx_id, effect);
            self.fx_tempo_controls(ui);
        });
    }

    fn fx_header_controls(&mut self, ui: &mut egui::Ui, fx_id: FxId, effect: &FxState) {
        inspector_heading(ui, self.language.tr("FX ENGINE", "效果器"), PURPLE);
        ui.horizontal(|ui| {
            let muted = effect.muted.effective().copied().unwrap_or(false);
            if mute_button(ui, muted, self.language.tr("Mute", "静音")).clicked() {
                self.dispatch(SemanticCommand::SetMuteTarget {
                    target: MuteTarget::Fx(fx_id),
                    enabled: !muted,
                });
            }
        });
    }

    fn fx_preset_controls(
        &mut self,
        ui: &mut egui::Ui,
        fx_id: FxId,
        index: usize,
        effect: &FxState,
    ) {
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
            ui.add_enabled_ui(self.device_controls_ready(), |ui| {
                egui::ComboBox::from_id_salt(("fx-preset", index))
                    .selected_text(preset_label)
                    .width(190.0 * self.metrics.ui_scale)
                    .show_ui(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .show(ui, |ui| {
                                for id in 0..FX_PRESET_COUNT as u8 {
                                    let info =
                                        fx_preset_info(fx_id, id).expect("preset ID in range");
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
    }

    fn fx_pan_controls(&mut self, ui: &mut egui::Ui, fx_id: FxId, effect: &FxState) {
        let mut pan = effect.pan.effective().copied().unwrap_or(0.0);
        if parameter_row(ui, self.language.tr("Pan", "声像"), &mut pan, specs::PAN).changed() {
            self.dispatch(SemanticCommand::SetFxPan {
                fx: fx_id,
                value: pan,
            });
        }
    }

    fn fx_setup_controls(&mut self, ui: &mut egui::Ui, fx_id: FxId, effect: &FxState) {
        let setup = fx_setup_values(effect);
        ui.add_enabled_ui(setup.is_some(), |ui| {
            // Placeholder values are only rendered in disabled controls;
            // a composite write requires every preserved field to be known.
            let (mut values, mut routes) = setup.unwrap_or(([0; 3], [false; 3]));
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
            ui.horizontal_wrapped(|ui| {
                for (index, label) in ["MAIN", "MON1", "MON2"].into_iter().enumerate() {
                    if state_button(ui, routes[index], label, PURPLE).clicked() {
                        routes[index] = !routes[index];
                        setup_changed = true;
                    }
                }
            });
            if setup_changed && setup.is_some() {
                self.dispatch(SemanticCommand::SetFxSetup {
                    fx: fx_id,
                    values,
                    return_to_main: routes[0],
                    return_to_mon1: routes[1],
                    return_to_mon2: routes[2],
                });
            }
        });
    }

    fn fx_tempo_controls(&mut self, ui: &mut egui::Ui) {
        let mut tempo = self
            .store
            .state
            .global_tempo_bpm
            .effective()
            .copied()
            .unwrap_or(specs::TEMPO_DISPLAY.default);
        if parameter_row(ui, "BPM", &mut tempo, specs::TEMPO_DISPLAY).changed() {
            self.dispatch(SemanticCommand::SetTempo {
                bpm: tempo.round() as u16,
            });
        }
    }
}
