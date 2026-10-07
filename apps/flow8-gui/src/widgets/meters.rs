use crate::theme::{GREEN, METER_SURFACE, RED, YELLOW};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Pos2, Rect, Sense, Vec2};
use flow8_model::specs;

pub(crate) fn meter_widget_sized(ui: &mut egui::Ui, db: f32, width: f32, height: f32) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height).round_ui(), Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            ui.is_enabled(),
            format!("Signal level {db:.1} dB"),
        )
    });
    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + 3.0, rect.top() + 2.0),
        Pos2::new(rect.left() + 14.0, rect.bottom() - 2.0),
    );
    ui.painter().rect_filled(bar, 2.0, METER_SURFACE);
    let normalized = ((db - specs::METER_DISPLAY.min)
        / (specs::METER_DISPLAY.max - specs::METER_DISPLAY.min))
        .clamp(0.0, 1.0);
    let top = egui::lerp(bar.bottom()..=bar.top(), normalized);
    let color = if db >= 9.5 {
        RED
    } else if db >= -6.0 {
        YELLOW
    } else {
        GREEN
    };
    ui.painter().rect_filled(
        Rect::from_min_max(Pos2::new(bar.left(), top), bar.right_bottom()),
        2.0,
        color,
    );
}
