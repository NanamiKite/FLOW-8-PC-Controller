use crate::metrics::{INSPECTOR_HEADING_SIZE, UiMetrics};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Color32, FontId, Stroke, Vec2};
use flow8_model::MixDestination;

pub(super) const BG: Color32 = Color32::from_rgb(17, 19, 23);
pub(super) const SURFACE: Color32 = Color32::from_rgb(24, 27, 32);
pub(super) const SURFACE_ALT: Color32 = Color32::from_rgb(27, 30, 36);
pub(super) const BORDER: Color32 = Color32::from_rgb(43, 47, 54);
pub(super) const TEXT: Color32 = Color32::from_rgb(230, 233, 238);
pub(super) const SECONDARY: Color32 = Color32::from_rgb(142, 150, 162);
pub(super) const YELLOW: Color32 = Color32::from_rgb(242, 201, 76);
pub(super) const PURPLE: Color32 = Color32::from_rgb(155, 106, 214);
pub(super) const GREEN: Color32 = Color32::from_rgb(85, 185, 121);
pub(super) const BLUE: Color32 = Color32::from_rgb(79, 146, 232);
pub(super) const RED: Color32 = Color32::from_rgb(215, 71, 82);

pub(super) const TOOLBAR_SURFACE: Color32 = Color32::from_rgb(20, 23, 27);
pub(super) const GRAPH_SURFACE: Color32 = Color32::from_rgb(13, 15, 18);
pub(super) const CONTROL_SURFACE: Color32 = Color32::from_rgb(37, 41, 48);
pub(super) const CONTROL_BORDER: Color32 = Color32::from_rgb(53, 58, 67);
pub(super) const CONTROL_HOVER_SURFACE: Color32 = Color32::from_rgb(55, 62, 73);
pub(super) const CONTROL_HOVER_BORDER: Color32 = Color32::from_rgb(130, 145, 164);
pub(super) const CONTROL_ACTIVE_SURFACE: Color32 = Color32::from_rgb(72, 79, 91);
pub(super) const CONTROL_OPEN_SURFACE: Color32 = Color32::from_rgb(49, 55, 65);
pub(super) const CONTROL_TRACK: Color32 = Color32::from_rgb(52, 57, 67);
pub(super) const CONTROL_THUMB: Color32 = Color32::from_rgb(215, 220, 227);
pub(super) const CONTROL_THUMB_BORDER: Color32 = Color32::from_rgb(90, 96, 105);
pub(super) const MIXER_DIVIDER: Color32 = Color32::from_rgb(78, 88, 103);
pub(super) const DESTINATION_ACTIVE_TEXT: Color32 = Color32::from_rgb(24, 25, 27);
pub(super) const DESTINATION_ACTIVE_FILL: Color32 = Color32::from_rgb(225, 185, 65);
pub(super) const ACTIVE_BADGE_TEXT: Color32 = Color32::from_rgb(200, 240, 213);
pub(super) const ACTIVE_BADGE_SURFACE: Color32 = Color32::from_rgb(27, 48, 35);
pub(super) const STAGE_SURFACE: Color32 = Color32::from_rgb(14, 16, 19);
pub(super) const STAGE_SELECTED_SURFACE: Color32 = Color32::from_rgb(38, 36, 27);
pub(super) const STAGE_RING: Color32 = Color32::from_rgb(50, 55, 63);
pub(super) const METER_SURFACE: Color32 = Color32::from_rgb(12, 14, 17);
pub(super) const MASTER_SURFACE: Color32 = Color32::from_rgb(29, 35, 45);

pub(super) fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(8)
        .inner_margin(12)
}

pub(super) fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_frame().show(ui, add).inner
}

pub(super) fn topology_bus_color(destination: MixDestination) -> Color32 {
    match destination {
        MixDestination::Main => BLUE,
        MixDestination::Monitor1 | MixDestination::Monitor2 => GREEN,
        MixDestination::Fx1 | MixDestination::Fx2 => PURPLE,
    }
}

pub(super) fn configure_style(context: &egui::Context, metrics: UiMetrics) {
    context.set_theme(egui::Theme::Dark);
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::proportional(metrics.heading_font),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::proportional(metrics.body_font),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::proportional(metrics.button_font),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::proportional(metrics.small_font),
    );
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SURFACE;
    style.visuals.widgets.inactive.bg_fill = CONTROL_SURFACE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, CONTROL_BORDER);
    style.visuals.widgets.hovered.bg_fill = CONTROL_HOVER_SURFACE;
    // egui includes this stroke width in Button layout on the next frame.
    // Keep it constant so hover/focus cannot move neighboring top-bar buttons.
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, CONTROL_HOVER_BORDER);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.active.bg_fill = CONTROL_ACTIVE_SURFACE;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, YELLOW);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.open.bg_fill = CONTROL_OPEN_SURFACE;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.selection.bg_fill = BLUE.gamma_multiply(0.55);
    style.visuals.selection.stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    style.spacing.item_spacing = Vec2::splat(metrics.spacing);
    style.spacing.button_padding =
        Vec2::new(12.0 * metrics.ui_scale, 7.0 * metrics.ui_scale).round_ui();
    style.spacing.interact_size.y = metrics.control_height;
    style.animation_time = 0.14;
    context.set_style_of(egui::Theme::Dark, style);
}

pub(super) fn section_heading(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.label(egui::RichText::new(text).strong().color(color))
}

pub(super) fn inspector_heading(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.label(
        egui::RichText::new(text)
            .size(INSPECTOR_HEADING_SIZE)
            .strong()
            .color(color),
    )
}
