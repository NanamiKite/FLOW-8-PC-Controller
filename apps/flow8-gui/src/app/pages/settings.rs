use crate::app::{ControlGesture, EqEditingMode, Flow8App};
use crate::i18n::{
    Language, display_name, localized_status_message, native_stage_text, session_state_text,
    setting_switch_text,
};
use crate::metrics::{SETTINGS_CONTENT_MAX_WIDTH, SettingsGridLayout, UI_SCALE_MAX, UI_SCALE_MIN};
use crate::theme::{BLUE, RED, SECONDARY, TEXT, YELLOW, card, card_frame, section_heading};
use crate::widgets::{committed_text, state_button_sized};
use eframe::egui::containers::frame::Prepared;
use eframe::egui::{self, Align, Layout, Vec2};
use flow8_core::{KnownSetting, SemanticCommand};
use flow8_model::{DEVICE_NAME_MAX_BYTES, DeviceSettingsState, InputId};

impl Flow8App {
    pub(in crate::app) fn settings_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(20.0);
                let content_width =
                    (ui.available_width() - 64.0).clamp(0.0, SETTINGS_CONTENT_MAX_WIDTH);
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
                            ui.add_space(12.0);
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
                                true,
                                |this, ui| this.application_settings_card(ui, grid),
                                |this, ui| this.mixer_display_settings_card(ui, grid),
                            );

                            settings_row_gap(ui, grid);
                            settings_grid_row(
                                self,
                                ui,
                                grid,
                                1,
                                true,
                                |this, ui| this.device_settings_card(ui, grid, &settings),
                                |this, ui| this.connection_settings_card(ui, grid),
                            );

                            if !self.discovered_devices.is_empty() {
                                settings_row_gap(ui, grid);
                                self.last_scan_settings_card(ui);
                            }
                        },
                    );
                });
            });
    }

    fn application_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
    ) -> Prepared {
        let labels = [
            self.language.tr("Language", "语言"),
            self.language.tr("UI Scale", "界面缩放"),
            self.language.tr("Control Gesture", "控制手势"),
            self.language.tr("EQ Editing Mode", "EQ 编辑模式"),
        ];
        let grid = settings_labels(ui, grid, &labels);
        settings_card(ui, grid, |ui| {
            self.application_preferences_section(ui, grid);
        })
    }

    fn connection_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
    ) -> Prepared {
        let mut labels = vec![
            self.language.tr("Mode", "模式"),
            self.language.tr("Status", "状态"),
        ];
        if self.native_stage.is_some() {
            labels.push(self.language.tr("Connection progress", "连接进度"));
        }
        if self.last_ble_error.is_some() {
            labels.push(self.language.tr("Last connection error", "最近连接错误"));
        }
        let grid = settings_labels(ui, grid, &labels);
        settings_card(ui, grid, |ui| {
            self.connection_settings_section(ui, grid);
        })
    }

    fn application_preferences_section(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
        section_heading(ui, self.language.tr("APPLICATION", "应用"), BLUE);
        settings_form_row(ui, grid, self.language.tr("Language", "语言"), |ui| {
            if settings_button(
                ui,
                grid,
                self.language == Language::English,
                "English",
                BLUE,
            )
            .clicked()
            {
                self.language = Language::English;
                self.message = localized_status_message(self.store.state.session, self.language);
                ui.ctx().request_repaint();
            }
            if settings_button(
                ui,
                grid,
                self.language == Language::Chinese,
                "简体中文",
                BLUE,
            )
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
        ui.scope(|ui| {
            settings_device_row(
                ui,
                grid,
                self.language.tr("Control Gesture", "控制手势"),
                false,
                |ui| {
                    for (mode, label) in [
                        (ControlGesture::Linear, self.language.tr("Linear", "线性")),
                        (ControlGesture::Rotary, self.language.tr("Rotary", "旋转")),
                    ] {
                        let _ = settings_button(
                            ui,
                            grid,
                            self.preferences.control_gesture == mode,
                            label,
                            BLUE,
                        );
                    }
                },
            );
            settings_device_row(
                ui,
                grid,
                self.language.tr("EQ Editing Mode", "EQ 编辑模式"),
                false,
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
                        let _ = settings_button(
                            ui,
                            grid,
                            self.preferences.eq_editing_mode == mode,
                            label,
                            BLUE,
                        );
                    }
                },
            );
        });
    }

    fn connection_settings_section(&mut self, ui: &mut egui::Ui, grid: SettingsGridLayout) {
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

    fn mixer_display_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
    ) -> Prepared {
        let grid = settings_labels(
            ui,
            grid,
            &[
                self.language.tr("Show Mute Buttons", "显示静音按钮"),
                self.language.tr("Show Channel Icons", "显示通道图标"),
                self.language
                    .tr("Show Output Delay Indicator", "显示输出延迟指示"),
            ],
        );
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
            ui.horizontal_wrapped(|ui| {
                for input in InputId::ALL {
                    ui.checkbox(
                        &mut self.preferences.channel_visible[input.index()],
                        display_name(input, self.language),
                    );
                }
            });
        })
    }

    fn device_settings_card(
        &mut self,
        ui: &mut egui::Ui,
        grid: SettingsGridLayout,
        settings: &DeviceSettingsState,
    ) -> Prepared {
        let ready = self.device_controls_ready();
        let grid = settings_labels(
            ui,
            grid,
            &[
                self.language.tr("Device Name", "设备名称"),
                self.language.tr("Footswitch Mode", "脚踏开关模式"),
                self.language
                    .tr("Device-linked Output Selection", "设备联动输出选择"),
            ],
        );
        settings_card(ui, grid, |ui| {
            section_heading(
                ui,
                self.language.tr("DEVICE / CONTROL", "设备 / 控制"),
                BLUE,
            );
            settings_device_row(
                ui,
                grid,
                self.language.tr("Device Name", "设备名称"),
                ready,
                |ui| {
                    if let Some(name) = committed_text(
                        ui,
                        ("device-name", self.session_generation),
                        settings.device_name.effective().map(String::as_str),
                        DEVICE_NAME_MAX_BYTES,
                        self.language,
                    ) && ready
                    {
                        self.dispatch(SemanticCommand::SetSetting(KnownSetting::DeviceName(name)));
                    }
                },
            );
            let footswitch = settings.footswitch_fx_mode.effective().copied();
            settings_device_row(
                ui,
                grid,
                self.language.tr("Footswitch Mode", "脚踏开关模式"),
                ready,
                |ui| {
                    if let Some(fx_mode) = settings_choice(
                        ui,
                        grid,
                        footswitch,
                        [(true, "FX"), (false, self.language.tr("Snapshot", "快照"))],
                        BLUE,
                    ) {
                        self.dispatch(SemanticCommand::SetSetting(KnownSetting::FootswitchFxMode(
                            fx_mode,
                        )));
                    }
                },
            );
            let linked = settings.device_linked_selection.effective().copied();
            settings_device_row(
                ui,
                grid,
                self.language
                    .tr("Device-linked Output Selection", "设备联动输出选择"),
                ready,
                |ui| {
                    if let Some(value) = settings_switch(ui, grid, linked, self.language, BLUE) {
                        self.dispatch(SemanticCommand::SetSetting(
                            KnownSetting::DeviceLinkedSelection(value),
                        ));
                    }
                },
            );
        })
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

fn settings_grid_row(
    app: &mut Flow8App,
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    row_id: u8,
    align_bottoms: bool,
    left: impl FnOnce(&mut Flow8App, &mut egui::Ui) -> Prepared,
    right: impl FnOnce(&mut Flow8App, &mut egui::Ui) -> Prepared,
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
        let mut left_card = left(app, &mut left_ui);
        left_card.allocate_space(&mut left_ui);
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
        let mut right_card = right(app, &mut right_ui);
        right_card.allocate_space(&mut right_ui);
        let right_bottom = right_ui.min_rect().bottom();

        if align_bottoms {
            // Extend only the frames to this pair's natural content height.
            // Leave controls in place and keep single-column cards natural.
            let height = left_card
                .content_ui
                .min_rect()
                .height()
                .max(right_card.content_ui.min_rect().height());
            for card in [&mut left_card, &mut right_card] {
                let mut rect = card.content_ui.min_rect();
                rect.max.y = rect.min.y + height;
                card.content_ui.expand_to_include_rect(rect);
            }
        }
        left_card.paint(&left_ui);
        right_card.paint(&right_ui);

        ui.advance_cursor_after_rect(egui::Rect::from_min_max(
            egui::pos2(left_x, top),
            egui::pos2(right_x + layout.column_width, left_bottom.max(right_bottom)),
        ));
    } else {
        left(app, ui).end(ui);
        settings_row_gap(ui, layout);
        right(app, ui).end(ui);
    }
}

