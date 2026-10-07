use crate::theme::{BLUE, CONTROL_THUMB, CONTROL_THUMB_BORDER, CONTROL_TRACK, SECONDARY, YELLOW};
use crate::widgets::{accessible_slider_input, accessible_slider_node};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2};

// Only the active drag lives in GUI memory. After release, Core reconciles
// pending and confirmed values; the widget never hides device state itself.
#[derive(Clone, Copy)]
pub(crate) struct FaderGesture {
    pub(crate) target: f32,
    last_sent: f32,
    last_pointer_y: f32,
    grab_offset_y: f32,
    fine: bool,
}

impl FaderGesture {
    pub(crate) fn begin(
        value: f32,
        pointer_y: f32,
        track: Rect,
        on_thumb: bool,
        fine: bool,
    ) -> Self {
        let grab_offset_y = if on_thumb {
            fader_thumb_y(track, value) - pointer_y
        } else {
            0.0
        };
        Self {
            target: if on_thumb {
                value
            } else {
                fader_value_at_y(track, pointer_y)
            },
            last_sent: value,
            last_pointer_y: pointer_y,
            grab_offset_y,
            fine,
        }
    }

    pub(crate) fn update(&mut self, pointer_y: f32, track: Rect, fine: bool) -> bool {
        if self.fine != fine {
            // Switching precision must not move the thumb on its own.
            self.grab_offset_y = fader_thumb_y(track, self.target) - pointer_y;
        }
        self.target = if fine {
            (self.target - (pointer_y - self.last_pointer_y) / track.height() * 0.1).clamp(0.0, 1.0)
        } else {
            fader_value_at_y(track, pointer_y + self.grab_offset_y)
        };
        self.last_pointer_y = pointer_y;
        self.fine = fine;
        if (self.target - self.last_sent).abs() > 1.0e-4 {
            self.last_sent = self.target;
            true
        } else {
            false
        }
    }
}

// Reserve a small bottom detent for mute, separate from finite fader levels.
const FADER_MUTE_GAP: f32 = 0.07;
pub(crate) const FADER_SCALE: [(f32, &str); 5] = [
    (1.0, "10"),
    (0.75, "0"),
    (0.5, "-10"),
    (0.25, "-30"),
    (0.0, "-∞"),
];

fn fader_visual_position(value: f32) -> f32 {
    if value <= 0.0 {
        0.0
    } else {
        FADER_MUTE_GAP + value.clamp(0.0, 1.0) * (1.0 - FADER_MUTE_GAP)
    }
}

pub(crate) fn fader_thumb_y(track: Rect, value: f32) -> f32 {
    egui::lerp(track.bottom()..=track.top(), fader_visual_position(value))
}

pub(crate) fn fader_value_at_y(track: Rect, y: f32) -> f32 {
    let position = ((track.bottom() - y) / track.height()).clamp(0.0, 1.0);
    if position <= FADER_MUTE_GAP {
        0.0
    } else {
        ((position - FADER_MUTE_GAP) / (1.0 - FADER_MUTE_GAP)).clamp(0.0, 1.0)
    }
}

