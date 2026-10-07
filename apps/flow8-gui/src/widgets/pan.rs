use crate::metrics::UiMetrics;
use crate::theme::{BLUE, CONTROL_THUMB, CONTROL_TRACK, SECONDARY};
use crate::widgets::{accessible_slider_input, accessible_slider_node};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Color32, Pos2, Response, Sense, Stroke, StrokeKind, Vec2};
use flow8_model::specs;

pub(crate) fn pan_control(
    ui: &mut egui::Ui,
    value: &mut f32,
    width: f32,
    metrics: UiMetrics,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(
        Vec2::new(width, 20.0 * metrics.ui_scale).round_ui(),
        Sense::click_and_drag(),
    );
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        response.request_focus();
        let next = ((pointer.x - rect.left()) / rect.width() * 2.0 - 1.0).clamp(-1.0, 1.0);
        if (*value - next).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
        }
    }
    if response.double_clicked() {
        *value = 0.0;
        response.mark_changed();
    }
    if ui.is_enabled() && response.has_focus() {
        let next = ui.input(|input| {
            let step = if input.modifiers.shift { 0.005 } else { 0.05 };
            if input.key_pressed(egui::Key::ArrowLeft) {
                (*value - step).max(specs::PAN.min)
            } else if input.key_pressed(egui::Key::ArrowRight) {
                (*value + step).min(specs::PAN.max)
            } else if input.key_pressed(egui::Key::Home) {
                specs::PAN.default
            } else {
                *value
            }
        });
        if next != *value {
            *value = next;
            response.mark_changed();
        }
        ui.painter().rect_stroke(
            rect.expand(2.0),
            3.0,
            Stroke::new(1.0, BLUE),
            StrokeKind::Inside,
        );
    }
    if accessible_slider_input(ui, &response, value, specs::PAN.min, specs::PAN.max, 0.05) {
        response.mark_changed();
    }
    let track_y = rect.center().y;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), track_y),
            Pos2::new(rect.right(), track_y),
        ],
        Stroke::new(3.0, CONTROL_TRACK),
    );
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, SECONDARY),
    );
    let x = egui::lerp(rect.left()..=rect.right(), (*value + 1.0) / 2.0);
    ui.painter().circle_filled(
        Pos2::new(x, track_y),
        6.0,
        if response.hovered() {
            Color32::WHITE
        } else {
            CONTROL_THUMB
        },
    );
    let cursor = if response.dragged() {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), "Pan / Balance")
    });
    accessible_slider_node(ui, &response, *value, specs::PAN.min, specs::PAN.max, 0.05);
    response.on_hover_cursor(cursor)
}

pub(crate) fn pan_text(value: f32, balance: bool) -> String {
    if value.abs() < 0.005 {
        if balance { "BAL C" } else { "PAN C" }.into()
    } else if value < 0.0 {
        format!("L {:.0}", value.abs() * 100.0)
    } else {
        format!("R {:.0}", value * 100.0)
    }
}
