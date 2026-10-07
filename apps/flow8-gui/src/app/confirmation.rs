use crate::app::{Flow8App, Page};
use crate::theme::{RED, SECONDARY};
use eframe::egui::{self, Vec2};
use flow8_core::{SemanticCommand, SessionState};
use flow8_model::InputId;
use flow8_model::SNAPSHOT_NAME_MAX_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ConfirmationAction {
    EnablePhantom(InputId),
    SaveDeviceSnapshot {
        slot: u8,
        name: String,
        observed_name: Option<String>,
    },
    LoadDeviceSnapshot {
        slot: u8,
        observed_name: Option<String>,
    },
    DeleteDeviceSnapshot {
        slot: u8,
        observed_name: Option<String>,
    },
    RenameDeviceSnapshot {
        slot: u8,
        name: String,
        observed_name: Option<String>,
    },
}

pub(super) fn valid_snapshot_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed.len() <= SNAPSHOT_NAME_MAX_BYTES
        && !trimmed.chars().any(char::is_control)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingConfirmation {
    pub(super) action: ConfirmationAction,
    pub(super) page: Page,
    pub(super) selected_input: Option<InputId>,
    pub(super) session: SessionState,
}

impl Flow8App {
    pub(super) fn request_confirmation(&mut self, action: ConfirmationAction) {
        if !self.device_controls_ready() {
            return;
        }
        self.pending_confirmation = Some(PendingConfirmation {
            action,
            page: self.page,
            selected_input: self.store.state.selected_input,
            session: self.store.state.session,
        });
    }

    fn confirmation_is_valid(&self, pending: &PendingConfirmation) -> bool {
        if self.page != pending.page
            || self.store.state.selected_input != pending.selected_input
            || self.store.state.session != pending.session
            || self.disconnect_request_pending
            || pending.session != SessionState::Ready
        {
            return false;
        }
        match &pending.action {
            ConfirmationAction::EnablePhantom(input) => {
                self.store.state.selected_input == Some(*input)
                    && self.store.state.channels[input.index()]
                        .capabilities
                        .phantom
                    && self.store.state.channels[input.index()]
                        .phantom_48v
                        .effective()
                        == Some(&false)
            }
            ConfirmationAction::SaveDeviceSnapshot {
                slot,
                observed_name,
                ..
            }
            | ConfirmationAction::LoadDeviceSnapshot {
                slot,
                observed_name,
            }
            | ConfirmationAction::DeleteDeviceSnapshot {
                slot,
                observed_name,
            }
            | ConfirmationAction::RenameDeviceSnapshot {
                slot,
                observed_name,
                ..
            } => self
                .store
                .state
                .snapshots
                .device_slots
                .get(*slot as usize)
                .is_some_and(|current| current.name.confirmed == *observed_name),
        }
    }

    pub(super) fn clear_stale_confirmation(&mut self) {
        if self
            .pending_confirmation
            .as_ref()
            .is_some_and(|pending| !self.confirmation_is_valid(pending))
        {
            self.pending_confirmation = None;
        }
    }

    pub(super) fn confirm_pending_action(&mut self) {
        let Some(pending) = self.pending_confirmation.take() else {
            return;
        };
        if !self.confirmation_is_valid(&pending) {
            self.message = self
                .language
                .tr(
                    "The device or selected context changed. Choose the action again.",
                    "设备状态或选择已变化，请重新选择操作。",
                )
                .into();
            return;
        }
        let command = match pending.action {
            ConfirmationAction::EnablePhantom(input) => SemanticCommand::SetPhantom {
                input,
                enabled: true,
            },
            ConfirmationAction::SaveDeviceSnapshot { slot, name, .. } => {
                if !valid_snapshot_name(&name) {
                    return;
                }
                SemanticCommand::SaveSnapshot {
                    slot,
                    name: name.trim().to_owned(),
                }
            }
            ConfirmationAction::RenameDeviceSnapshot { slot, name, .. } => {
                if !valid_snapshot_name(&name) {
                    return;
                }
                SemanticCommand::RenameSnapshot {
                    slot,
                    name: name.trim().to_owned(),
                }
            }
            ConfirmationAction::LoadDeviceSnapshot { slot, .. } => {
                SemanticCommand::LoadSnapshot { slot }
            }
            ConfirmationAction::DeleteDeviceSnapshot { slot, .. } => {
                SemanticCommand::DeleteSnapshot { slot }
            }
        };
        self.dispatch(command);
    }

