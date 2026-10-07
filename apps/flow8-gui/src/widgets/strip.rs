use crate::i18n::{Language, display_name};
use crate::metrics::UiMetrics;
use crate::theme::{BLUE, BORDER, MIXER_DIVIDER, RED, SECONDARY, SURFACE, TEXT, YELLOW};
use crate::widgets::{
    badge, fader_level_text, fader_sized, meter_widget_sized, pan_control, pan_text, state_button,
};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Align, Layout, Sense, Stroke, Vec2};
use flow8_model::{InputChannelState, InputId, MixDestination};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum StripAction {
    Select,
    Route(f32),
    Pan(f32),
    Mute(bool),
    Solo(bool),
}

pub(crate) fn mixer_divider(ui: &mut egui::Ui, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, MIXER_DIVIDER),
    );
}

fn strip_controls_height(metrics: UiMetrics, show_icons: bool) -> f32 {
    ((if show_icons { 182.0 } else { 152.0 }) * metrics.ui_scale).round_ui()
}

pub(crate) fn strip_controls(
    ui: &mut egui::Ui,
    width: f32,
    metrics: UiMetrics,
    show_icons: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    // Allocate the same fixed header for inputs and master. Unlike a minimum
    // height, this cannot move the fader endpoints when header contents differ.
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, strip_controls_height(metrics, show_icons)).round_ui(),
        Sense::hover(),
    );
    // Use the same aligned bounds for child layout and clipping.
    let rect = rect.round_ui();
    let mut controls = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("strip-controls")
            .max_rect(rect)
            .layout(Layout::top_down(Align::Center)),
    );
    controls.set_clip_rect(ui.clip_rect().intersect(rect));
    add(&mut controls);
}

pub(crate) fn channel_strip(
    ui: &mut egui::Ui,
    channel: &InputChannelState,
    meter_db: f32,
    eq_modified: bool,
    destination: MixDestination,
    selected: bool,
    source_heading: &'static str,
    language: Language,
    show_channel_icons: bool,
    show_mute_buttons: bool,
    metrics: UiMetrics,
    strip_width: f32,
    fader_height: f32,
) -> Option<StripAction> {
    let mut action = None;
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, if selected { YELLOW } else { BORDER }))
        .corner_radius(8)
        .inner_margin(8)
        .show(ui, |ui| {
            // strip_width includes the frame margin and stroke.
            let content_width = (strip_width - 18.0).max(100.0).round_ui();
            ui.set_width(content_width);
            ui.vertical_centered(|ui| {
                strip_controls(ui, content_width, metrics, show_channel_icons, |ui| {
                    ui.label(
                        egui::RichText::new(source_heading)
                            .size(metrics.small_font)
                            .strong()
                            .color(SECONDARY),
                    );
                    let icon = if channel.id == InputId::UsbBluetooth {
                        "▣"
                    } else if channel.id.is_stereo() {
                        "◫"
                    } else {
                        "●"
                    };
                    if show_channel_icons {
                        ui.label(
                            egui::RichText::new(icon)
                                .size(20.0 * metrics.ui_scale)
                                .color(if selected { YELLOW } else { TEXT }),
                        );
                    }
                    let name = channel
                        .name
                        .effective()
                        .map(String::as_str)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| display_name(channel.id, language));
                    if ui
                        .add_sized(
                            [content_width, metrics.control_height],
                            egui::Button::new(
                                egui::RichText::new(name)
                                    .size(metrics.body_font)
                                    .strong()
                                    .color(TEXT),
                            )
                            .selected(selected)
                            .truncate(),
                        )
                        .on_hover_text(name)
                        .clicked()
                    {
                        action = Some(StripAction::Select);
                    }
                    let phantom = channel.capabilities.phantom
                        && *channel.phantom_48v.effective().unwrap_or(&false);
                    let status = if phantom {
                        "48 V"
                    } else if channel.id.is_stereo() {
                        language.tr("STEREO", "立体声")
                    } else {
                        " "
                    };
                    ui.label(
                        egui::RichText::new(status)
                            .size(metrics.small_font)
                            .color(if phantom { RED } else { BLUE }),
                    );
                    ui.add_space(4.0);
                    let mut pan = *channel.pan.effective().unwrap_or(&0.0);
                    let pan_response = pan_control(
                        ui,
                        &mut pan,
                        (content_width - 14.0 * metrics.ui_scale).max(72.0),
                        metrics,
                    );
                    if pan_response.changed() {
                        action = Some(StripAction::Pan(pan));
                    }
                    ui.label(
                        egui::RichText::new(pan_text(pan, channel.id.is_stereo()))
                            .size(metrics.small_font)
                            .color(SECONDARY),
                    );
                    ui.add_space(4.0);
                });
                let route_state = &channel.route_levels[destination.index()];
                let mut route = *route_state.effective().unwrap_or(&0.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(content_width, fader_height),
                    Layout::left_to_right(Align::Min),
                    |ui| {
                        let fader_width = (content_width * 0.65)
                            .clamp(68.0, 82.0)
                            .min(content_width - metrics.spacing - 20.0)
                            .round_ui();
                        let meter_width = (content_width - fader_width - metrics.spacing).max(20.0);
                        let changed = fader_sized(
                            ui,
                            &mut route,
                            fader_width,
                            fader_height,
                            0.75,
                            route_state.confirmed,
                        )
                        .on_hover_text(language.tr(
                            "Drag to adjust · Shift: fine adjustment · Double-click: 0 dB",
                            "拖动调节 · Shift 精调 · 双击回到 0 dB",
                        ))
                        .changed();
                        meter_widget_sized(ui, meter_db, meter_width, fader_height);
                        if changed {
                            action = Some(StripAction::Route(route));
                        }
                    },
                );
                ui.label(
                    egui::RichText::new(format!("{} dB", fader_level_text(route)))
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    if show_mute_buttons {
                        let muted = *channel.muted.effective().unwrap_or(&false);
                        if state_button(ui, muted, language.tr("MUTE", "静音"), RED).clicked() {
                            action = Some(StripAction::Mute(!muted));
                        }
                    }
                    let soloed = *channel.soloed.effective().unwrap_or(&false);
                    if state_button(ui, soloed, language.tr("SOLO", "独奏"), YELLOW).clicked() {
                        action = Some(StripAction::Solo(!soloed));
                    }
                });
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    badge(ui, "EQ", eq_modified);
                    badge(
                        ui,
                        "COMP",
                        channel
                            .compressor
                            .amount
                            .effective()
                            .copied()
                            .unwrap_or(0.0)
                            > 0.01,
                    );
                });
            });
        });
    action
}
