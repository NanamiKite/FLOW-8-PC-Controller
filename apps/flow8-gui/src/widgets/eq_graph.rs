use crate::i18n::Language;
use crate::metrics::UiMetrics;
use crate::theme::{BLUE, BORDER, GRAPH_SURFACE, SECONDARY, YELLOW};
use crate::widgets::{accessible_slider_input, accessible_slider_node};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2};
use flow8_model::{EqState, InputChannelState, specs};

pub(crate) fn eq_gain_graph(
    ui: &mut egui::Ui,
    channel: &InputChannelState,
    metrics: UiMetrics,
    language: Language,
) -> Option<(usize, f32)> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 96.0 * metrics.ui_scale),
        Sense::hover(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            ui.is_enabled(),
            "EQ response graph; use the band controls to edit frequency, Q and gain",
        )
    });
    ui.painter().rect_filled(rect, 4.0, GRAPH_SURFACE);
    for index in 1..4 {
        let x = egui::lerp(rect.left()..=rect.right(), index as f32 / 4.0);
        ui.painter()
            .vline(x, rect.y_range(), Stroke::new(1.0, BORDER));
    }
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, BORDER));
    let mut points = Vec::new();
    let mut edit = None;
    for (index, band) in channel.eq.bands.iter().enumerate() {
        let x = egui::lerp(rect.left()..=rect.right(), (index as f32 + 0.5) / 4.0);
        let gain = *band.gain_db.effective().unwrap_or(&0.0);
        let y = egui::lerp(rect.bottom()..=rect.top(), (gain + 15.0) / 30.0);
        points.push(Pos2::new(x, y));
        let editable = channel.capabilities.peq
            && band.frequency_hz.effective().is_some()
            && band.q.effective().is_some()
            && band.gain_db.effective().is_some();
        // Each band owns a disjoint column, not just a small moving node.
        // Double-clicking anywhere in it resets that band's gain only.
        let hit_rect = Rect::from_min_max(
            Pos2::new(
                egui::lerp(rect.left()..=rect.right(), index as f32 / 4.0),
                rect.top(),
            ),
            Pos2::new(
                egui::lerp(rect.left()..=rect.right(), (index as f32 + 1.0) / 4.0),
                rect.bottom(),
            ),
        );
        let response = ui
            .interact(
                hit_rect,
                ui.id().with(("input-eq-band", index)),
                if editable {
                    Sense::click_and_drag()
                } else {
                    Sense::hover()
                },
            )
            .on_hover_cursor(if editable {
                egui::CursorIcon::ResizeVertical
            } else {
                egui::CursorIcon::Default
            })
            .on_hover_text(language.tr(
                "Drag to adjust gain; double-click to reset to 0 dB",
                "拖动调整增益；双击归零",
            ));
        let next = eq_band_control(ui, &response, index, gain, editable, rect.height());
        if let Some(gain) = next {
            edit = Some((index, gain));
        }
        draw_eq_point(
            ui,
            Pos2::new(x, y),
            metrics,
            response.hovered() || response.has_focus(),
            YELLOW,
        );
    }
    draw_eq_line(ui, points, YELLOW);
    edit
}

fn eq_band_control(
    ui: &egui::Ui,
    response: &Response,
    index: usize,
    gain: f32,
    editable: bool,
    plot_height: f32,
) -> Option<f32> {
    let label = format!("EQ band {} gain", index + 1);
    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled() && editable, f64::from(gain), &label)
    });
    if !editable || !ui.is_enabled() {
        return None;
    }
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    let mut next = gain;
    if response.double_clicked() {
        next = specs::EQ_GAIN.default;
    } else if response.dragged() {
        let fine = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
        next -= response.drag_delta().y / plot_height.max(1.0)
            * (specs::EQ_GAIN.max - specs::EQ_GAIN.min)
            * fine;
    }
    if response.has_focus() {
        next = ui.input(|input| {
            let step = if input.modifiers.shift { 0.01 } else { 0.1 };
            if input.key_pressed(egui::Key::ArrowUp) || input.key_pressed(egui::Key::ArrowRight) {
                next + step
            } else if input.key_pressed(egui::Key::ArrowDown)
                || input.key_pressed(egui::Key::ArrowLeft)
            {
                next - step
            } else if input.key_pressed(egui::Key::Home) {
                specs::EQ_GAIN.default
            } else {
                next
            }
        });
        ui.painter().rect_stroke(
            response.rect,
            3.0,
            Stroke::new(1.0, BLUE),
            StrokeKind::Inside,
        );
    }
    accessible_slider_input(
        ui,
        response,
        &mut next,
        specs::EQ_GAIN.min,
        specs::EQ_GAIN.max,
        0.1,
    );
    accessible_slider_node(
        ui,
        response,
        gain,
        specs::EQ_GAIN.min,
        specs::EQ_GAIN.max,
        0.1,
    );
    next = next.clamp(specs::EQ_GAIN.min, specs::EQ_GAIN.max);
    (next.is_finite() && (next - gain).abs() > f32::EPSILON).then_some(next)
}

