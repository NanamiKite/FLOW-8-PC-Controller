use super::inspector::fx_setup_values;
use crate::app::Flow8App;
use crate::i18n::{Language, destination_name, waiting_for_device_text};
use crate::metrics::{ROUTING_CONTENT_MAX_WIDTH, STATE_BUTTON_MIN_WIDTH, UiMetrics};
use crate::theme::{
    BLUE, GREEN, MIXER_DIVIDER, PURPLE, SECONDARY, TEXT, card_frame, section_heading,
};
use crate::widgets::state_button_sized;
use eframe::egui::containers::frame::Prepared;
use eframe::egui::{self, Align, Color32, FontId, Layout, Vec2};
use flow8_core::{KnownSetting, SemanticCommand, SessionState};
use flow8_model::{
    DELAY_TICKS_PER_MILLISECOND, FxId, HeadphoneSource, MixBusId, MixDestination,
    MonitorRoutingSource, TapPoint,
};

impl Flow8App {
    pub(in crate::app) fn routing_page(&mut self, ui: &mut egui::Ui) {
        let width = (ui.available_width() - 48.0).clamp(0.0, ROUTING_CONTENT_MAX_WIDTH);
        let layout = RoutingLayout::new(ui, width, self.metrics, self.language);
        ui.add_space(20.0);
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.allocate_ui_with_layout(Vec2::new(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.heading(
                    egui::RichText::new(self.language.tr("Routing & Outputs", "路由与输出"))
                        .color(TEXT),
                );
                let subtitle = if self.device_controls_ready() {
                    self.language.tr(
                        "Audio sources, monitoring and physical outputs",
                        "音频来源、监听与物理输出",
                    )
                } else if matches!(
                    self.store.state.session,
                    SessionState::Disconnected | SessionState::Error
                ) {
                    self.language.tr(
                        "Connect to FLOW 8 to view and change device routing.",
                        "连接 FLOW 8 后，可查看并调整设备路由。",
                    )
                } else {
                    self.language.tr(
                        "Device routing will be available after synchronization.",
                        "设备路由将在同步完成后显示。",
                    )
                };
                ui.label(egui::RichText::new(subtitle).color(SECONDARY));
                ui.add_space(12.0);
                self.routing_card_pair(
                    ui,
                    layout,
                    Self::usb_routing_card,
                    Self::monitor_routing_card,
                );
                ui.add_space(layout.gap);
                self.routing_card_pair(
                    ui,
                    layout,
                    Self::fx_return_routing_card,
                    Self::physical_output_routing_card,
                );
            });
        });
    }

    fn routing_card_pair(
        &mut self,
        ui: &mut egui::Ui,
        layout: RoutingLayout,
        left: fn(&mut Self, &mut egui::Ui, RoutingLayout) -> Prepared,
        right: fn(&mut Self, &mut egui::Ui, RoutingLayout) -> Prepared,
    ) {
        if layout.two_columns {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = layout.gap;
                let mut cards = Vec::with_capacity(2);
                for render in [left, right] {
                    let card = ui
                        .allocate_ui_with_layout(
                            Vec2::new(layout.column_width, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                let card = render(self, ui, layout);
                                card.allocate_space(ui);
                                card
                            },
                        )
                        .inner;
                    cards.push(card);
                }
                // Both bodies are laid out once. Align only this row's frames
                // to their natural maximum, without a fixed card height.
                let height = cards
                    .iter()
                    .map(|card| card.content_ui.min_rect().height())
                    .fold(0.0, f32::max);
                for mut card in cards {
                    let mut content_rect = card.content_ui.min_rect();
                    content_rect.max.y = content_rect.min.y + height;
                    card.content_ui.expand_to_include_rect(content_rect);
                    card.paint(ui);
                }
            });
        } else {
            left(self, ui, layout).end(ui);
            ui.add_space(layout.gap);
            right(self, ui, layout).end(ui);
        }
    }

    fn usb_routing_card(&mut self, ui: &mut egui::Ui, layout: RoutingLayout) -> Prepared {
        let ready = self.device_controls_ready();
        let language = self.language;
        let settings = self.store.state.routing.settings.clone();
        routing_card(
            ui,
            layout,
            language.tr("USB / BLUETOOTH", "USB / 蓝牙"),
            BLUE,
            |ui| {
                let streaming = settings.usb_streaming.effective().copied();
                if let Some(value) = setting_row(
                    ui,
                    layout,
                    language.tr("Recording / Streaming", "录音 / 推流"),
                    |ui| routing_switch(ui, layout, streaming, ready, language, BLUE),
                ) {
                    self.dispatch(SemanticCommand::SetSetting(KnownSetting::UsbStreaming(
                        value,
                    )));
                }
                ui.separator();
                for (source, destination, current, setting) in [
                    (
                        "USB 1/2",
                        language.tr("Input 5/6", "输入 5/6"),
                        settings.input56_from_usb12.effective().copied(),
                        KnownSetting::Input56FromUsb12 as fn(bool) -> KnownSetting,
                    ),
                    (
                        "USB 3/4",
                        language.tr("Input 7/8", "输入 7/8"),
                        settings.input78_from_usb34.effective().copied(),
                        KnownSetting::Input78FromUsb34 as fn(bool) -> KnownSetting,
                    ),
                ] {
                    if let Some(value) =
                        route_row(ui, layout, source, destination, layout.switch_width, |ui| {
                            routing_switch(ui, layout, current, ready, language, BLUE)
                        })
                    {
                        self.dispatch(SemanticCommand::SetSetting(setting(value)));
                    }
                }
                ui.separator();
                for (label, current, setting) in [
                    (
                        language.tr("Bluetooth / USB to Headphones Only", "蓝牙 / USB 仅送耳机"),
                        settings.phones_only.effective().copied(),
                        KnownSetting::PhonesOnly as fn(bool) -> KnownSetting,
                    ),
                    (
                        language.tr("Bluetooth / USB source switch", "蓝牙 / USB 信号源切换"),
                        settings.bt_usb_switch.effective().copied(),
                        KnownSetting::BtUsbSwitch as fn(bool) -> KnownSetting,
                    ),
                ] {
                    if let Some(value) = setting_row(ui, layout, label, |ui| {
                        routing_switch(ui, layout, current, ready, language, BLUE)
                    }) {
                        self.dispatch(SemanticCommand::SetSetting(setting(value)));
                    }
                }
            },
        )
    }

    fn monitor_routing_card(&mut self, ui: &mut egui::Ui, layout: RoutingLayout) -> Prepared {
        let ready = self.device_controls_ready();
        let language = self.language;
        let routing = &self.store.state.routing;
        let linked = routing.monitor_link.stereo_linked.effective().copied();
        let source = routing.headphones.source.effective().copied();
        let tap = routing.headphones.tap_point.effective().copied();
        let monitor_source = routing.settings.monitor_routing.effective().copied();
        let monitor_post = routing.settings.monitor_post_fader.effective().copied();
        routing_card(
            ui,
            layout,
            language.tr("MONITOR / HEADPHONES", "监听 / 耳机"),
            GREEN,
            |ui| {
                if let Some(value) = setting_row(
                    ui,
                    layout,
                    language.tr("Monitor source", "监听来源"),
                    |ui| {
                        routing_choice(
                            ui,
                            layout,
                            ready,
                            monitor_source,
                            [
                                (MonitorRoutingSource::MonitorMix, "MON"),
                                (MonitorRoutingSource::Usb12, "USB 1/2"),
                                (MonitorRoutingSource::Usb34, "USB 3/4"),
                            ],
                            GREEN,
                        )
                    },
                ) {
                    self.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorRouting(
                        value,
                    )));
                }
                if let Some(value) = setting_row(
                    ui,
                    layout,
                    language.tr("Monitor tap", "监听取样点"),
                    |ui| routing_toggle(ui, layout, monitor_post, ready, ["PRE", "POST"], GREEN),
                ) {
                    self.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorPostFader(
                        value,
                    )));
                }
                if let Some(value) = setting_row(
                    ui,
                    layout,
                    language.tr("MON1/2 Stereo Link", "MON1/2 立体声链接"),
                    |ui| routing_switch(ui, layout, linked, ready, language, GREEN),
                ) {
                    self.dispatch(SemanticCommand::SetSetting(
                        KnownSetting::MonitorStereoLink(value),
                    ));
                }
                ui.separator();
                if let Some(value) = setting_row(
                    ui,
                    layout,
                    language.tr("Headphone source", "耳机来源"),
                    |ui| {
                        routing_choice(
                            ui,
                            layout,
                            ready,
                            source,
                            [
                                (HeadphoneSource::Main, "MAIN"),
                                (HeadphoneSource::Monitor, "MONITOR"),
                            ],
                            GREEN,
                        )
                    },
                ) {
                    self.dispatch(SemanticCommand::SetSetting(
                        KnownSetting::HeadphonesUseMonitor(value == HeadphoneSource::Monitor),
                    ));
                }
                if let Some(value) = setting_row(
                    ui,
                    layout,
                    language.tr("Headphone tap", "耳机取样点"),
                    |ui| {
                        routing_choice(
                            ui,
                            layout,
                            ready,
                            tap,
                            [(TapPoint::PreFader, "PRE"), (TapPoint::PostFader, "POST")],
                            GREEN,
                        )
                    },
                ) {
                    self.dispatch(SemanticCommand::SetSetting(
                        KnownSetting::HeadphonesPostFader(value == TapPoint::PostFader),
                    ));
                }
            },
        )
    }

    fn fx_return_routing_card(&mut self, ui: &mut egui::Ui, layout: RoutingLayout) -> Prepared {
        let ready = self.device_controls_ready();
        let language = self.language;
        routing_card(
            ui,
            layout,
            language.tr("FX RETURNS", "FX 返回路由"),
            PURPLE,
            |ui| {
                let mut unavailable = false;
                for (index, fx) in [FxId::Fx1, FxId::Fx2].into_iter().enumerate() {
                    let setup = fx_setup_values(&self.store.state.effects[index]);
                    unavailable |= setup.is_none();
                    let mut routes = setup.map(|(_, routes)| routes);
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        route_source(ui, layout, if index == 0 { "FX 1" } else { "FX 2" });
                        ui.add_enabled_ui(ready && setup.is_some(), |ui| {
                            ui.horizontal(|ui| {
                                for (route, label) in
                                    ["MAIN", "MON1", "MON2"].into_iter().enumerate()
                                {
                                    let selected = routes.is_some_and(|values| values[route]);
                                    if state_button_sized(
                                        ui,
                                        selected,
                                        label,
                                        PURPLE,
                                        Vec2::new(layout.fx_button_width, layout.row_height),
                                    )
                                    .clicked()
                                        && let Some(values) = routes.as_mut()
                                    {
                                        values[route] = !values[route];
                                        changed = true;
                                    }
                                }
                            });
                        });
                    });
                    if changed && let (Some((values, _)), Some(routes)) = (setup, routes) {
                        self.dispatch(SemanticCommand::SetFxSetup {
                            fx,
                            values,
                            return_to_main: routes[0],
                            return_to_mon1: routes[1],
                            return_to_mon2: routes[2],
                        });
                    }
                }
                if unavailable {
                    ui.label(
                        egui::RichText::new(waiting_for_device_text(language))
                            .size(layout.secondary_font)
                            .color(SECONDARY),
                    );
                }
            },
        )
    }

    fn output_delay_settings(&mut self, ui: &mut egui::Ui, layout: RoutingLayout) {
        let ready = self.device_controls_ready();
        let layout = RoutingLayout {
            label_width: layout.output_label_width,
            ..layout
        };
        section_heading(ui, self.language.tr("OUTPUT DELAY", "输出延迟"), TEXT);
        for (index, bus, destination) in [
            (0, MixBusId::Main, MixDestination::Main),
            (1, MixBusId::Monitor1, MixDestination::Monitor1),
            (2, MixBusId::Monitor2, MixDestination::Monitor2),
        ] {
            let delay = &self
                .store
                .state
                .bus_for_destination(destination)
                .expect("MAIN and MON outputs have bus state")
                .delay_ticks;
            let current_ticks = delay.effective().copied();
            let confirmed_ticks = delay.confirmed;
            if !self.delay_ms_dirty[index] {
                self.delay_ms_draft[index] = current_ticks.map(format_delay_ms).unwrap_or_default();
            }
            setting_row(ui, layout, destination_name(destination), |ui| {
                ui.add_enabled_ui(ready, |ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.delay_ms_draft[index])
                                .id_salt(("output-delay-ms", index))
                                .hint_text("—")
                                .desired_width(100.0 * self.metrics.ui_scale),
                        )
                        .changed()
                    {
                        self.delay_ms_dirty[index] = true;
                    }
                    ui.label("ms");
                    if ui
                        .add_enabled(
                            self.delay_ms_dirty[index],
                            egui::Button::new(self.language.tr("Apply", "应用")),
                        )
                        .clicked()
                    {
                        match parse_delay_ms(&self.delay_ms_draft[index]) {
                            Some(ticks) => {
                                if current_ticks != Some(ticks) {
                                    self.dispatch(SemanticCommand::SetDelay { bus, ticks });
                                }
                                self.delay_ms_dirty[index] = false;
                            }
                            None => {
                                self.message = self
                                    .language
                                    .tr(
                                        "Enter a valid non-negative delay.",
                                        "请输入有效的非负延迟值。",
                                    )
                                    .into();
                            }
                        }
                    }
                });
            });
            if let Some(ticks) = confirmed_ticks {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {} ms · {} ticks",
                        self.language.tr("Device reported", "设备回报"),
                        format_delay_ms(ticks),
                        ticks,
                    ))
                    .size(self.metrics.small_font)
                    .color(SECONDARY),
                );
            }
        }
    }

    fn physical_output_routing_card(
        &mut self,
        ui: &mut egui::Ui,
        layout: RoutingLayout,
    ) -> Prepared {
        let ready = self.device_controls_ready();
        let language = self.language;
        let settings = self.store.state.routing.settings.clone();
        routing_card(
            ui,
            layout,
            language.tr("OUTPUTS", "物理输出"),
            SECONDARY,
            |ui| {
                let gap = layout.gap;
                let group_width = (ui.available_width() - 2.0 * gap - 1.0).max(0.0);
                let route_width = (group_width * 0.55)
                    .max(layout.output_route_min_width)
                    .min((group_width - layout.output_level_min_width).max(0.0));
                let level_width = group_width - route_width;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    let routes_response = ui.allocate_ui_with_layout(
                        Vec2::new(route_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = layout.control_gap;
                            ui.set_width(route_width);
                            section_heading(ui, language.tr("Output sockets", "输出接口"), TEXT);
                            let routes = RoutingLayout {
                                row_height: ui.text_style_height(&egui::TextStyle::Body).ceil(),
                                ..layout
                            };
                            for (source, destination) in [
                                ("MAIN", language.tr("MAIN OUT", "主输出")),
                                ("MON1", language.tr("MONITOR OUT 1", "监听输出 1")),
                                ("MON2", language.tr("MONITOR OUT 2", "监听输出 2")),
                            ] {
                                route_row(ui, routes, source, destination, 0.0, |_| ());
                            }
                        },
                    );
                    let (divider, _) =
                        ui.allocate_exact_size(Vec2::new(1.0, 0.0), egui::Sense::hover());
                    let levels_response = ui.allocate_ui_with_layout(
                        Vec2::new(level_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = layout.control_gap;
                            ui.set_width(level_width);
                            section_heading(ui, language.tr("Output level", "输出电平"), TEXT);
                            let levels = RoutingLayout {
                                label_width: layout.output_label_width,
                                ..layout
                            };
                            for (label, current, setting) in [
                                (
                                    "MAIN",
                                    settings.main_minus_10_dbv.effective().copied(),
                                    KnownSetting::MainMinus10Dbv as fn(bool) -> KnownSetting,
                                ),
                                (
                                    "MON1/2",
                                    settings.monitor_minus_10_dbv.effective().copied(),
                                    KnownSetting::MonitorMinus10Dbv as fn(bool) -> KnownSetting,
                                ),
                            ] {
                                if let Some(value) = setting_row(ui, levels, label, |ui| {
                                    output_level_switch(ui, layout, current, ready, language)
                                }) {
                                    self.dispatch(SemanticCommand::SetSetting(setting(value)));
                                }
                            }
                        },
                    );
                    let sections = routes_response
                        .response
                        .rect
                        .union(levels_response.response.rect);
                    ui.painter().vline(
                        divider.center().x,
                        sections.y_range(),
                        egui::Stroke::new(1.0, MIXER_DIVIDER),
                    );
                });
                ui.separator();
                self.output_delay_settings(ui, layout);
            },
        )
    }
}

