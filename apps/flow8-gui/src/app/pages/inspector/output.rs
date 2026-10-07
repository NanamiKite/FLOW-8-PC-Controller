use crate::app::Flow8App;
use crate::i18n::waiting_for_device_text;
use crate::theme::{GREEN, SECONDARY, card, inspector_heading};
use crate::widgets::mute_button;
use crate::widgets::{output_eq_graph, parameter_row, state_button};
use eframe::egui;
use flow8_core::{KnownSetting, MuteTarget, SemanticCommand};
use flow8_model::DELAY_TICKS_PER_MILLISECOND;
use flow8_model::{MixBusId, MixBusState, specs};

impl Flow8App {
    pub(in crate::app) fn bus_output_inspector(
        &mut self,
        ui: &mut egui::Ui,
        bus_id: MixBusId,
        bus: &MixBusState,
    ) {
        ui.add_space(8.0);
        card(ui, |ui| {
            self.bus_header_controls(ui, bus_id, bus);
            self.bus_dynamics_controls(ui, bus_id, bus);
            self.bus_eq_controls(ui, bus_id, bus);
            self.bus_delay_indicator(ui, bus);
        });
    }

    fn bus_header_controls(&mut self, ui: &mut egui::Ui, bus_id: MixBusId, bus: &MixBusState) {
        inspector_heading(
            ui,
            self.language.tr("OUTPUT PROCESSING", "输出处理"),
            SECONDARY,
        );
        ui.horizontal(|ui| {
            let muted = bus.muted.effective().copied().unwrap_or(false);
            if mute_button(ui, muted, self.language.tr("Mute", "静音")).clicked() {
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
    }

    fn bus_dynamics_controls(&mut self, ui: &mut egui::Ui, bus_id: MixBusId, bus: &MixBusState) {
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
    }

    fn bus_eq_controls(&mut self, ui: &mut egui::Ui, bus_id: MixBusId, bus: &MixBusState) {
        ui.separator();
        ui.label(egui::RichText::new(self.language.tr("9-band GEQ", "9 段图示均衡")).strong());
        if let Some((index, gain_db)) = output_eq_graph(ui, &bus.eq, self.metrics) {
            let band = &bus.eq.bands[index];
            if let (Some(frequency), Some(q)) = (band.frequency_hz.effective(), band.q.effective())
            {
                self.dispatch(SemanticCommand::SetGeqBand {
                    bus: bus_id,
                    band: index as u8,
                    frequency_hz: *frequency as u16,
                    q: *q,
                    gain_db,
                });
            }
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
                let (Some(frequency), Some(q), Some(mut gain)) = (
                    band.frequency_hz.effective().copied(),
                    band.q.effective().copied(),
                    band.gain_db.effective().copied(),
                ) else {
                    ui.label(waiting_for_device_text(self.language));
                    continue;
                };
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
                        frequency_hz: frequency as u16,
                        q,
                        gain_db: gain,
                    });
                }
            }
        });
    }

    fn bus_delay_indicator(&mut self, ui: &mut egui::Ui, bus: &MixBusState) {
        if self.preferences.show_output_delay_indicator {
            let ticks = bus.delay_ticks.effective().copied().unwrap_or(0);
            ui.label(
                egui::RichText::new(format!(
                    "{}: {} ticks · {:.2} ms",
                    self.language.tr("Delay", "延迟"),
                    ticks,
                    ticks as f32 / DELAY_TICKS_PER_MILLISECOND as f32
                ))
                .size(10.0)
                .color(SECONDARY),
            );
        }
    }
}
