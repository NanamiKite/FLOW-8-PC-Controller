use crate::app::Flow8App;
use crate::app::confirmation::ConfirmationAction;
use crate::i18n::{Language, waiting_for_device_text};
use crate::metrics::SnapshotGridLayout;
use crate::theme::{GREEN, RED, SECONDARY, SURFACE_ALT, TEXT, YELLOW, card_frame};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Align, FontId, Layout, Response, Sense, Vec2};
use flow8_core::SemanticCommand;
use flow8_model::SnapshotSlotState;

impl Flow8App {
    pub(in crate::app) fn snapshots_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        let snapshots = self.store.state.snapshots.device_slots.clone();
        let viewport_width = ui.available_width().min(ui.clip_rect().width()).max(0.0);
        let layout = SnapshotGridLayout::new(
            (viewport_width - 36.0).max(0.0),
            self.metrics.ui_scale,
            self.metrics.spacing,
            snapshots.len(),
        );
        let _ = centered_snapshot_content(ui, layout.group_width(), |ui| {
            ui.heading(
                egui::RichText::new(self.language.tr("Device Snapshots", "设备快照")).color(TEXT),
            );
            ui.label(
                egui::RichText::new(self.language.tr(
                    "These slots are stored on FLOW 8.",
                    "这些槽位保存在 FLOW 8 上。",
                ))
                .color(SECONDARY),
            );
            if ui
                .button(self.language.tr("Refresh slots", "刷新槽位"))
                .clicked()
            {
                self.dispatch(SemanticCommand::RequestSnapshotNames);
            }
            ui.add_space(10.0);
            for row in snapshots.chunks(layout.columns) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = layout.gap;
                    for snapshot in row {
                        let _ = snapshot_card(ui, layout.card_inner_width, |ui| {
                            let status = device_slot_status(snapshot);
                            let busy = snapshot.name.pending.is_some();
                            let occupied = matches!(status, DeviceSlotStatus::Occupied(_)) && !busy;
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{:02}", snapshot.slot + 1))
                                        .strong()
                                        .color(YELLOW),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    snapshot_status_badge(ui, &status, self.language);
                                });
                            });
                            let name = match &status {
                                DeviceSlotStatus::Occupied(name) => Some(name.as_str()),
                                _ => None,
                            };
                            snapshot_name_row(
                                ui,
                                name,
                                busy,
                                layout.card_inner_width,
                                self.metrics.body_font,
                                self.language,
                            );
                            let button_size =
                                Vec2::new(layout.button_width, self.metrics.control_height);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = layout.gap;
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Load", "载入"),
                                    occupied,
                                    button_size,
                                )
                                .clicked()
                                {
                                    self.request_confirmation(
                                        ConfirmationAction::LoadDeviceSnapshot {
                                            slot: snapshot.slot,
                                            observed_name: snapshot.name.confirmed.clone(),
                                        },
                                    );
                                }
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Save", "保存"),
                                    !busy,
                                    button_size,
                                )
                                .clicked()
                                {
                                    let name = match &status {
                                        DeviceSlotStatus::Occupied(name) => name.clone(),
                                        _ => format!("Snapshot {:02}", snapshot.slot + 1),
                                    };
                                    self.request_confirmation(
                                        ConfirmationAction::SaveDeviceSnapshot {
                                            slot: snapshot.slot,
                                            name,
                                            observed_name: snapshot.name.confirmed.clone(),
                                        },
                                    );
                                }
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Delete", "删除"),
                                    occupied,
                                    button_size,
                                )
                                .clicked()
                                {
                                    self.request_confirmation(
                                        ConfirmationAction::DeleteDeviceSnapshot {
                                            slot: snapshot.slot,
                                            observed_name: snapshot.name.confirmed.clone(),
                                        },
                                    );
                                }
                                if snapshot_action_button(
                                    ui,
                                    self.language.tr("Rename", "重命名"),
                                    occupied,
                                    button_size,
                                )
                                .clicked()
                                {
                                    if let DeviceSlotStatus::Occupied(name) = &status {
                                        self.request_confirmation(
                                            ConfirmationAction::RenameDeviceSnapshot {
                                                slot: snapshot.slot,
                                                name: name.clone(),
                                                observed_name: snapshot.name.confirmed.clone(),
                                            },
                                        );
                                    }
                                }
                            });
                        });
                    }
                });
                ui.add_space(layout.gap);
            }
            ui.add_space(10.0);
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum DeviceSlotStatus {
    Unknown,
    Free,
    Occupied(String),
}