const CARD_INSET: f32 = 16.0;

fn format_delay_ms(ticks: u32) -> String {
    format!("{:.3}", ticks as f64 / DELAY_TICKS_PER_MILLISECOND)
}

fn parse_delay_ms(value: &str) -> Option<u32> {
    let milliseconds = value.trim().parse::<f64>().ok()?;
    let ticks = (milliseconds * DELAY_TICKS_PER_MILLISECOND).round();
    (milliseconds.is_finite() && milliseconds >= 0.0 && ticks <= u32::MAX as f64)
        .then_some(ticks as u32)
}

#[derive(Clone, Copy)]
struct RoutingLayout {
    two_columns: bool,
    column_width: f32,
    label_width: f32,
    source_width: f32,
    arrow_width: f32,
    switch_width: f32,
    choice_width: f32,
    fx_button_width: f32,
    output_label_width: f32,
    output_route_min_width: f32,
    output_level_min_width: f32,
    row_height: f32,
    secondary_font: f32,
    control_gap: f32,
    gap: f32,
}

impl RoutingLayout {
    fn new(ui: &egui::Ui, width: f32, metrics: UiMetrics, language: Language) -> Self {
        let measure = |text: &str| {
            ui.painter()
                .layout_no_wrap(
                    text.to_owned(),
                    FontId::proportional(metrics.body_font),
                    TEXT,
                )
                .size()
                .x
        };
        let gap = (16.0 * metrics.ui_scale).round();
        let label_width = [
            language.tr("Recording / Streaming", "录音 / 推流"),
            language.tr("MON1/2 Stereo Link", "MON1/2 立体声链接"),
            language.tr("Headphone source", "耳机来源"),
            language.tr("Headphone tap", "耳机取样点"),
            language.tr("Monitor source", "监听来源"),
            language.tr("Monitor tap", "监听取样点"),
            language.tr("Bluetooth / USB to Headphones Only", "蓝牙 / USB 仅送耳机"),
            language.tr("Bluetooth / USB source switch", "蓝牙 / USB 信号源切换"),
        ]
        .into_iter()
        .map(measure)
        .fold(0.0, f32::max)
        .min(220.0 * metrics.ui_scale)
            + gap;
        let button_padding = 2.0 * ui.spacing().button_padding.x + 4.0;
        let choice_width = (measure("MONITOR").max(measure("USB 3/4")) + button_padding)
            .ceil()
            .max(STATE_BUTTON_MIN_WIDTH);
        let choices_width = 3.0 * choice_width + 2.0 * metrics.spacing;
        let source_width = measure("USB 3/4").max(84.0 * metrics.ui_scale);
        let arrow_width = 28.0 * metrics.ui_scale;
        let fx_button_width = ["MAIN", "MON1", "MON2"]
            .into_iter()
            .map(measure)
            .fold(STATE_BUTTON_MIN_WIDTH - button_padding, f32::max)
            + button_padding;
        let fx_choices_width = 3.0 * fx_button_width + 2.0 * metrics.spacing;
        let output_label_width = measure("MON1/2").ceil();
        let switch_width = choice_width.max(measure("−10 dBV") + button_padding).ceil();
        let output_route_min_width = source_width
            + arrow_width
            + measure(language.tr("MONITOR OUT 2", "监听输出 2"))
            + 3.0 * metrics.spacing;
        let output_level_min_width = (output_label_width + switch_width + metrics.spacing)
            .max(measure(language.tr("Output level", "输出电平")));
        let output_width = output_route_min_width + output_level_min_width + 2.0 * gap + 1.0;
        let min_column = (label_width + choices_width + metrics.spacing)
            .max(source_width + arrow_width + fx_choices_width + 3.0 * metrics.spacing)
            .max(output_width)
            + 2.0 * (CARD_INSET + 1.0);
        let two_columns = width >= 2.0 * min_column + gap;
        Self {
            two_columns,
            column_width: if two_columns {
                ((width - gap) / 2.0).floor()
            } else {
                width
            },
            label_width,
            source_width,
            arrow_width,
            switch_width,
            choice_width,
            fx_button_width,
            output_label_width,
            output_route_min_width,
            output_level_min_width,
            row_height: metrics.control_height,
            secondary_font: metrics.secondary_font.max(12.0),
            control_gap: metrics.spacing,
            gap,
        }
    }
}

