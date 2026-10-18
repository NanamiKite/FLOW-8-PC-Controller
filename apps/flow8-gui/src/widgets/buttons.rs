use crate::metrics::{DESTINATION_BUTTON_MIN_WIDTH, STATE_BUTTON_MIN_WIDTH, TAB_BUTTON_MIN_WIDTH};
use crate::theme::palette;
use eframe::egui::{self, Color32, Response, Stroke, StrokeKind, Vec2};

pub(crate) fn small_tab(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    accent: Color32,
) -> Response {
    let colors = palette(ui);
    selection_button(
        ui,
        selected,
        label,
        accent,
        SelectionButtonStyle {
            text: if selected { accent } else { colors.secondary },
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
    let colors = palette(ui);
    selection_button(
        ui,
        selected,
        label,
        colors.yellow,
        SelectionButtonStyle {
            text: if selected {
                colors.destination_active_text
            } else {
                colors.secondary
            },
            fill: if selected {
                colors.destination_active_fill
            } else {
                colors.surface_alt
            },
            stroke: Stroke::new(
                1.0,
                if selected {
                    colors.yellow
                } else {
                    colors.border
                },
            ),
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
    let colors = palette(ui);
    selection_button(
        ui,
        selected,
        label,
        accent,
        SelectionButtonStyle {
            text: if selected {
                if ui.visuals().dark_mode {
                    Color32::WHITE
                } else {
                    accent
                }
            } else {
                colors.text
            },
            fill: if selected {
                if ui.visuals().dark_mode {
                    accent.gamma_multiply(0.72)
                } else {
                    // A selection must remain distinct from the neutral button
                    // fill, including inside a disabled settings row.
                    accent.gamma_multiply(0.32)
                }
            } else {
                colors.control_surface
            },
            stroke: Stroke::new(
                1.0,
                if selected {
                    if ui.visuals().dark_mode {
                        accent
                    } else {
                        accent.gamma_multiply(0.65)
                    }
                } else {
                    colors.control_border
                },
            ),
            radius: 5,
            min_width: STATE_BUTTON_MIN_WIDTH,
        },
    )
}

pub(crate) fn mute_button(ui: &mut egui::Ui, muted: bool, label: &str) -> Response {
    let colors = palette(ui);
    state_button(ui, muted, label, colors.red)
}

pub(crate) fn state_button_sized(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    accent: Color32,
    size: Vec2,
) -> Response {
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| state_button(ui, selected, label, accent),
    )
    .inner
}

/// Settings selections share one light-theme color, independent of section
/// accents. Keep the original dark-theme colors and semantic warning buttons.
pub(crate) fn setting_button_sized(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    dark_accent: Color32,
    size: Vec2,
) -> Response {
    let accent = if ui.visuals().dark_mode {
        dark_accent
    } else {
        palette(ui).green
    };
    state_button_sized(ui, selected, label, accent, size)
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
    let colors = palette(ui);
    ui.label(
        egui::RichText::new(label)
            .text_style(egui::TextStyle::Button)
            .strong()
            .color(if active {
                colors.active_badge_text
            } else {
                colors.secondary
            })
            .background_color(if active {
                colors.active_badge_surface
            } else {
                colors.control_surface
            }),
    );
}