fn settings_card(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    add: impl FnOnce(&mut egui::Ui),
) -> Prepared {
    let mut card = card_frame().corner_radius(6).inner_margin(16).begin(ui);
    let ui = &mut card.content_ui;
    ui.set_width((layout.column_width - 34.0).max(0.0));
    ui.spacing_mut().item_spacing = Vec2::new(layout.control_gap, layout.row_gap);
    ui.spacing_mut().icon_width = (18.0 * layout.scale).round();
    ui.spacing_mut().icon_width_inner = (12.0 * layout.scale).round();
    add(ui);
    card
}

fn settings_form_row(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    label: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    let row_width = layout.label_width + layout.control_width + layout.control_gap;
    if ui.available_width() < row_width {
        ui.vertical(|ui| {
            ui.label(label);
            ui.horizontal_wrapped(add);
        });
    } else {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(layout.label_width, layout.row_height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_max_width(layout.label_width);
                    ui.set_min_size(Vec2::new(layout.label_width, layout.row_height));
                    ui.add(egui::Label::new(label).wrap());
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(layout.control_width, layout.row_height),
                Layout::left_to_right(Align::Center),
                add,
            );
        });
    }
}

fn settings_row_gap(ui: &mut egui::Ui, layout: SettingsGridLayout) {
    // The preceding row already advanced the cursor by item_spacing.y.
    ui.add_space((layout.gap - ui.spacing().item_spacing.y).max(0.0));
}

