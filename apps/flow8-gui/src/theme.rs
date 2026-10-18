use crate::metrics::UiMetrics;
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Color32, FontId, Stroke, Vec2};
use flow8_model::MixDestination;

/// Shared semantic colors for standard egui controls and our painted controls.
/// Read the palette from the current Ui, never from global mutable state.
#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) bg: Color32,
    pub(super) surface: Color32,
    pub(super) surface_alt: Color32,
    pub(super) border: Color32,
    pub(super) text: Color32,
    pub(super) secondary: Color32,
    pub(super) yellow: Color32,
    pub(super) purple: Color32,
    pub(super) green: Color32,
    pub(super) blue: Color32,
    pub(super) red: Color32,
    pub(super) toolbar_surface: Color32,
    pub(super) graph_surface: Color32,
    pub(super) control_surface: Color32,
    pub(super) control_border: Color32,
    pub(super) control_hover_surface: Color32,
    pub(super) control_hover_border: Color32,
    pub(super) control_active_surface: Color32,
    pub(super) control_open_surface: Color32,
    pub(super) control_track: Color32,
    pub(super) control_thumb: Color32,
    pub(super) control_thumb_border: Color32,
    pub(super) mixer_divider: Color32,
    pub(super) destination_active_text: Color32,
    pub(super) destination_active_fill: Color32,
    pub(super) active_badge_text: Color32,
    pub(super) active_badge_surface: Color32,
    pub(super) stage_surface: Color32,
    pub(super) stage_selected_surface: Color32,
    pub(super) stage_ring: Color32,
    pub(super) meter_surface: Color32,
    pub(super) master_surface: Color32,
}

const DARK: Palette = Palette {
    bg: Color32::from_rgb(17, 19, 23),
    surface: Color32::from_rgb(24, 27, 32),
    surface_alt: Color32::from_rgb(27, 30, 36),
    border: Color32::from_rgb(43, 47, 54),
    text: Color32::from_rgb(230, 233, 238),
    secondary: Color32::from_rgb(142, 150, 162),
    yellow: Color32::from_rgb(242, 201, 76),
    purple: Color32::from_rgb(155, 106, 214),
    green: Color32::from_rgb(85, 185, 121),
    blue: Color32::from_rgb(79, 146, 232),
    red: Color32::from_rgb(215, 71, 82),
    toolbar_surface: Color32::from_rgb(20, 23, 27),
    graph_surface: Color32::from_rgb(13, 15, 18),
    control_surface: Color32::from_rgb(37, 41, 48),
    control_border: Color32::from_rgb(53, 58, 67),
    control_hover_surface: Color32::from_rgb(55, 62, 73),
    control_hover_border: Color32::from_rgb(130, 145, 164),
    control_active_surface: Color32::from_rgb(72, 79, 91),
    control_open_surface: Color32::from_rgb(49, 55, 65),
    control_track: Color32::from_rgb(52, 57, 67),
    control_thumb: Color32::from_rgb(215, 220, 227),
    control_thumb_border: Color32::from_rgb(90, 96, 105),
    mixer_divider: Color32::from_rgb(78, 88, 103),
    destination_active_text: Color32::from_rgb(24, 25, 27),
    destination_active_fill: Color32::from_rgb(225, 185, 65),
    active_badge_text: Color32::from_rgb(200, 240, 213),
    active_badge_surface: Color32::from_rgb(27, 48, 35),
    stage_surface: Color32::from_rgb(14, 16, 19),
    stage_selected_surface: Color32::from_rgb(38, 36, 27),
    stage_ring: Color32::from_rgb(50, 55, 63),
    meter_surface: Color32::from_rgb(12, 14, 17),
    master_surface: Color32::from_rgb(29, 35, 45),
};