pub(in crate::app) fn device_slot_status(slot: &SnapshotSlotState) -> DeviceSlotStatus {
    match slot.name.confirmed.as_deref() {
        None => DeviceSlotStatus::Unknown,
        Some("") => DeviceSlotStatus::Free,
        Some(name) => DeviceSlotStatus::Occupied(name.to_owned()),
    }
}

fn ellipsize_snapshot_name(
    painter: &egui::Painter,
    name: &str,
    font: &FontId,
    max_width: f32,
) -> String {
    let width = |text: String| painter.layout_no_wrap(text, font.clone(), TEXT).size().x;
    if width(name.to_owned()) <= max_width {
        return name.to_owned();
    }
    let characters: Vec<char> = name.chars().collect();
    let mut low = 0;
    let mut high = characters.len();
    while low < high {
        let middle = (low + high + 1) / 2;
        let candidate = format!("{}…", characters[..middle].iter().collect::<String>());
        if width(candidate) <= max_width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    format!("{}…", characters[..low].iter().collect::<String>())
}

pub(in crate::app) fn snapshot_status_badge(
    ui: &mut egui::Ui,
    status: &DeviceSlotStatus,
    language: Language,
) -> Response {
    let (label, fill, foreground) = match status {
        DeviceSlotStatus::Free => (
            language.tr("Free", "空闲"),
            GREEN.gamma_multiply(0.35),
            GREEN,
        ),
        DeviceSlotStatus::Occupied(_) => (
            language.tr("Occupied", "已占用"),
            RED.gamma_multiply(0.35),
            RED,
        ),
        DeviceSlotStatus::Unknown => (language.tr("Unknown", "未知"), SURFACE_ALT, SECONDARY),
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).color(foreground));
        })
        .response
}

pub(in crate::app) fn snapshot_name_row(
    ui: &mut egui::Ui,
    name: Option<&str>,
    busy: bool,
    width: f32,
    font_size: f32,
    language: Language,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, (font_size * 1.4).round_ui()),
        Sense::hover(),
    );
    let text = if busy {
        waiting_for_device_text(language)
    } else {
        name.unwrap_or("")
    };
    if !text.is_empty() {
        let painter = ui.painter().with_clip_rect(rect);
        let font = FontId::proportional(font_size);
        let display = ellipsize_snapshot_name(&painter, text, &font, rect.width() - 4.0);
        painter.text(
            rect.left_center() + Vec2::new(2.0, 0.0),
            egui::Align2::LEFT_CENTER,
            display,
            font,
            if busy { SECONDARY } else { TEXT },
        );
    }
    if let Some(name) = name {
        response.on_hover_text(name)
    } else {
        response
    }
}

pub(in crate::app) fn centered_snapshot_content<R>(
    ui: &mut egui::Ui,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.allocate_ui_with_layout(Vec2::new(width, 0.0), Layout::top_down(Align::Min), add)
    })
    .inner
}

pub(in crate::app) fn snapshot_card<R>(
    ui: &mut egui::Ui,
    inner_width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_frame().show(ui, |ui| {
        ui.set_width(inner_width);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.set_width(inner_width);
            add(ui)
        })
        .inner
    })
}

pub(in crate::app) fn snapshot_action_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    size: Vec2,
) -> Response {
    ui.add_enabled(enabled, egui::Button::new(label).small().min_size(size))
}