fn settings_device_row(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    label: &str,
    enabled: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    settings_form_row(ui, layout, label, |ui| {
        // Keep labels readable; only the actual controls are unavailable.
        ui.add_enabled_ui(enabled, add);
    });
}

fn settings_switch(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    value: Option<bool>,
    language: Language,
    accent: egui::Color32,
) -> Option<bool> {
    settings_toggle(
        ui,
        layout,
        value,
        [
            setting_switch_text(language, false),
            setting_switch_text(language, true),
        ],
        accent,
    )
}

fn settings_toggle(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    value: Option<bool>,
    labels: [&str; 2],
    accent: egui::Color32,
) -> Option<bool> {
    let label = value.map_or("—", |value| labels[usize::from(value)]);
    let clicked = ui
        .scope(|ui| {
            // A disconnected row is already disabled; avoid dimming it twice.
            if value.is_none() && ui.is_enabled() {
                ui.disable();
            }
            settings_button(ui, layout, value == Some(true), label, accent).clicked()
        })
        .inner;
    clicked.then(|| value.map(|value| !value)).flatten()
}

fn settings_choice<T: Copy + PartialEq, const N: usize>(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    value: Option<T>,
    choices: [(T, &str); N],
    accent: egui::Color32,
) -> Option<T> {
    let mut selected = None;
    ui.scope(|ui| {
        if value.is_none() && ui.is_enabled() {
            ui.disable();
        }
        ui.horizontal_wrapped(|ui| {
            for (candidate, label) in choices {
                if settings_button(ui, layout, value == Some(candidate), label, accent).clicked() {
                    selected = Some(candidate);
                }
            }
        });
    });
    selected
}

fn settings_labels(
    ui: &egui::Ui,
    mut layout: SettingsGridLayout,
    labels: &[&str],
) -> SettingsGridLayout {
    let font = egui::TextStyle::Body.resolve(ui.style());
    layout.label_width = labels
        .iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap((*label).to_owned(), font.clone(), TEXT)
                .size()
                .x
        })
        .fold(0.0, f32::max)
        .ceil()
        .min(layout.label_width);
    layout
}

fn settings_button(
    ui: &mut egui::Ui,
    layout: SettingsGridLayout,
    selected: bool,
    label: &str,
    accent: egui::Color32,
) -> egui::Response {
    state_button_sized(
        ui,
        selected,
        label,
        accent,
        Vec2::new(layout.choice_width, layout.row_height),
    )
}
