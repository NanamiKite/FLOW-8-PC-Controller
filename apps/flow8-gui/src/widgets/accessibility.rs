use eframe::egui::{self, Response};

pub(crate) fn accessible_slider_input(
    ui: &egui::Ui,
    response: &Response,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
) -> bool {
    use egui::accesskit::{Action, ActionData};

    if !ui.is_enabled() {
        return false;
    }
    let requested = ui.input(|input| {
        let increment = input.num_accesskit_action_requests(response.id, Action::Increment);
        let decrement = input.num_accesskit_action_requests(response.id, Action::Decrement);
        let mut requested = (increment != 0 || decrement != 0)
            .then(|| f64::from(*value) + (increment as f64 - decrement as f64) * f64::from(step));
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(next)) = request.data
                && next.is_finite()
            {
                requested = Some(next);
            }
        }
        requested
    });
    // Do not normalize an untouched device value while merely rendering.
    // This is the same intent-only path used by mouse and keyboard controls.
    if let Some(next) = requested.filter(|value| value.is_finite()) {
        let next = next.clamp(f64::from(min), f64::from(max)) as f32;
        if next != *value {
            *value = next;
            return true;
        }
    }
    false
}

pub(crate) fn accessible_slider_node(
    ui: &egui::Ui,
    response: &Response,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
) {
    use egui::accesskit::Action;

    ui.ctx().accesskit_node_builder(response.id, |builder| {
        builder.set_min_numeric_value(f64::from(min));
        builder.set_max_numeric_value(f64::from(max));
        builder.set_numeric_value_step(f64::from(step));
        if ui.is_enabled() {
            builder.add_action(Action::SetValue);
            if value < max {
                builder.add_action(Action::Increment);
            }
            if value > min {
                builder.add_action(Action::Decrement);
            }
        }
    });
}