fn routing_card(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    title: &str,
    accent: Color32,
    content: impl FnOnce(&mut egui::Ui),
) -> Prepared {
    let mut card = card_frame()
        .corner_radius(6)
        .inner_margin(CARD_INSET as i8)
        .begin(ui);
    let ui = &mut card.content_ui;
    ui.spacing_mut().item_spacing.x = layout.control_gap;
    ui.set_width((layout.column_width - 2.0 * (CARD_INSET + 1.0)).max(0.0));
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 1.0, accent);
        ui.label(egui::RichText::new(title).strong().color(TEXT));
    });
    ui.add_space(6.0);
    content(ui);
    card
}

fn setting_row<R>(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    label: &str,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(layout.label_width, layout.row_height),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_max_width(layout.label_width);
                ui.set_min_size(Vec2::new(layout.label_width, layout.row_height));
                ui.add(egui::Label::new(egui::RichText::new(label).color(TEXT)).wrap());
            },
        );
        control(ui)
    })
    .inner
}

fn route_source(ui: &mut egui::Ui, layout: RoutingLayout, source: &str) {
    ui.allocate_ui_with_layout(
        Vec2::new(layout.source_width, layout.row_height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(layout.source_width, layout.row_height));
            ui.label(egui::RichText::new(source).strong().color(TEXT));
        },
    );
    ui.add_sized(
        Vec2::new(layout.arrow_width, layout.row_height),
        egui::Label::new(egui::RichText::new("→").color(SECONDARY)),
    );
}