pub(crate) fn output_eq_graph(
    ui: &mut egui::Ui,
    eq: &EqState,
    metrics: UiMetrics,
) -> Option<(usize, f32)> {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 176.0 * metrics.ui_scale).round_ui(),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 5.0, GRAPH_SURFACE);
    // Inset both end points and their labels so the scroll viewport never cuts them off.
    let plot = Rect::from_min_max(
        rect.min + Vec2::new(30.0, 14.0) * metrics.ui_scale,
        rect.max - Vec2::new(18.0, 34.0) * metrics.ui_scale,
    );
    if eq.bands.is_empty() || plot.width() <= 0.0 || plot.height() <= 0.0 {
        return None;
    }
    let gain_range = specs::EQ_GAIN.max - specs::EQ_GAIN.min;
    for gain in [specs::EQ_GAIN.min, 0.0, specs::EQ_GAIN.max] {
        let y = egui::lerp(
            plot.bottom()..=plot.top(),
            (gain - specs::EQ_GAIN.min) / gain_range,
        );
        ui.painter().hline(
            plot.x_range(),
            y,
            Stroke::new(1.0, if gain == 0.0 { SECONDARY } else { BORDER }),
        );
        ui.painter().text(
            Pos2::new(plot.left() - 5.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{gain:+.0}"),
            FontId::proportional(metrics.small_font),
            SECONDARY,
        );
    }
    let steps = eq.bands.len().saturating_sub(1).max(1) as f32;
    let points: Vec<Pos2> = eq
        .bands
        .iter()
        .enumerate()
        .map(|(index, band)| {
            let gain = band.gain_db.effective().copied().unwrap_or(0.0);
            Pos2::new(
                egui::lerp(plot.left()..=plot.right(), index as f32 / steps),
                egui::lerp(
                    plot.bottom()..=plot.top(),
                    ((gain - specs::EQ_GAIN.min) / gain_range).clamp(0.0, 1.0),
                ),
            )
        })
        .collect();
    // This is a band-gain line, not an invented DSP frequency response.
    draw_eq_line(ui, points.clone(), BLUE);
    let mut edit = None;
    for (index, (point, band)) in points.iter().zip(&eq.bands).enumerate() {
        let frequency = band.frequency_hz.effective().copied();
        let gain = band.gain_db.effective().copied().unwrap_or(0.0);
        let frequency_text = match frequency {
            Some(hz) if hz >= 1000.0 => format!("{}k", hz / 1000.0),
            Some(hz) => format!("{hz:.0}"),
            None => "—".to_owned(),
        };
        ui.painter()
            .vline(point.x, plot.y_range(), Stroke::new(1.0, BORDER));
        let label_row = if index % 2 == 0 {
            0.0
        } else {
            metrics.small_font + 2.0
        };
        ui.painter().text(
            Pos2::new(point.x, plot.bottom() + 5.0 + label_row),
            egui::Align2::CENTER_TOP,
            frequency_text,
            FontId::proportional(metrics.small_font),
            SECONDARY,
        );
        let hit_size = Vec2::splat((plot.width() / (steps + 1.0)).min(20.0 * metrics.ui_scale));
        let editable = frequency.is_some()
            && band.gain_db.effective().is_some()
            && band.q.effective().is_some();
        let response = ui
            .interact(
                Rect::from_center_size(*point, hit_size),
                ui.id().with(("output-eq-band", index)),
                if editable {
                    Sense::click_and_drag()
                } else {
                    Sense::hover()
                },
            )
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
            .on_hover_text(format!(
                "{} Hz · {gain:+.1} dB",
                frequency
                    .map(|hz| format!("{hz:.0}"))
                    .unwrap_or_else(|| "—".into())
            ));
        draw_eq_point(
            ui,
            *point,
            metrics,
            response.hovered() || response.dragged(),
            BLUE,
        );
        if let Some(next) = eq_band_control(ui, &response, index, gain, editable, plot.height()) {
            edit = Some((index, next));
        }
    }
    edit
}

fn draw_eq_point(
    ui: &egui::Ui,
    point: Pos2,
    metrics: UiMetrics,
    highlighted: bool,
    accent: Color32,
) {
    ui.painter().circle_filled(
        point,
        4.0 * metrics.ui_scale,
        if highlighted { Color32::WHITE } else { accent },
    );
}

fn draw_eq_line(ui: &egui::Ui, points: Vec<Pos2>, accent: Color32) {
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.0, accent)));
}
