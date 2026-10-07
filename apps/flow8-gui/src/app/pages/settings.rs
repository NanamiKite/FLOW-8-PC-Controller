use crate::app::{ControlGesture, EqEditingMode, Flow8App};
use crate::i18n::{
    Language, destination_name, display_name, localized_status_message, native_stage_text,
    session_state_text, setting_switch_text,
};
use crate::metrics::{SETTINGS_CONTENT_MAX_WIDTH, SettingsGridLayout, UI_SCALE_MAX, UI_SCALE_MIN};
use crate::theme::{BLUE, GREEN, RED, SECONDARY, TEXT, YELLOW, card, section_heading};
use crate::widgets::{committed_text, state_button};
use eframe::egui::{self, Align, Layout, Vec2};
use flow8_core::{KnownSetting, SemanticCommand};
use flow8_model::{DELAY_TICKS_PER_MILLISECOND, DEVICE_NAME_MAX_BYTES};
use flow8_model::{
    DeviceSettingsState, HeadphoneSource, InputId, MixBusId, MixDestination, MonitorRoutingSource,
    TapPoint,
};

impl Flow8App {
    pub(in crate::app) fn settings_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(18.0);
                let content_width =
                    (ui.available_width() - 36.0).clamp(0.0, SETTINGS_CONTENT_MAX_WIDTH);
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(content_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.heading(
                                egui::RichText::new(self.language.tr("Preferences", "偏好设置"))
                                    .color(TEXT),
                            );
                            ui.label(
                                egui::RichText::new(
                                    self.language
                                        .tr("Application and device settings", "应用与设备设置"),
                                )
                                .color(SECONDARY),
                            );
                            ui.add_space(10.0);
                            let settings = self.store.state.routing.settings.clone();
                            let grid = SettingsGridLayout::calculate(
                                ui,
                                content_width,
                                self.metrics,
                                self.language,
                            );

                            settings_grid_row(
                                self,
                                ui,
                                grid,
                                0,
                                |this, ui| {
                                    this.application_settings_card(ui, grid);
                                },
                                |this, ui| {
                                    this.mixer_display_settings_card(ui, grid);
                                },
                            );

                            let device_ready = self.device_controls_ready();
                            ui.add_enabled_ui(device_ready, |ui| {
                                ui.add_space(grid.gap);
                                settings_grid_row(
                                    self,
                                    ui,
                                    grid,
                                    1,
                                    |this, ui| {
                                        this.device_settings_card(ui, grid, &settings);
                                    },
                                    |this, ui| {
                                        this.monitor_settings_card(ui, grid, &settings);
                                    },
                                );

                                ui.add_space(grid.gap);
                                settings_grid_row(
                                    self,
                                    ui,
                                    grid,
                                    2,
                                    |this, ui| {
                                        this.usb_headphones_settings_card(ui, grid, &settings);
                                    },
                                    |this, ui| {
                                        this.output_settings_card(ui, grid, &settings);
                                    },
                                );
                            });

                            if !self.discovered_devices.is_empty() {
                                ui.add_space(grid.gap);
                                self.last_scan_settings_card(ui);
                            }
                        },
                    );
                });
            });
    }

    fn application_settings_card(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        settings_card(ui, grid, |ui| {
            self.application_preferences_section(ui, grid);
            self.connection_settings_section(ui, grid);
        })
    }

    fn application_preferences_section(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        section_heading(ui, self.language.tr("APPLICATION", "应用"), BLUE);
        settings_form_row(ui, grid, self.language.tr("Language", "语言"), |ui| {
            if ui
                .selectable_label(self.language == Language::English, "English")
                .clicked()
            {
                self.language = Language::English;
                self.message = localized_status_message(self.store.state.session, self.language);
                ui.ctx().request_repaint();
            }
            if ui
                .selectable_label(self.language == Language::Chinese, "简体中文")
                .clicked()
            {
                self.language = Language::Chinese;
                self.message = localized_status_message(self.store.state.session, self.language);
                ui.ctx().request_repaint();
            }
        });
        settings_form_row(
            ui,
            grid,
            self.language.tr("UI Scale", "界面缩放"),
            |ui| {
                let response = ui.add(
                    egui::Slider::new(&mut self.preferences.ui_scale, UI_SCALE_MIN..=UI_SCALE_MAX)
                        .step_by(0.05)
                        .custom_formatter(|value, _| format!("{:.0}%", value * 100.0))
                        .show_value(true),
                );
                if response.changed() {
                    ui.ctx().request_repaint();
                }
            },
        );
        ui.add_enabled_ui(false, |ui| {
            settings_form_row(
                ui,
                grid,
                self.language.tr("Control Gesture", "控制手势"),
                |ui| {
                    for (mode, label) in [
                        (ControlGesture::Linear, self.language.tr("Linear", "线性")),
                        (ControlGesture::Rotary, self.language.tr("Rotary", "旋转")),
                    ] {
                        let _ =
                            ui.selectable_label(self.preferences.control_gesture == mode, label);
                    }
                },
            );
            settings_form_row(
                ui,
                grid,
                self.language.tr("EQ Editing Mode", "EQ 编辑模式"),
                |ui| {
                    for (mode, label) in [
                        (
                            EqEditingMode::Standard,
                            self.language.tr("Standard", "标准"),
                        ),
                        (
                            EqEditingMode::Parametric,
                            self.language.tr("Parametric", "参数"),
                        ),
                    ] {
                        let _ =
                            ui.selectable_label(self.preferences.eq_editing_mode == mode, label);
                    }
                },
            );
        });
    }

    fn connection_settings_section(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        ui.separator();
        section_heading(ui, self.language.tr("CONNECTION", "连接"), BLUE);
        settings_form_row(ui, grid, self.language.tr("Mode", "模式"), |ui| {
            ui.label(self.language.tr("Bluetooth", "蓝牙"));
        });
        settings_form_row(ui, grid, self.language.tr("Status", "状态"), |ui| {
            ui.label(session_state_text(self.store.state.session, self.language));
        });
        if !self.device_controls_ready() {
            ui.label(
                egui::RichText::new(self.language.tr(
                    "Device settings are available when the connection is ready.",
                    "连接就绪后可编辑设备设置。",
                ))
                .size(self.metrics.small_font)
                .color(SECONDARY),
            );
        }
        if let Some(stage) = self.native_stage {
            settings_form_row(
                ui,
                grid,
                self.language.tr("Connection progress", "连接进度"),
                |ui| {
                    ui.label(native_stage_text(stage, self.language));
                },
            );
        }
        if let Some(error) = &self.last_ble_error {
            settings_form_row(
                ui,
                grid,
                self.language.tr("Last connection error", "最近连接错误"),
                |ui| {
                    ui.label(egui::RichText::new(error).color(RED));
                },
            );
        }
    }

    fn mixer_display_settings_card(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        settings_card(ui, grid, |ui| {
            section_heading(ui, self.language.tr("MIXER DISPLAY", "混音器显示"), YELLOW);
            settings_form_row(
                ui,
                grid,
                self.language.tr("Show Mute Buttons", "显示静音按钮"),
                |ui| {
                    ui.checkbox(&mut self.preferences.show_mute_buttons, "");
                },
            );
            settings_form_row(
                ui,
                grid,
                self.language.tr("Show Channel Icons", "显示通道图标"),
                |ui| {
                    ui.checkbox(&mut self.preferences.show_channel_icons, "");
                },
            );
            settings_form_row(
                ui,
                grid,
                self.language
                    .tr("Show Output Delay Indicator", "显示输出延迟指示"),
                |ui| {
                    ui.checkbox(&mut self.preferences.show_output_delay_indicator, "");
                },
            );
            ui.separator();
            ui.label(self.language.tr("Visible mixer inputs", "可见混音输入"));
            egui::Grid::new("settings-visible-inputs")
                .num_columns(2)
                .spacing(Vec2::new(grid.gap, 4.0 * grid.scale))
                .show(ui, |ui| {
                    for (index, input) in InputId::ALL.into_iter().enumerate() {
                        ui.checkbox(
                            &mut self.preferences.channel_visible[input.index()],
                            display_name(input, self.language),
                        );
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                    if InputId::ALL.len() % 2 != 0 {
                        ui.end_row();
                    }
                });
        })
    }

    fn device_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
        settings: &DeviceSettingsState,
    ) {
        settings_card(ui, grid, |ui| {
            section_heading(
                ui,
                self.language.tr("DEVICE / CONTROL", "设备 / 控制"),
                BLUE,
            );
            settings_form_row(
                ui,
                grid,
                self.language.tr("Device Name", "设备名称"),
                |ui| {
                    if let Some(name) = committed_text(
                        ui,
                        ("device-name", self.session_generation),
                        settings.device_name.effective().map(String::as_str),
                        DEVICE_NAME_MAX_BYTES,
                        self.language,
                    ) {
                        self.dispatch(SemanticCommand::SetSetting(KnownSetting::DeviceName(name)));
                    }
                },
            );
            let footswitch = settings
                .footswitch_fx_mode
                .effective()
                .copied()
                .unwrap_or(false);
            settings_form_row(
                ui,
                grid,
                self.language.tr("Footswitch Mode", "脚踏开关模式"),
                |ui| {
                    for (fx_mode, label) in
                        [(true, "FX"), (false, self.language.tr("Snapshot", "快照"))]
                    {
                        if state_button(ui, footswitch == fx_mode, label, BLUE).clicked() {
                            self.dispatch(SemanticCommand::SetSetting(
                                KnownSetting::FootswitchFxMode(fx_mode),
                            ));
                        }
                    }
                },
            );
            let linked = settings
                .device_linked_selection
                .effective()
                .copied()
                .unwrap_or(false);
            settings_form_row(
                ui,
                grid,
                self.language
                    .tr("Device-linked Output Selection", "设备联动输出选择"),
                |ui| {
                    if state_button(ui, linked, setting_switch_text(self.language, linked), BLUE)
                        .clicked()
                    {
                        self.dispatch(SemanticCommand::SetSetting(
                            KnownSetting::DeviceLinkedSelection(!linked),
                        ));
                    }
                },
            );
        })
    }

    fn monitor_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
        settings: &DeviceSettingsState,
    ) {
        settings_card(ui, grid, |ui| {
            section_heading(ui, self.language.tr("MONITOR ROUTING", "监听路由"), GREEN);
            let current = settings
                .monitor_routing
                .effective()
                .copied()
                .unwrap_or(MonitorRoutingSource::MonitorMix);
            settings_form_row(
                ui,
                grid,
                self.language.tr("Monitor Source", "监听来源"),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (candidate, label) in [
                            (MonitorRoutingSource::MonitorMix, "MON"),
                            (MonitorRoutingSource::Usb12, "USB 1/2"),
                            (MonitorRoutingSource::Usb34, "USB 3/4"),
                        ] {
                            if state_button(ui, current == candidate, label, GREEN).clicked() {
                                self.dispatch(SemanticCommand::SetSetting(
                                    KnownSetting::MonitorRouting(candidate),
                                ));
                            }
                        }
                    });
                },
            );
            let post = settings
                .monitor_post_fader
                .effective()
                .copied()
                .unwrap_or(false);
            settings_form_row(
                ui,
                grid,
                self.language.tr("Monitor Tap", "监听取样点"),
                |ui| {
                    if state_button(ui, post, if post { "POST" } else { "PRE" }, GREEN).clicked() {
                        self.dispatch(SemanticCommand::SetSetting(KnownSetting::MonitorPostFader(
                            !post,
                        )));
                    }
                },
            );
            let linked = self
                .store
                .state
                .routing
                .monitor_link
                .stereo_linked
                .effective()
                .copied()
                .unwrap_or(false);
            settings_form_row(
                ui,
                grid,
                self.language.tr("MON1/2 Stereo Link", "MON1/2 立体声链接"),
                |ui| {
                    if state_button(
                        ui,
                        linked,
                        setting_switch_text(self.language, linked),
                        GREEN,
                    )
                    .clicked()
                    {
                        self.dispatch(SemanticCommand::SetSetting(
                            KnownSetting::MonitorStereoLink(!linked),
                        ));
                    }
                },
            );
        })
    }

    fn usb_headphones_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
        settings: &DeviceSettingsState,
    ) {
        settings_card(ui, grid, |ui| {
            self.usb_source_settings_section(ui, grid, settings);
            self.headphone_settings_section(ui, grid);
        })
    }

    fn usb_source_settings_section(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
        settings: &DeviceSettingsState,
    ) {
        section_heading(
            ui,
            self.language.tr("USB / HEADPHONES", "USB / 耳机"),
            GREEN,
        );
        for (active, label, command) in [
            (
                settings.usb_streaming.effective().copied().unwrap_or(false),
                self.language.tr("USB Streaming", "USB 推流"),
                0_u8,
            ),
            (
                settings
                    .input56_from_usb12
                    .effective()
                    .copied()
                    .unwrap_or(false),
                self.language
                    .tr("USB 1/2 → Input 5/6", "USB 1/2 → 输入 5/6"),
                1_u8,
            ),
            (
                settings
                    .input78_from_usb34
                    .effective()
                    .copied()
                    .unwrap_or(false),
                self.language
                    .tr("USB 3/4 → Input 7/8", "USB 3/4 → 输入 7/8"),
                2_u8,
            ),
            (
                settings.phones_only.effective().copied().unwrap_or(false),
                self.language
                    .tr("Bluetooth / USB to Headphones Only", "蓝牙 / USB 仅送耳机"),
                3_u8,
            ),
        ] {
            settings_form_row(ui, grid, label, |ui| {
                if state_button(
                    ui,
                    active,
                    setting_switch_text(self.language, active),
                    GREEN,
                )
                .clicked()
                {
                    let setting = match command {
                        0 => KnownSetting::UsbStreaming(!active),
                        1 => KnownSetting::Input56FromUsb12(!active),
                        2 => KnownSetting::Input78FromUsb34(!active),
                        _ => KnownSetting::PhonesOnly(!active),
                    };
                    self.dispatch(SemanticCommand::SetSetting(setting));
                }
            });
        }
        let bt_usb_switch = settings.bt_usb_switch.effective().copied().unwrap_or(false);
        settings_form_row(
            ui,
            grid,
            self.language
                .tr("Bluetooth / USB source switch", "蓝牙 / USB 信号源切换"),
            |ui| {
                if state_button(
                    ui,
                    bt_usb_switch,
                    setting_switch_text(self.language, bt_usb_switch),
                    GREEN,
                )
                .clicked()
                {
                    self.dispatch(SemanticCommand::SetSetting(KnownSetting::BtUsbSwitch(
                        !bt_usb_switch,
                    )));
                }
            },
        );
    }

    fn headphone_settings_section(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        let headphone_source = self
            .store
            .state
            .routing
            .headphones
            .source
            .effective()
            .copied()
            .unwrap_or(HeadphoneSource::Main);
        settings_form_row(
            ui,
            grid,
            self.language.tr("Headphone Source", "耳机信号源"),
            |ui| {
                for (source, label) in [
                    (HeadphoneSource::Main, "MAIN"),
                    (HeadphoneSource::Monitor, "MON"),
                ] {
                    if state_button(ui, headphone_source == source, label, GREEN).clicked() {
                        self.dispatch(SemanticCommand::SetSetting(
                            KnownSetting::HeadphonesUseMonitor(source == HeadphoneSource::Monitor),
                        ));
                    }
                }
            },
        );
        let headphone_tap = self
            .store
            .state
            .routing
            .headphones
            .tap_point
            .effective()
            .copied()
            .unwrap_or(TapPoint::PostFader);
        settings_form_row(
            ui,
            grid,
            self.language.tr("Headphone Tap", "耳机取样点"),
            |ui| {
                for (tap, label) in [(TapPoint::PreFader, "PRE"), (TapPoint::PostFader, "POST")] {
                    if state_button(ui, headphone_tap == tap, label, GREEN).clicked() {
                        self.dispatch(SemanticCommand::SetSetting(
                            KnownSetting::HeadphonesPostFader(tap == TapPoint::PostFader),
                        ));
                    }
                }
            },
        );
    }

    fn output_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
        settings: &DeviceSettingsState,
    ) {
        settings_card(ui, grid, |ui| {
            section_heading(ui, self.language.tr("OUTPUT", "输出"), TEXT);
            ui.label(
                egui::RichText::new(self.language.tr("OUTPUT LEVEL", "输出电平"))
                    .size(self.metrics.small_font)
                    .color(SECONDARY),
            );
            let main_pad = settings
                .main_minus_10_dbv
                .effective()
                .copied()
                .unwrap_or(false);
            settings_form_row(ui, grid, "MAIN -10 dBV", |ui| {
                if state_button(
                    ui,
                    main_pad,
                    setting_switch_text(self.language, main_pad),
                    TEXT,
                )
                .clicked()
                {
                    self.dispatch(SemanticCommand::SetSetting(KnownSetting::MainMinus10Dbv(
                        !main_pad,
                    )));
                }
            });
            let monitor_pad = settings
                .monitor_minus_10_dbv
                .effective()
                .copied()
                .unwrap_or(false);
            settings_form_row(ui, grid, "MON -10 dBV", |ui| {
                if state_button(
                    ui,
                    monitor_pad,
                    setting_switch_text(self.language, monitor_pad),
                    TEXT,
                )
                .clicked()
                {
                    self.dispatch(SemanticCommand::SetSetting(
                        KnownSetting::MonitorMinus10Dbv(!monitor_pad),
                    ));
                }
            });
            ui.separator();
            self.output_delay_settings(ui, grid);
        })
    }

    fn output_delay_settings(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
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
            settings_form_row(ui, grid, destination_name(destination), |ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.delay_ms_draft[index])
                            .id_salt(("output-delay-ms", index))
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

    fn last_scan_settings_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            section_heading(
                ui,
                self.language.tr("Last BLE Scan", "最近一次 BLE 扫描"),
                BLUE,
            );
            for device in &self.discovered_devices {
                ui.label(format!(
                    "{} · {} · RSSI {:?}",
                    device
                        .name
                        .as_deref()
                        .unwrap_or_else(|| { self.language.tr("Unknown", "未知") }),
                    device.address,
                    device.rssi
                ));
            }
        })
    }
}