const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(233, 234, 231),
    surface: Color32::from_rgb(249, 249, 247),
    surface_alt: Color32::from_rgb(239, 241, 238),
    border: Color32::from_rgb(208, 213, 211),
    text: Color32::from_rgb(52, 61, 67),
    secondary: Color32::from_rgb(99, 110, 117),
    yellow: Color32::from_rgb(151, 116, 45),
    purple: Color32::from_rgb(119, 98, 143),
    green: Color32::from_rgb(67, 119, 90),
    blue: Color32::from_rgb(70, 103, 139),
    red: Color32::from_rgb(164, 78, 76),
    toolbar_surface: Color32::from_rgb(242, 244, 241),
    graph_surface: Color32::from_rgb(239, 242, 239),
    control_surface: Color32::from_rgb(232, 236, 233),
    control_border: Color32::from_rgb(195, 203, 201),
    control_hover_surface: Color32::from_rgb(222, 230, 229),
    control_hover_border: Color32::from_rgb(154, 173, 179),
    control_active_surface: Color32::from_rgb(213, 225, 228),
    control_open_surface: Color32::from_rgb(225, 232, 231),
    control_track: Color32::from_rgb(139, 154, 158),
    control_thumb: Color32::from_rgb(252, 252, 249),
    control_thumb_border: Color32::from_rgb(155, 170, 173),
    mixer_divider: Color32::from_rgb(196, 206, 204),
    destination_active_text: Color32::from_rgb(111, 86, 41),
    destination_active_fill: Color32::from_rgb(238, 227, 196),
    active_badge_text: Color32::from_rgb(62, 110, 82),
    active_badge_surface: Color32::from_rgb(221, 234, 224),
    stage_surface: Color32::from_rgb(225, 230, 225),
    stage_selected_surface: Color32::from_rgb(243, 237, 219),
    stage_ring: Color32::from_rgb(193, 205, 199),
    meter_surface: Color32::from_rgb(215, 223, 218),
    master_surface: Color32::from_rgb(232, 238, 240),
};

pub(super) fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

pub(super) fn card_frame(ui: &egui::Ui) -> egui::Frame {
    let colors = palette(ui);
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(8)
        .inner_margin(12)
}

pub(super) fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_frame(ui).show(ui, add).inner
}

pub(super) fn topology_bus_color(ui: &egui::Ui, destination: MixDestination) -> Color32 {
    let colors = palette(ui);
    match destination {
        MixDestination::Main => colors.blue,
        MixDestination::Monitor1 | MixDestination::Monitor2 => colors.green,
        MixDestination::Fx1 | MixDestination::Fx2 => colors.purple,
    }
}

pub(super) fn configure_style(context: &egui::Context, metrics: UiMetrics) {
    // Install both styles without resetting the user's choice when resizing.
    for (theme, colors) in [(egui::Theme::Dark, DARK), (egui::Theme::Light, LIGHT)] {
        let mut style = (*context.style_of(theme)).clone();
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
        style.visuals = match theme {
            egui::Theme::Dark => egui::Visuals::dark(),
            egui::Theme::Light => egui::Visuals::light(),
        };
        style.visuals.panel_fill = colors.bg;
        style.visuals.window_fill = colors.surface;
        style.visuals.widgets.inactive.bg_fill = colors.control_surface;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, colors.control_border);
        style.visuals.widgets.hovered.bg_fill = colors.control_hover_surface;
        // egui includes this stroke width in Button layout on the next frame.
        // Keep it constant so hover/focus cannot move neighboring top-bar buttons.
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, colors.control_hover_border);
        let interactive_text = match theme {
            egui::Theme::Dark => Color32::WHITE,
            egui::Theme::Light => colors.text,
        };
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, interactive_text);
        style.visuals.widgets.active.bg_fill = colors.control_active_surface;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, colors.yellow);
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, interactive_text);
        style.visuals.widgets.open.bg_fill = colors.control_open_surface;
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, colors.text);
        style.visuals.selection.bg_fill = match theme {
            egui::Theme::Dark => colors.blue.gamma_multiply(0.55),
            egui::Theme::Light => colors.blue.gamma_multiply(0.14),
        };
        style.visuals.selection.stroke = Stroke::new(
            1.5,
            match theme {
                egui::Theme::Dark => Color32::WHITE,
                egui::Theme::Light => colors.blue,
            },
        );
        if theme == egui::Theme::Light {
            // Offline sections can contain another disabled control group. The
            // default 0.5 compounds to 0.25 and makes labels nearly disappear.
            // Keep disabled content readable; interaction remains disabled.
            style.visuals.disabled_alpha = 0.86;
            style.visuals.weak_text_color = Some(colors.secondary);
            style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, colors.text);
            style.visuals.widgets.inactive.weak_bg_fill = colors.control_surface;
            style.visuals.widgets.hovered.weak_bg_fill = colors.control_hover_surface;
            style.visuals.widgets.active.weak_bg_fill = colors.control_active_surface;
            style.visuals.widgets.open.weak_bg_fill = colors.control_open_surface;
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors.border);
            style.visuals.window_stroke = Stroke::new(1.0, colors.border);
        }
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
        style.spacing.item_spacing = Vec2::splat(metrics.spacing);
        style.spacing.button_padding =
            Vec2::new(12.0 * metrics.ui_scale, 7.0 * metrics.ui_scale).round_ui();
        style.spacing.interact_size.y = metrics.control_height;
        style.animation_time = 0.14;
        context.set_style_of(theme, style);
    }
}

pub(super) fn section_heading(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.label(
        egui::RichText::new(text)
            .text_style(egui::TextStyle::Body)
            .strong()
            .color(color),
    )
}
