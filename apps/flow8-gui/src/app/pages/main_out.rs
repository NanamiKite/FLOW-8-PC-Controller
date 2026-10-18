use crate::app::Flow8App;
use crate::i18n::{
    channel_number, destination_name, display_name, headphone_source_text, tap_point_text,
};
use crate::metrics::TOPOLOGY_CONTENT_MAX_WIDTH;
use crate::theme::{
    BLUE, GREEN, PURPLE, SECONDARY, TEXT, card_frame, section_heading, topology_bus_color,
};
use crate::widgets::fader_level_text;
use eframe::egui::containers::frame::Prepared;
use eframe::egui::{self, Align, FontId, Layout, Vec2};
use flow8_model::{FxId, InputId, MixDestination};

impl Flow8App {
    pub(in crate::app) fn main_out_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("main-out-topology-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(18.0);
                let content_width =
                    (ui.available_width() - 36.0).clamp(0.0, TOPOLOGY_CONTENT_MAX_WIDTH);
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
                            let scale = self.metrics.ui_scale;
                            let gap = 2.0 * self.metrics.spacing;
                            // Include each card's margins, the complete send matrix,
                            // and the longest output row before choosing two columns.
                            let sends_min = (200.0 + 5.0 * 88.0 + 5.0 * 8.0) * scale + 26.0;
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
                            // Measurement only, not a fallback device value.
                            let output_label = format!(
                                "MONITOR OUT 2 · −89.5 dB · {}",
                                self.language.tr("Muted", "已静音"),
                            );
                            let outputs_min = measure("MON2")
                                + measure("→")
                                + measure(&output_label)
                                + 28.0 * scale
                                + 26.0;
                            if content_width >= sends_min + outputs_min + gap {
                                let outputs_width = outputs_min
                                    .max(content_width * 0.30)
                                    .min(content_width - sends_min - gap)
                                    .floor();
                                let sends_width = content_width - outputs_width - gap;
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    let sends = ui
                                        .allocate_ui_with_layout(
                                            Vec2::new(sends_width, 0.0),
                                            Layout::top_down(Align::Min),
                                            |ui| {
                                                let card = self.topology_sends_card(ui);
                                                card.allocate_space(ui);
                                                card
                                            },
                                        )
                                        .inner;
                                    let outputs = ui
                                        .allocate_ui_with_layout(
                                            Vec2::new(outputs_width, 0.0),
                                            Layout::top_down(Align::Min),
                                            |ui| {
                                                let card = self.topology_outputs_card(ui);
                                                card.allocate_space(ui);
                                                card
                                            },
                                        )
                                        .inner;
                                    // Align only the frames to this row's natural
                                    // maximum; keep the content and spacing intact.
                                    let height = sends
                                        .content_ui
                                        .min_rect()
                                        .height()
                                        .max(outputs.content_ui.min_rect().height());
                                    for mut card in [sends, outputs] {
                                        let mut rect = card.content_ui.min_rect();
                                        rect.max.y = rect.min.y + height;
                                        card.content_ui.expand_to_include_rect(rect);
                                        card.paint(ui);
                                    }
                                });
                            } else {
                                self.topology_sends_card(ui).end(ui);
                                ui.add_space(12.0);
                                self.topology_outputs_card(ui).end(ui);
                            }
                        },
                    );
                });
            });
    }

    fn topology_sends_card(&self, ui: &mut egui::Ui) -> Prepared {
        let mut card = card_frame().begin(ui);
        let ui = &mut card.content_ui;
        section_heading(ui, self.language.tr("CHANNEL SENDS", "通道发送"), BLUE);
        ui.add_space(8.0);
        egui::ScrollArea::horizontal()
            .id_salt("main-out-channel-sends")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let row_height = self.metrics.control_height;
                let input_width = 200.0 * self.metrics.ui_scale;
                let bus_width =
                    ((ui.available_width() - input_width - 5.0 * 8.0 * self.metrics.ui_scale)
                        / 5.0)
                        .max(88.0 * self.metrics.ui_scale);
                egui::Grid::new("main-out-send-matrix")
                    .num_columns(6)
                    .striped(true)
                    .spacing(Vec2::new(
                        8.0 * self.metrics.ui_scale,
                        4.0 * self.metrics.ui_scale,
                    ))
                    .show(ui, |ui| {
                        ui.add_sized(
                            [input_width, row_height],
                            egui::Label::new(self.language.tr("INPUT", "输入通道")),
                        );
                        for destination in MixDestination::ALL {
                            ui.add_sized(
                                [bus_width, row_height],
                                egui::Label::new(
                                    egui::RichText::new(destination_name(destination))
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
                                .unwrap_or_else(|| display_name(channel.id, self.language));
                            let title = format!("{source} · {name}");
                            ui.add_sized(
                                [input_width, row_height],
                                egui::Label::new(egui::RichText::new(&title).color(TEXT))
                                    .truncate(),
                            )
                            .on_hover_text(title);
                            for destination in MixDestination::ALL {
                                let level = channel.route_levels[destination.index()]
                                    .effective()
                                    .copied();
                                let (text, color) = match level {
                                    Some(value) if value > 0.0 => (
                                        format!("{} dB", fader_level_text(value)),
                                        topology_bus_color(destination),
                                    ),
                                    Some(_) => ("−∞".to_owned(), SECONDARY),
                                    None => ("—".to_owned(), SECONDARY),
                                };
                                ui.add_sized(
                                    [bus_width, row_height],
                                    egui::Label::new(egui::RichText::new(text).color(color)),
                                );
                            }
                            ui.end_row();
                        }
                    });
            });
        card
    }

    fn topology_outputs_card(&self, ui: &mut egui::Ui) -> Prepared {
        let mut card = card_frame().begin(ui);
        let ui = &mut card.content_ui;
        ui.set_min_width(ui.available_width());
        section_heading(ui, self.language.tr("OUTPUTS", "物理输出"), GREEN);
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
                        .map(|value| format!("{} dB", fader_level_text(value)))
                        .unwrap_or_else(|| "—".to_owned());
                    let muted = bus.muted.effective().copied().unwrap_or(false);
                    let status = if muted {
                        format!("{sink} · {level} · {}", self.language.tr("Muted", "已静音"))
                    } else {
                        format!("{sink} · {level}")
                    };
                    ui.label(status);
                    ui.end_row();
                }
            });

        ui.separator();
        section_heading(ui, self.language.tr("FX RETURNS", "FX 返回"), PURPLE);
        egui::Grid::new("main-out-fx-returns")
            .num_columns(3)
            .spacing(Vec2::new(
                14.0 * self.metrics.ui_scale,
                7.0 * self.metrics.ui_scale,
            ))
            .show(ui, |ui| {
                for effect in &self.store.state.effects {
                    let source = match effect.id {
                        FxId::Fx1 => "FX1",
                        FxId::Fx2 => "FX2",
                    };
                    ui.label(egui::RichText::new(source).strong().color(PURPLE));
                    ui.label("→");
                    let returns = [
                        (effect.return_to_main.effective().copied(), "MAIN"),
                        (effect.return_to_mon1.effective().copied(), "MON1"),
                        (effect.return_to_mon2.effective().copied(), "MON2"),
                    ];
                    let destinations: Vec<_> = returns
                        .iter()
                        .filter_map(|(enabled, bus)| (*enabled == Some(true)).then_some(*bus))
                        .collect();
                    let text = if !destinations.is_empty() {
                        destinations.join(" · ")
                    } else if returns.iter().any(|(enabled, _)| enabled.is_none()) {
                        "—".to_owned()
                    } else {
                        self.language.tr("No return", "未设置返回").to_owned()
                    };
                    ui.label(text);
                    ui.end_row();
                }
            });

        ui.separator();
        section_heading(ui, self.language.tr("HEADPHONES", "耳机"), GREEN);
        egui::Grid::new("main-out-headphones")
            .num_columns(2)
            .spacing(Vec2::new(
                14.0 * self.metrics.ui_scale,
                7.0 * self.metrics.ui_scale,
            ))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(self.language.tr("Source", "来源")).color(SECONDARY));
                let headphones = &self.store.state.routing.headphones;
                let source = headphones
                    .source
                    .effective()
                    .copied()
                    .map(|source| headphone_source_text(source, self.language))
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
        card
    }
}