pub(crate) fn fader_sized(
    ui: &mut egui::Ui,
    value: &mut f32,
    width: f32,
    height: f32,
    default: f32,
    _confirmed: Option<f32>,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, height).round_ui(), Sense::click_and_drag());
    let track = Rect::from_center_size(
        Pos2::new(rect.left() + 26.0, rect.center().y),
        Vec2::new(5.0, (rect.height() - 24.0).max(1.0)),
    );
    let pointer_held =
        response.is_pointer_button_down_on() && ui.input(|input| input.pointer.primary_down());
    if response.clicked() || pointer_held {
        response.request_focus();
    }
    let drag_id = response.id.with("fader-drag");
    if !ui.is_enabled() {
        ui.ctx()
            .data_mut(|data| data.remove::<FaderGesture>(drag_id));
    } else if pointer_held {
        if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
            let started =
                ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary));
            let previous = if started {
                None
            } else {
                ui.ctx().data(|data| data.get_temp::<FaderGesture>(drag_id))
            };
            let fine = ui.input(|input| input.modifiers.shift);
            let thumb = Rect::from_center_size(
                Pos2::new(track.center().x, fader_thumb_y(track, *value)),
                Vec2::new(29.0, 15.0),
            );
            let mut gesture = previous.unwrap_or_else(|| {
                FaderGesture::begin(
                    *value,
                    pointer.y,
                    track,
                    thumb.expand(3.0).contains(pointer),
                    fine,
                )
            });
            if gesture.update(pointer.y, track, fine) {
                response.mark_changed();
            }
            *value = gesture.target;
            ui.ctx().data_mut(|data| data.insert_temp(drag_id, gesture));
        }
    } else {
        ui.ctx()
            .data_mut(|data| data.remove::<FaderGesture>(drag_id));
    }
    if response.double_clicked() && (*value - default).abs() > f32::EPSILON {
        *value = default;
        response.mark_changed();
        ui.ctx()
            .data_mut(|data| data.remove::<FaderGesture>(drag_id));
    }
    // Wheel input belongs to the scroll area, never to a merely hovered fader.
    if ui.is_enabled() && response.has_focus() {
        let delta = ui.input(|input| {
            if input.key_pressed(egui::Key::ArrowUp) {
                0.01
            } else if input.key_pressed(egui::Key::ArrowDown) {
                -0.01
            } else {
                0.0
            }
        });
        let sensitivity = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
        let next = (*value + delta * sensitivity).clamp(0.0, 1.0);
        if (*value - next).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
            if !pointer_held {
                ui.ctx()
                    .data_mut(|data| data.remove::<FaderGesture>(drag_id));
            }
        }
    }
    if accessible_slider_input(ui, &response, value, 0.0, 1.0, 0.01) {
        response.mark_changed();
        ui.ctx()
            .data_mut(|data| data.remove::<FaderGesture>(drag_id));
    }
    let cursor = if pointer_held {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };
    response = response.on_hover_cursor(cursor);
    let painter = ui.painter();
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            4.0,
            Stroke::new(1.0, BLUE),
            StrokeKind::Inside,
        );
    }
    painter.rect_filled(track, 2.0, CONTROL_TRACK);
    let fill_top = fader_thumb_y(track, *value);
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(track.left(), fill_top), track.right_bottom()),
        2.0,
        YELLOW,
    );
    for (level, label) in FADER_SCALE {
        let y = fader_thumb_y(track, level);
        painter.line_segment(
            [
                Pos2::new(track.right() + 3.0, y),
                Pos2::new(track.right() + 7.0, y),
            ],
            Stroke::new(1.0, SECONDARY),
        );
        painter.text(
            Pos2::new(track.right() + 12.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(10.0),
            SECONDARY,
        );
    }
    let thumb_y = fader_thumb_y(track, *value);
    let thumb = Rect::from_center_size(Pos2::new(track.center().x, thumb_y), Vec2::new(29.0, 15.0));
    painter.rect_filled(
        thumb,
        4.0,
        if pointer_held {
            YELLOW
        } else if response.hovered() {
            Color32::WHITE
        } else {
            CONTROL_THUMB
        },
    );
    painter.rect_stroke(
        thumb,
        4.0,
        Stroke::new(1.0, CONTROL_THUMB_BORDER),
        StrokeKind::Inside,
    );
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), "Level"));
    accessible_slider_node(ui, &response, *value, 0.0, 1.0, 0.01);
    response
}

pub(crate) fn fader_level_text(value: f32) -> String {
    if value <= 0.0 {
        "−∞".into()
    } else {
        format!("{:+.1}", normalized_to_display_db(value))
    }
}

fn normalized_to_display_db(value: f32) -> f32 {
    // Presentation floor observed on the physical fader; wire conversion stays in flow8-protocol.
    flow8_protocol::normalized_to_fader_db(value.clamp(0.0, 1.0))
        .unwrap_or(-89.5)
        .max(-89.5)
}