fn format_delay_ms(ticks: u32) -> String {
    format!("{:.3}", ticks as f64 / DELAY_TICKS_PER_MILLISECOND)
}

fn parse_delay_ms(value: &str) -> Option<u32> {
    let milliseconds = value.trim().parse::<f64>().ok()?;
    let ticks = (milliseconds * DELAY_TICKS_PER_MILLISECOND).round();
    (milliseconds.is_finite() && milliseconds >= 0.0 && ticks <= u32::MAX as f64)
        .then_some(ticks as u32)
}

fn settings_grid_row(
    app: &mut Flow8App,
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    row_id: u8,
    left: impl FnOnce(&mut Flow8App, &mut egui::Ui),
    right: impl FnOnce(&mut Flow8App, &mut egui::Ui),
) {
    if layout.two_columns {
        // Both cards start at the same top edge. Advance the parent only once,
        // by the taller card's actual height; Grid centers zero-height child
        // Uis within its previous row height and leaves a large empty band.
        let available = ui.available_rect_before_wrap();
        let top = available.top();
        let left_x = available.left();
        let right_x = left_x + layout.column_width + layout.gap;
        let height = available.height().max(0.0);
        let mut left_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("settings-left", row_id))
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(left_x, top),
                    Vec2::new(layout.column_width, height),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        left(app, &mut left_ui);
        let left_bottom = left_ui.min_rect().bottom();

        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("settings-right", row_id))
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(right_x, top),
                    Vec2::new(layout.column_width, height),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        right(app, &mut right_ui);
        let right_bottom = right_ui.min_rect().bottom();

        ui.advance_cursor_after_rect(egui::Rect::from_min_max(
            egui::pos2(left_x, top),
            egui::pos2(right_x + layout.column_width, left_bottom.max(right_bottom)),
        ));
    } else {
        left(app, ui);
        ui.add_space(layout.gap);
        right(app, ui);
    }
}

fn settings_card<R>(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card(ui, |ui| {
        ui.set_width((layout.column_width - 26.0).max(0.0));
        add(ui)
    })
}

fn settings_form_row(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    label: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    let min_value_width = 220.0 * layout.scale;
    if ui.available_width() < layout.label_width + min_value_width + layout.gap {
        ui.vertical(|ui| {
            ui.label(label);
            add(ui);
        });
    } else {
        ui.horizontal(|ui| {
            ui.add_sized(
                [layout.label_width, ui.spacing().interact_size.y],
                egui::Label::new(label),
            );
            add(ui);
        });
    }
}
