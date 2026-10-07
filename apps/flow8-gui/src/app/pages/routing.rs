use super::inspector::fx_setup_values;
use crate::app::Flow8App;
use crate::i18n::waiting_for_device_text;
use crate::theme::{BLUE, GREEN, PURPLE, TEXT, card, section_heading};
use crate::widgets::state_button;
use eframe::egui;
use flow8_core::{KnownSetting, SemanticCommand};
use flow8_model::{FxId, HeadphoneSource, TapPoint};

impl Flow8App {
    pub(in crate::app) fn routing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.heading(egui::RichText::new(self.language.tr("Routing", "路由")).color(TEXT));
                ui.add_space(12.0);
                ui.columns(2, |columns| {
                    card(&mut columns[0], |ui| {
                        section_heading(ui, self.language.tr("USB AUDIO", "USB 音频"), BLUE);
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
                        section_heading(ui, self.language.tr("FX RETURNS", "FX 返回路由"), PURPLE);
                        for (index, fx_id) in [FxId::Fx1, FxId::Fx2].into_iter().enumerate() {
                            let effect = self.store.state.effects[index].clone();
                            ui.label(format!("FX {}", index + 1));
                            let Some((values, mut routes)) = fx_setup_values(&effect) else {
                                ui.weak(waiting_for_device_text(self.language));
                                continue;
                            };
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
                                    values,
                                    return_to_main: routes[0],
                                    return_to_mon1: routes[1],
                                    return_to_mon2: routes[2],
                                });
                            }
                        }
                    });
                    card(&mut columns[1], |ui| {
                        section_heading(ui, self.language.tr("OUTPUTS", "物理输出"), TEXT);
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