fn route_row<R>(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    source: &str,
    destination: &str,
    control_width: f32,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        route_source(ui, layout, source);
        let destination_width =
            (ui.available_width() - control_width - ui.spacing().item_spacing.x).max(0.0);
        ui.allocate_ui_with_layout(
            Vec2::new(destination_width, layout.row_height),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_size(Vec2::new(destination_width, layout.row_height));
                ui.add(egui::Label::new(egui::RichText::new(destination).color(TEXT)).truncate())
                    .on_hover_text(destination);
            },
        );
        control(ui)
    })
    .inner
}

fn output_level_switch(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    value: Option<bool>,
    ready: bool,
    language: Language,
) -> Option<bool> {
    let (label, status) = match value {
        Some(true) => ("−10 dBV", language.tr("On", "开启")),
        Some(false) => ("−10 dBV", language.tr("Off", "关闭")),
        None => ("—", waiting_for_device_text(language)),
    };
    let clicked = ui
        .add_enabled_ui(ready && value.is_some(), |ui| {
            state_button_sized(
                ui,
                value == Some(true),
                label,
                TEXT,
                Vec2::new(layout.switch_width, layout.row_height),
            )
            .on_hover_text(status)
            .on_disabled_hover_text(status)
            .clicked()
        })
        .inner;
    if clicked {
        value.map(|value| !value)
    } else {
        None
    }
}

