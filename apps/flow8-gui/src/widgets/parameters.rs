use crate::i18n::Language;
use crate::theme::{RED, TEXT};
use eframe::egui::{self, Align, Layout};
use flow8_model::{ParameterSpec, specs};

pub(crate) struct ParameterRowResponse {
    user_changed: bool,
}

impl ParameterRowResponse {
    pub(crate) fn changed(&self) -> bool {
        self.user_changed
    }
}

pub(crate) fn parameter_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    spec: ParameterSpec,
) -> ParameterRowResponse {
    parameter_row_impl(ui, label, value, spec, true, None)
}

pub(crate) fn parameter_value_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    spec: ParameterSpec,
) -> ParameterRowResponse {
    // A wire-format bound is an encoding guard, not a useful physical slider
    // range. Numeric entry avoids advertising an uncalibrated sweep range.
    parameter_row_impl(ui, label, value, spec, false, None)
}

pub(crate) fn eq_gain_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    language: Language,
) -> ParameterRowResponse {
    parameter_row_impl(
        ui,
        label,
        value,
        specs::EQ_GAIN,
        true,
        Some(language.tr("Double-click to reset gain to 0 dB", "双击将增益归零")),
    )
}

fn parameter_row_impl(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    spec: ParameterSpec,
    slider: bool,
    reset_hint: Option<&str>,
) -> ParameterRowResponse {
    let original = *value;
    let scale = ui.spacing().interact_size.y / 30.0;
    let stacked = ui.available_width() < 330.0 * scale;
    let response = ui
        .vertical(|ui| {
            if stacked {
                ui.label(egui::RichText::new(label).color(TEXT));
            }
            // horizontal() bounds vertical centering to one control row. A bare
            // with_layout() here would center the slider in the remaining viewport.
            ui.horizontal(|ui| {
                if !stacked {
                    ui.label(egui::RichText::new(label).color(TEXT));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - 100.0 * scale).clamp(36.0, 140.0 * scale);
                    let suffix = match spec.unit {
                        flow8_model::Unit::Decibels => " dB",
                        flow8_model::Unit::Hertz => " Hz",
                        _ => "",
                    };
                    if slider {
                        let mut response = ui.add(
                            egui::Slider::new(value, spec.min..=spec.max)
                                .step_by(spec.step.unwrap_or(0.01) as f64)
                                .suffix(suffix)
                                .show_value(reset_hint.is_none()),
                        );
                        if let Some(hint) = reset_hint {
                            // Keep the value editor separate: double-clicking
                            // the rail resets gain without stealing text edits.
                            if ui.is_enabled() && response.double_clicked() {
                                *value = spec.default;
                                response.mark_changed();
                            }
                            response = response.on_hover_text(hint);
                            let value_response = ui.add(
                                egui::DragValue::new(value)
                                    .range(spec.min..=spec.max)
                                    .speed(spec.step.unwrap_or(0.1))
                                    .suffix(suffix)
                                    .max_decimals(spec.display_decimals as usize),
                            );
                            response = if value_response.gained_focus()
                                || value_response.has_focus()
                                || value_response.lost_focus()
                            {
                                value_response.union(response)
                            } else {
                                response.union(value_response)
                            };
                        }
                        response
                    } else {
                        ui.add(
                            egui::DragValue::new(value)
                                .range(spec.min..=spec.max)
                                .speed(spec.step.unwrap_or(0.1))
                                .suffix(suffix)
                                .max_decimals(if spec.step == Some(1.0) { 0 } else { 2 }),
                        )
                    }
                })
                .inner
            })
            .inner
        })
        .inner;
    // egui may mark Slider::changed when merely snapping a received device
    // value to a display step. That is a rendering detail, not user intent.
    let pointer_edit =
        response.dragged() || response.clicked() || response.is_pointer_button_down_on();
    let keyboard_edit = (response.has_focus() || response.lost_focus())
        && ui.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Key { pressed: true, .. }
                )
            })
        });
    let wheel_edit = response.hovered() && ui.input(|input| input.smooth_scroll_delta.y != 0.0);
    let accessibility_edit = ui.input(|input| {
        use egui::accesskit::Action;
        [Action::Increment, Action::Decrement, Action::SetValue]
            .into_iter()
            .any(|action| input.num_accesskit_action_requests(response.id, action) != 0)
    });
    let user_changed = response.changed()
        && ui.is_enabled()
        && (pointer_edit || keyboard_edit || wheel_edit || accessibility_edit)
        && (*value - original).abs() > 1.0e-6;
    if !user_changed {
        *value = original;
    }
    ParameterRowResponse { user_changed }
}

pub(crate) fn committed_text(
    ui: &mut egui::Ui,
    key: impl std::hash::Hash + std::fmt::Debug,
    value: Option<&str>,
    maximum: usize,
    language: Language,
) -> Option<String> {
    let id = ui.make_persistent_id(key);
    let draft_id = id.with("draft");
    let mut draft = ui
        .ctx()
        .data(|data| data.get_temp::<String>(draft_id))
        .unwrap_or_else(|| value.unwrap_or_default().to_owned());
    let response = ui.add(
        egui::TextEdit::singleline(&mut draft)
            .id(id)
            .hint_text("—")
            .desired_width(ui.available_width())
            .char_limit(maximum),
    );
    let escape = response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape));
    if escape {
        ui.ctx().data_mut(|data| data.remove::<String>(draft_id));
        response.surrender_focus();
        return None;
    }
    if draft.len() > maximum {
        ui.label(
            egui::RichText::new(language.tr("Name is too long", "名称过长"))
                .color(RED)
                .small(),
        );
    }
    let commit = response.lost_focus()
        || response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    if commit && draft.len() <= maximum {
        ui.ctx().data_mut(|data| data.remove::<String>(draft_id));
        return (Some(draft.as_str()) != value).then_some(draft);
    }
    if response.changed()
        || ui
            .ctx()
            .data(|data| data.get_temp::<String>(draft_id))
            .is_some()
    {
        ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
    }
    None
}
