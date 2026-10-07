use crate::metrics::{DESTINATION_BUTTON_MIN_WIDTH, STATE_BUTTON_MIN_WIDTH, TAB_BUTTON_MIN_WIDTH};
use crate::theme::{
    ACTIVE_BADGE_SURFACE, ACTIVE_BADGE_TEXT, BORDER, CONTROL_BORDER, CONTROL_SURFACE,
    DESTINATION_ACTIVE_FILL, DESTINATION_ACTIVE_TEXT, RED, SECONDARY, SURFACE_ALT, TEXT, YELLOW,
};
use eframe::egui::{self, Color32, Response, Stroke, StrokeKind, Vec2};

pub(crate) fn small_tab(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    accent: Color32,
) -> Response {
    selection_button(
        ui,
        selected,
        label,
        accent,
        SelectionButtonStyle {
            text: if selected { accent } else { SECONDARY },
            fill: if selected {
                accent.gamma_multiply(0.17)
            } else {
                Color32::TRANSPARENT
            },
            stroke: Stroke::new(0.0, Color32::TRANSPARENT),
            radius: 5,
            min_width: TAB_BUTTON_MIN_WIDTH,
        },
    )
}

pub(crate) fn destination_button(ui: &mut egui::Ui, selected: bool, label: &str) -> Response {
    selection_button(
        ui,
        selected,
        label,
        YELLOW,
        SelectionButtonStyle {
            text: if selected {
                DESTINATION_ACTIVE_TEXT
            } else {
                SECONDARY
            },
            fill: if selected {
                DESTINATION_ACTIVE_FILL
            } else {
                SURFACE_ALT
            },
            stroke: Stroke::new(1.0, if selected { YELLOW } else { BORDER }),
            radius: 6,
            min_width: DESTINATION_BUTTON_MIN_WIDTH,
        },
    )
}

pub(crate) fn state_button(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    accent: Color32,
) -> Response {
    selection_button(
        ui,
        selected,
        label,
        accent,
        SelectionButtonStyle {
            text: if selected { Color32::WHITE } else { TEXT },
            fill: if selected {
                accent.gamma_multiply(0.72)
            } else {
                CONTROL_SURFACE
            },
            stroke: Stroke::new(1.0, if selected { accent } else { CONTROL_BORDER }),
            radius: 5,
            min_width: STATE_BUTTON_MIN_WIDTH,
        },
    )
}

pub(crate) fn mute_button(ui: &mut egui::Ui, muted: bool, label: &str) -> Response {
    state_button(ui, muted, label, RED)
}

struct SelectionButtonStyle {
    text: Color32,
    fill: Color32,
    stroke: Stroke,
    radius: u8,
    min_width: f32,
}

fn selection_button(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    accent: Color32,
    style: SelectionButtonStyle,
) -> Response {
    let control_height = ui.spacing().interact_size.y;
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).strong().color(style.text))
            .fill(style.fill)
            .stroke(style.stroke)
            .corner_radius(style.radius)
            .min_size(Vec2::new(style.min_width, control_height))
            .selected(selected),
    );
    enhanced_click_feedback(ui, response, selected, accent)
}

fn enhanced_click_feedback(
    ui: &egui::Ui,
    response: Response,
    selected: bool,
    accent: Color32,
) -> Response {
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(response.rect.shrink(1.0), 5.0, accent.gamma_multiply(0.28));
    }
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            5.0,
            Stroke::new(if selected { 2.0 } else { 1.5 }, accent),
            StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(crate) fn badge(ui: &mut egui::Ui, label: &str, active: bool) {
    ui.label(
        egui::RichText::new(label)
            .size(8.0)
            .color(if active { ACTIVE_BADGE_TEXT } else { SECONDARY })
            .background_color(if active {
                ACTIVE_BADGE_SURFACE
            } else {
                CONTROL_SURFACE
            }),
    );
}