fn routing_switch(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    value: Option<bool>,
    ready: bool,
    language: Language,
    accent: Color32,
) -> Option<bool> {
    routing_toggle(
        ui,
        layout,
        value,
        ready,
        [language.tr("Off", "关闭"), language.tr("On", "开启")],
        accent,
    )
}

fn routing_toggle(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    value: Option<bool>,
    ready: bool,
    labels: [&str; 2],
    accent: Color32,
) -> Option<bool> {
    let label = value.map_or("—", |value| labels[usize::from(value)]);
    let clicked = ui
        .add_enabled_ui(ready && value.is_some(), |ui| {
            state_button_sized(
                ui,
                value == Some(true),
                label,
                accent,
                Vec2::new(layout.switch_width, layout.row_height),
            )
            .clicked()
        })
        .inner;
    if clicked {
        value.map(|value| !value)
    } else {
        None
    }
}

fn routing_choice<T: Copy + PartialEq, const N: usize>(
    ui: &mut egui::Ui,
    layout: RoutingLayout,
    ready: bool,
    value: Option<T>,
    choices: [(T, &str); N],
    accent: Color32,
) -> Option<T> {
    let mut selected = None;
    ui.add_enabled_ui(ready && value.is_some(), |ui| {
        ui.horizontal(|ui| {
            for (candidate, label) in choices {
                if state_button_sized(
                    ui,
                    value == Some(candidate),
                    label,
                    accent,
                    Vec2::new(layout.choice_width, layout.row_height),
                )
                .clicked()
                {
                    selected = Some(candidate);
                }
            }
        });
    });
    selected
}
