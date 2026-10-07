use crate::theme::TEXT;
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{self, FontId, Vec2};

use crate::i18n::Language;

pub(super) const UI_SCALE_MIN: f32 = 0.85;
pub(super) const UI_SCALE_MAX: f32 = 1.40;

pub(super) const MIXER_COMPACT_WIDTH: f32 = 1180.0;
pub(super) const SETTINGS_CONTENT_MAX_WIDTH: f32 = 1180.0;
pub(super) const TOPOLOGY_CONTENT_MAX_WIDTH: f32 = 1180.0;
pub(super) const TAB_BUTTON_MIN_WIDTH: f32 = 72.0;
pub(super) const DESTINATION_BUTTON_MIN_WIDTH: f32 = 68.0;
pub(super) const STATE_BUTTON_MIN_WIDTH: f32 = 52.0;
pub(super) const INSPECTOR_HEADING_SIZE: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UiMetrics {
    pub(super) ui_scale: f32,
    pub(super) body_font: f32,
    pub(super) button_font: f32,
    pub(super) secondary_font: f32,
    pub(super) small_font: f32,
    pub(super) heading_font: f32,
    pub(super) control_height: f32,
    pub(super) spacing: f32,
}

impl UiMetrics {
    pub(super) fn calculate(available: Vec2, _pixels_per_point: f32, user_scale: f32) -> Self {
        let user_scale = user_scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX);
        let viewport_scale = (available.x / 1440.0).clamp(0.90, 1.10);
        let scale = (user_scale * viewport_scale).clamp(UI_SCALE_MIN, 1.45);
        Self {
            ui_scale: scale,
            body_font: 14.0 * scale,
            button_font: 13.0 * scale,
            secondary_font: 11.0 * scale,
            small_font: 10.0 * scale,
            heading_font: 22.0 * scale,
            control_height: (30.0 * scale).round_ui(),
            // add_space advances the cursor without rounding it. Keep scaled
            // spacing on egui's logical grid so trailing gaps stay aligned too.
            spacing: (8.0 * scale).round_ui(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LayoutMetrics {
    pub(super) fader_height: f32,
    pub(super) channel_width: f32,
    pub(super) inspector_width: f32,
    pub(super) compact: bool,
    pub(super) mixer_scrolls: bool,
}

impl LayoutMetrics {
    pub(super) fn calculate(available: Vec2, ui: UiMetrics, visible_channels: usize) -> Self {
        let available_width = available.x.max(0.0);
        let compact = available_width < MIXER_COMPACT_WIDTH * ui.ui_scale;
        let wide = available_width >= 1760.0 * ui.ui_scale;
        let inspector_width = (available_width * if wide { 0.24 } else { 0.27 })
            .clamp(290.0 * ui.ui_scale, 430.0 * ui.ui_scale)
            .round_ui();
        let channel_area = if compact {
            available_width
        } else {
            (available_width - inspector_width - ui.spacing).max(0.0)
        };
        let channel_min_width = 100.0 * ui.ui_scale;
        let channel_max_width = 146.0 * ui.ui_scale;
        let channels = visible_channels.max(1) as f32;
        let gaps = ui.spacing * (channels - 1.0);
        let natural_channel_width = ((channel_area - gaps) / channels).max(0.0);
        let fader_height = (available.y - 290.0 * ui.ui_scale)
            .clamp(160.0 * ui.ui_scale, 320.0 * ui.ui_scale)
            .round_ui();
        Self {
            fader_height,
            channel_width: natural_channel_width
                .clamp(channel_min_width, channel_max_width)
                .round_ui(),
            inspector_width,
            compact,
            mixer_scrolls: natural_channel_width < channel_min_width,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SnapshotGridLayout {
    pub(super) columns: usize,
    pub(super) card_inner_width: f32,
    pub(super) button_width: f32,
    pub(super) gap: f32,
}

impl SnapshotGridLayout {
    pub(super) fn new(available_width: f32, scale: f32, gap: f32, slots: usize) -> Self {
        // Count a new column only when another complete card, including four
        // usable buttons and the card margins, fits in the visible width.
        let min_card_width = (4.0 * 90.0 * scale + 3.0 * gap + 28.0).max(410.0 * scale);
        let columns = (((available_width + gap) / (min_card_width + gap)).floor() as usize)
            .clamp(1, slots.max(1));
        let card_width =
            ((available_width - gap * (columns - 1) as f32) / columns as f32).min(560.0 * scale);
        // The card has 12-point side margins; leave extra rounding slack.
        let card_inner_width = (card_width - 28.0).max(0.0).floor();
        let button_width = ((card_inner_width - 3.0 * gap) / 4.0).max(0.0).floor();
        Self {
            columns,
            card_inner_width,
            button_width,
            gap,
        }
    }

    pub(super) fn group_width(self) -> f32 {
        // Frame: two 12-point inner margins and two 1-point strokes.
        self.columns as f32 * (self.card_inner_width + 26.0) + (self.columns - 1) as f32 * self.gap
    }
}

#[derive(Clone, Copy)]
pub(super) struct SettingsGridLayout {
    pub(super) two_columns: bool,
    pub(super) column_width: f32,
    pub(super) label_width: f32,
    pub(super) gap: f32,
    pub(super) scale: f32,
}

impl SettingsGridLayout {
    pub(super) fn calculate(
        ui: &egui::Ui,
        content_width: f32,
        metrics: UiMetrics,
        language: Language,
    ) -> Self {
        let gap = (12.0 * metrics.ui_scale).round_ui();
        let label_width = [
            language.tr("Bluetooth / USB to Headphones Only", "蓝牙 / USB 仅送耳机"),
            language.tr("Device-linked Output Selection", "设备联动输出选择"),
            language.tr("Show Output Delay Indicator", "显示输出延迟指示"),
            language.tr("Connection progress", "连接进度"),
        ]
        .into_iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap(
                    label.to_owned(),
                    FontId::proportional(metrics.body_font),
                    TEXT,
                )
                .size()
                .x
        })
        .fold(0.0, f32::max)
            + gap;
        // The breakpoint follows actual form needs: measured labels, a usable
        // value/control area, card margins, and the gutter between columns.
        let min_control_width = 220.0 * metrics.ui_scale;
        let min_column_width = label_width + min_control_width + 26.0 + gap;
        let two_columns = content_width >= 2.0 * min_column_width + gap;
        let column_width = if two_columns {
            ((content_width - gap) / 2.0).floor()
        } else {
            content_width
        };
        Self {
            two_columns,
            column_width,
            label_width,
            gap,
            scale: metrics.ui_scale,
        }
    }
}