    pub(super) fn confirmation_dialog(&mut self, context: &egui::Context) {
        self.clear_stale_confirmation();
        let Some(mut pending) = self.pending_confirmation.clone() else {
            return;
        };
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.pending_confirmation = None;
            return;
        }
        let (title, description) = match pending.action.clone() {
            ConfirmationAction::EnablePhantom(input) => (
                self.language.tr("Enable 48 V?", "开启 48 V？").to_owned(),
                format!(
                    "{} {}. {}",
                    self.language.tr("Input", "输入"),
                    input.index() + 1,
                    self.language.tr(
                        "48 V may damage incompatible microphones or connected equipment. Only this input will change.",
                        "48 V 可能损坏不兼容的麦克风或连接设备；仅此输入通道会改变。",
                    )
                ),
            ),
            ConfirmationAction::SaveDeviceSnapshot { slot, observed_name, .. } => (
                self.language.tr("Save device snapshot?", "保存设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    match observed_name.as_deref() {
                        Some("") => self.language.tr(
                            "Current mixer settings will be saved to this free device slot.",
                            "当前混音设置将保存到此空闲设备槽位。",
                        ),
                        Some(_) => self.language.tr(
                            "Current mixer settings will replace the snapshot stored in this slot.",
                            "当前混音设置会覆盖此槽位中的快照。",
                        ),
                        None => self.language.tr(
                            "Slot occupancy is unknown. Saving may overwrite a device snapshot.",
                            "槽位占用状态未知；保存可能覆盖设备快照。",
                        ),
                    }
                ),
            ),
            ConfirmationAction::LoadDeviceSnapshot { slot, .. } => (
                self.language.tr("Load device snapshot?", "载入设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    self.language.tr(
                        "This applies the saved snapshot to the live FLOW 8 mixer. Current settings may change according to the snapshot scope.",
                        "这会将快照应用到 FLOW 8 当前混音；具体设置可能按快照范围改变。",
                    )
                ),
            ),
            ConfirmationAction::DeleteDeviceSnapshot { slot, .. } => (
                self.language.tr("Delete device snapshot?", "删除设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    self.language.tr(
                        "The snapshot stored on FLOW 8 will be deleted. This cannot be undone here.",
                        "FLOW 8 上保存的快照将被删除，此处无法撤销。",
                    )
                ),
            ),
            ConfirmationAction::RenameDeviceSnapshot { slot, .. } => (
                self.language.tr("Rename device snapshot?", "重命名设备快照？").to_owned(),
                format!(
                    "{} {:02}. {}",
                    self.language.tr("FLOW 8 device slot", "FLOW 8 设备槽位"),
                    slot + 1,
                    self.language.tr(
                        "Only the name stored in this FLOW 8 slot will change.",
                        "只修改 FLOW 8 此槽位保存的名称。",
                    )
                ),
            ),
        };
        let mut open = true;
        let mut decision = None;
        egui::Window::new(title)
            .id(egui::Id::new("device-action-confirmation"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(context, |ui| {
                ui.set_max_width(420.0);
                ui.label(description);
                let mut can_confirm = true;
                if let ConfirmationAction::SaveDeviceSnapshot { name, .. }
                | ConfirmationAction::RenameDeviceSnapshot { name, .. } = &mut pending.action
                {
                    ui.add_space(8.0);
                    ui.label(self.language.tr("Snapshot name", "快照名称"));
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .desired_width(300.0)
                            .hint_text(self.language.tr("Name", "名称")),
                    );
                    can_confirm = valid_snapshot_name(name);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} / {} {}",
                            name.trim().len(),
                            SNAPSHOT_NAME_MAX_BYTES,
                            self.language.tr("UTF-8 bytes", "UTF-8 字节")
                        ))
                        .color(if can_confirm { SECONDARY } else { RED }),
                    );
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(self.language.tr("Cancel", "取消")).clicked() {
                        decision = Some(false);
                    }
                    if ui
                        .add_enabled(
                            can_confirm,
                            egui::Button::new(self.language.tr("Confirm", "确认")),
                        )
                        .clicked()
                    {
                        decision = Some(true);
                    }
                });
            });
        self.pending_confirmation = Some(pending);
        match decision {
            Some(true) => self.confirm_pending_action(),
            Some(false) => self.pending_confirmation = None,
            None if !open => self.pending_confirmation = None,
            None => {}
        }
    }
}
