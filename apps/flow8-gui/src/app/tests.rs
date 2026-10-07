use crate::app::confirmation::{ConfirmationAction, valid_snapshot_name};
use crate::app::pages::snapshots::{
    DeviceSlotStatus, centered_snapshot_content, device_slot_status, snapshot_action_button,
    snapshot_card, snapshot_name_row, snapshot_status_badge,
};
use crate::app::{Flow8App, Page, device_unsynced_notice};
use crate::fonts::configure_fonts;
use crate::i18n::Language;
use crate::metrics::{LayoutMetrics, SnapshotGridLayout, UiMetrics};
use crate::theme::configure_style;
use crate::widgets::{
    FADER_SCALE, FaderGesture, fader_level_text, fader_sized, fader_thumb_y, fader_value_at_y,
};
use eframe::egui::{self, Align, Layout, Pos2, Rect, Vec2};
use flow8_core::{Flow8Store, SemanticCommand, SessionState};
use flow8_model::{EvidenceStatus, InputId};

use crate::logging::flow8_ble_verbose_logging;

#[test]
fn headless_fader_ignores_delayed_rx_during_drag_and_after_release() {
    fn render(
        context: &egui::Context,
        events: Vec<egui::Event>,
        store_value: f32,
        time: f64,
    ) -> (f32, bool, Rect) {
        let mut displayed = store_value;
        let mut changed = false;
        let mut rect = Rect::NOTHING;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut output = context.run_ui(raw, |ui| {
            let response = fader_sized(ui, &mut displayed, 50.0, 200.0, 0.75, Some(store_value));
            changed = response.changed();
            rect = response.rect;
        });
        output.textures_delta.clear();
        (displayed, changed, rect)
    }

    let context = egui::Context::default();
    let (_, _, rect) = render(&context, vec![], 0.5, 0.0);
    let thumb = Pos2::new(rect.center().x, rect.center().y);
    let press = egui::Event::PointerButton {
        pos: thumb,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::default(),
    };
    let (pressed, _, _) = render(
        &context,
        vec![egui::Event::PointerMoved(thumb), press],
        0.5,
        0.1,
    );
    assert!((pressed - 0.5).abs() < 1.0e-6);

    let moved = Pos2::new(thumb.x, thumb.y - 40.0);
    let (dragged, changed, _) = render(&context, vec![egui::Event::PointerMoved(moved)], 0.5, 0.2);
    assert!(changed);
    assert!(dragged > 0.6);

    let (held, repeated, _) = render(&context, vec![], 0.2, 0.3);
    assert!(
        !repeated,
        "stale RX must not enqueue another identical fader TX"
    );
    assert!((held - dragged).abs() < 1.0e-6);

    let release = egui::Event::PointerButton {
        pos: moved,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    };
    let (released, _, _) = render(&context, vec![release], 0.2, 0.4);
    assert!((released - dragged).abs() < 1.0e-6);
    let (confirmed, _, _) = render(&context, vec![], dragged, 0.5);
    assert!((confirmed - dragged).abs() < 1.0e-6);
}

#[test]
fn fader_drag_tracks_pointer_without_reusing_stale_device_values() {
    let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(5.0, 200.0));
    let mut gesture = FaderGesture::begin(0.5, 100.0, track, true, false);
    assert!(!gesture.update(100.0, track, false));
    assert!(gesture.update(60.0, track, false));
    let dragged_target = fader_value_at_y(track, fader_thumb_y(track, 0.5) - 40.0);
    assert!((gesture.target - dragged_target).abs() < 1.0e-6);
    // A delayed RX might replace the Store value with 0.2. The active
    // gesture still displays the dragged target and does not enqueue another TX.
    let stale_confirmed = 0.2;
    assert!((gesture.target - stale_confirmed).abs() > 0.4);
    assert!(!gesture.update(60.0, track, false));
    assert!((gesture.target - dragged_target).abs() < 1.0e-6);
}

#[test]
fn fader_track_click_and_fine_drag_preserve_pointer_position() {
    let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(5.0, 200.0));
    let mut click = FaderGesture::begin(0.25, 20.0, track, false, false);
    assert!(click.update(20.0, track, false));
    assert!((click.target - fader_value_at_y(track, 20.0)).abs() < 1.0e-6);

    let mut fine = FaderGesture::begin(0.5, 100.0, track, true, false);
    assert!(!fine.update(100.0, track, false));
    assert!(fine.update(80.0, track, true));
    assert!((fine.target - 0.51).abs() < 1.0e-6);
    assert!(!fine.update(80.0, track, false));
    assert!((fine.target - 0.51).abs() < 1.0e-6);
}

#[test]
fn fader_scale_matches_physical_labels_and_thumb_positions() {
    assert_eq!(
        FADER_SCALE.map(|(_, label)| label),
        ["10", "0", "-10", "-30", "-∞"]
    );
    let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(5.0, 200.0));
    for (level, _) in FADER_SCALE {
        let y = fader_thumb_y(track, level);
        assert!((fader_value_at_y(track, y) - level).abs() < 1.0e-6);
    }
    assert!(fader_thumb_y(track, 0.001) < fader_thumb_y(track, 0.0));
    assert_eq!(fader_value_at_y(track, track.bottom() - 5.0), 0.0);
    assert_eq!(fader_level_text(0.0), "−∞");
    assert_eq!(fader_level_text(0.0001), "-89.5");
    assert_eq!(fader_level_text(1.0), "+10.0");
}

#[test]
fn fader_gesture_contains_only_active_drag_intent() {
    let track = Rect::from_min_max(Pos2::ZERO, Pos2::new(5.0, 200.0));
    let mut gesture = FaderGesture::begin(0.7, fader_thumb_y(track, 0.7), track, true, false);
    assert_eq!(gesture.target, 0.7);
    assert!(!gesture.update(fader_thumb_y(track, 0.7), track, false));
}

#[test]
fn snapshot_slots_use_only_confirmed_device_names_for_occupancy() {
    let mut slot = Flow8Store::disconnected().state.snapshots.device_slots[0].clone();
    assert_eq!(device_slot_status(&slot), DeviceSlotStatus::Unknown);
    slot.name.set_pending("Proposed name".into());
    assert_eq!(device_slot_status(&slot), DeviceSlotStatus::Unknown);
    slot.name
        .observe(String::new(), EvidenceStatus::VerifiedFromDevice);
    assert_eq!(device_slot_status(&slot), DeviceSlotStatus::Free);
    slot.name
        .observe("Vocal".into(), EvidenceStatus::VerifiedFromDevice);
    assert_eq!(
        device_slot_status(&slot),
        DeviceSlotStatus::Occupied("Vocal".into())
    );
}

#[test]
fn snapshot_grid_reflows_and_four_actions_keep_equal_nonoverlapping_widths() {
    assert_eq!(SnapshotGridLayout::new(831.0, 1.0, 8.0, 10).columns, 1);
    assert_eq!(SnapshotGridLayout::new(832.0, 1.0, 8.0, 10).columns, 2);
    let mut previous_columns = 0;
    for window_width in [1040.0, 1280.0, 1440.0, 1600.0, 1920.0, 2560.0] {
        let metrics = UiMetrics::calculate(Vec2::new(window_width, 920.0), 1.0, 1.0);
        let layout =
            SnapshotGridLayout::new(window_width - 36.0, metrics.ui_scale, metrics.spacing, 10);
        assert!(layout.columns >= previous_columns);
        previous_columns = layout.columns;
        assert!(4.0 * layout.button_width + 3.0 * layout.gap <= layout.card_inner_width + 0.1);
        for language in [Language::English, Language::Chinese] {
            let context = egui::Context::default();
            configure_style(&context, metrics);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(window_width, 920.0),
                )),
                ..Default::default()
            };
            let mut rects = Vec::new();
            let mut output = context.run_ui(raw, |ui| {
                ui.set_width(layout.card_inner_width);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = layout.gap;
                    for (english, chinese) in [
                        ("Load", "载入"),
                        ("Save", "保存"),
                        ("Delete", "删除"),
                        ("Rename", "重命名"),
                    ] {
                        rects.push(
                            snapshot_action_button(
                                ui,
                                language.tr(english, chinese),
                                true,
                                Vec2::new(layout.button_width, metrics.control_height),
                            )
                            .rect,
                        );
                    }
                });
            });
            output.textures_delta.clear();
            assert_eq!(rects.len(), 4);
            for pair in rects.windows(2) {
                assert!(pair[0].right() <= pair[1].left());
                assert!((pair[0].width() - pair[1].width()).abs() < 0.1);
            }
            assert!(rects[3].right() - rects[0].left() <= layout.card_inner_width + 0.1);
        }
    }
    assert!(previous_columns > 1);
}

#[test]
fn snapshot_card_group_has_balanced_window_margins() {
    for width in [1040.0, 1280.0, 1440.0, 1600.0, 1920.0, 2560.0] {
        let metrics = UiMetrics::calculate(Vec2::new(width, 920.0), 1.0, 1.0);
        let layout = SnapshotGridLayout::new(width - 36.0, metrics.ui_scale, metrics.spacing, 10);
        let context = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 920.0))),
            ..Default::default()
        };
        let mut margins = (0.0, 0.0);
        let mut output = context.run_ui(raw, |ui| {
            let viewport = ui.max_rect();
            let group = centered_snapshot_content(ui, layout.group_width(), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = layout.gap;
                    for _ in 0..layout.columns {
                        let _ = snapshot_card(ui, layout.card_inner_width, |ui| {
                            ui.label("01");
                        });
                    }
                });
            });
            margins = (
                group.response.rect.left() - viewport.left(),
                viewport.right() - group.response.rect.right(),
            );
        });
        output.textures_delta.clear();
        assert!(
            (margins.0 - margins.1).abs() <= 3.0,
            "width={width}, left={}, right={}",
            margins.0,
            margins.1
        );
        assert!(margins.0 >= 16.0 && margins.1 >= 16.0);
    }
}

#[test]
fn snapshot_cards_keep_status_in_header_and_reserve_name_height() {
    for language in [Language::English, Language::Chinese] {
        let context = egui::Context::default();
        let metrics = UiMetrics::calculate(Vec2::new(1040.0, 700.0), 1.0, 1.0);
        configure_style(&context, metrics);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1040.0, 700.0))),
            ..Default::default()
        };
        let mut cards = Vec::new();
        let mut output = context.run_ui(raw, |ui| {
            ui.horizontal(|ui| {
                for status in [
                    DeviceSlotStatus::Free,
                    DeviceSlotStatus::Occupied("Vocal".into()),
                ] {
                    let card = snapshot_card(ui, 410.0, |ui| {
                        let header = ui.horizontal(|ui| {
                            ui.label("01");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                snapshot_status_badge(ui, &status, language);
                            });
                        });
                        let name = match &status {
                            DeviceSlotStatus::Occupied(name) => Some(name.as_str()),
                            _ => None,
                        };
                        let name =
                            snapshot_name_row(ui, name, false, 410.0, metrics.body_font, language);
                        let actions = ui.horizontal(|ui| {
                            for (english, chinese) in [
                                ("Load", "载入"),
                                ("Save", "保存"),
                                ("Delete", "删除"),
                                ("Rename", "重命名"),
                            ] {
                                snapshot_action_button(
                                    ui,
                                    language.tr(english, chinese),
                                    true,
                                    Vec2::new(90.0, metrics.control_height),
                                );
                            }
                        });
                        (header.response.rect, name.rect, actions.response.rect)
                    });
                    cards.push((card.response.rect, card.inner));
                }
            });
        });
        output.textures_delta.clear();
        assert_eq!(cards.len(), 2);
        for (_, (header, name, actions)) in &cards {
            assert!(header.bottom() <= name.top());
            assert!(name.bottom() <= actions.top());
        }
        assert!((cards[0].0.height() - cards[1].0.height()).abs() < 0.1);
        assert!(cards[0].0.right() <= cards[1].0.left());
    }
}

#[test]
fn long_snapshot_name_does_not_expand_responsive_grid() {
    fn render_width(language: Language, name: &str) -> f32 {
        let context = egui::Context::default();
        let mut app = Flow8App::from_context(&context);
        app.language = language;
        app.store.state.snapshots.device_slots[0]
            .name
            .observe(name.into(), EvidenceStatus::VerifiedFromDevice);
        let mut width = 0.0;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
            ..Default::default()
        };
        let mut output = context.run_ui(raw, |ui| {
            width = ui.scope(|ui| app.snapshots_page(ui)).response.rect.width();
        });
        output.textures_delta.clear();
        width
    }

    for language in [Language::English, Language::Chinese] {
        let short = render_width(language, "A");
        let long = render_width(language, &"VeryLongSnapshotName".repeat(20));
        assert!(
            (short - long).abs() < 1.0,
            "snapshot grid changed width for {language:?}: {short} vs {long}"
        );
    }
}

#[test]
fn device_snapshot_actions_require_confirmation_and_cancel_is_safe() {
    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);
    app.store.state.session = SessionState::Ready;
    app.page = Page::Snapshots;
    app.store.state.snapshots.device_slots[0]
        .name
        .observe("Vocal".into(), EvidenceStatus::VerifiedFromDevice);
    let observed_name = Some("Vocal".into());
    app.request_confirmation(ConfirmationAction::LoadDeviceSnapshot {
        slot: 0,
        observed_name: observed_name.clone(),
    });
    assert!(app.store.queue.is_empty());
    app.pending_confirmation = None; // Explicit Cancel.
    assert!(app.store.queue.is_empty());

    app.request_confirmation(ConfirmationAction::DeleteDeviceSnapshot {
        slot: 0,
        observed_name: observed_name.clone(),
    });
    app.page = Page::Settings;
    app.clear_stale_confirmation();
    assert!(app.pending_confirmation.is_none());
    app.confirm_pending_action();
    assert!(app.store.queue.is_empty());

    app.page = Page::Snapshots;
    app.request_confirmation(ConfirmationAction::SaveDeviceSnapshot {
        slot: 0,
        name: "Vocal".into(),
        observed_name,
    });
    assert!(app.store.queue.is_empty());
    app.confirm_pending_action();
    assert!(matches!(
        app.store.queue.pop(),
        Some(SemanticCommand::SaveSnapshot { slot: 0, .. })
    ));
}

#[test]
fn snapshot_names_obey_protocol_byte_limit_and_submit_edited_name() {
    assert!(!valid_snapshot_name("  "));
    assert!(!valid_snapshot_name("line\nbreak"));
    assert!(valid_snapshot_name("中文中文中文")); // 18 UTF-8 bytes.
    assert!(!valid_snapshot_name("中文中文中文中")); // 21 UTF-8 bytes.
    assert!(valid_snapshot_name("  Vocal  "));

    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);
    app.store.state.session = SessionState::Ready;
    app.page = Page::Snapshots;
    app.store.state.snapshots.device_slots[0]
        .name
        .observe("Old".into(), EvidenceStatus::VerifiedFromDevice);

    app.request_confirmation(ConfirmationAction::SaveDeviceSnapshot {
        slot: 0,
        name: "  New name  ".into(),
        observed_name: Some("Old".into()),
    });
    assert!(app.store.queue.is_empty());
    app.confirm_pending_action();
    assert!(matches!(
        app.store.queue.pop(),
        Some(SemanticCommand::SaveSnapshot { slot: 0, name }) if name == "New name"
    ));

    app.request_confirmation(ConfirmationAction::RenameDeviceSnapshot {
        slot: 0,
        name: "Vocal".into(),
        observed_name: Some("Old".into()),
    });
    assert!(app.store.queue.is_empty());
    app.confirm_pending_action();
    assert!(matches!(
        app.store.queue.pop(),
        Some(SemanticCommand::RenameSnapshot { slot: 0, name }) if name == "Vocal"
    ));

    app.request_confirmation(ConfirmationAction::SaveDeviceSnapshot {
        slot: 0,
        name: "中文中文中文中".into(),
        observed_name: Some("Old".into()),
    });
    app.confirm_pending_action();
    assert!(app.store.queue.is_empty());
}

#[test]
fn phantom_confirmation_expires_when_input_or_session_changes() {
    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);
    app.store.state.session = SessionState::Ready;
    app.store.state.channels[0]
        .phantom_48v
        .observe(false, EvidenceStatus::VerifiedFromDevice);
    app.request_confirmation(ConfirmationAction::EnablePhantom(InputId::Input1));
    assert!(app.store.queue.is_empty());
    app.store.state.selected_input = Some(InputId::Input2);
    app.clear_stale_confirmation();
    assert!(app.pending_confirmation.is_none());
    app.confirm_pending_action();
    assert!(app.store.queue.is_empty());

    app.store.state.selected_input = Some(InputId::Input1);
    app.request_confirmation(ConfirmationAction::EnablePhantom(InputId::Input1));
    app.store.state.session = SessionState::Disconnected;
    app.clear_stale_confirmation();
    assert!(app.pending_confirmation.is_none());
    assert!(app.store.queue.is_empty());
}

#[test]
fn snapshot_name_refresh_uses_existing_read_only_protocol_request() {
    let mut queue = flow8_core::SemanticCommandQueue::default();
    queue.push(SemanticCommand::RequestSnapshotNames).unwrap();
    assert!(matches!(
        queue.front_protocol_tracked(),
        Some((_, flow8_protocol::TxCommand::GetSnapshotNames))
    ));
}

#[test]
fn production_gui_starts_disconnected_without_simulator_source() {
    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);
    assert!(!app.store.is_simulator());
    assert_eq!(app.store.state.session, SessionState::Disconnected);
    assert!(app.store.queue.is_empty());
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
        ..Default::default()
    };
    let mut output = context.run_ui(raw, |ui| {
        device_unsynced_notice(ui, app.store.state.session, app.language);
        ui.add_enabled_ui(false, |ui| app.mixer(ui));
    });
    assert!(!output.shapes.is_empty());
    output.textures_delta.clear();
    app.dispatch(SemanticCommand::SetMute {
        input: InputId::Input1,
        enabled: true,
    });
    assert!(app.store.queue.is_empty());
    assert_eq!(app.store.state.channels[0].muted.pending, None);
}

#[test]
fn idle_mixer_never_echoes_received_pan_to_tx_queue() {
    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);
    app.store = Flow8Store::disconnected();
    app.store.state.session = SessionState::Ready;
    for (endpoint, value) in [(0, 1.0 / 127.0), (6, 0.0), (15, 0.0)] {
        app.store
            .apply_rx(
                flow8_protocol::RxCommand::Pan { endpoint, value },
                EvidenceStatus::VerifiedFromDevice,
            )
            .unwrap();
    }
    app.store
        .apply_rx(
            flow8_protocol::RxCommand::Compressor {
                endpoint: 0,
                amount: 101.0 / 255.0,
            },
            EvidenceStatus::VerifiedFromDevice,
        )
        .unwrap();
    for _ in 0..3 {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
            ..Default::default()
        };
        let mut output = context.run_ui(raw, |ui| app.mixer(ui));
        output.textures_delta.clear();
        let mut commands = Vec::new();
        while let Some(command) = app.store.queue.pop() {
            commands.push(command);
        }
        assert!(commands.is_empty(), "idle GUI sent commands: {commands:?}");
    }
}

#[test]
fn all_eleven_pages_render_in_english_and_simplified_chinese() {
    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);

    for size in [
        Vec2::new(1040.0, 700.0),
        Vec2::new(1280.0, 720.0),
        Vec2::new(1440.0, 920.0),
        Vec2::new(1920.0, 1080.0),
        Vec2::new(2560.0, 1080.0),
    ] {
        app.metrics = UiMetrics::calculate(size, 1.0, app.preferences.ui_scale);
        configure_style(&context, app.metrics);
        for language in [Language::English, Language::Chinese] {
            app.language = language;
            for page in [
                Page::Mixer,
                Page::Stage,
                Page::Fx1,
                Page::Fx2,
                Page::Monitor1,
                Page::Monitor2,
                Page::Main,
                Page::MainOut,
                Page::Routing,
                Page::Snapshots,
                Page::Settings,
            ] {
                app.page = page;
                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                };
                let mut output = context.run_ui(raw, |ui| {
                    app.connection_bar(ui);
                    app.layer_bar(ui);
                    match page {
                        Page::Mixer
                        | Page::Fx1
                        | Page::Fx2
                        | Page::Monitor1
                        | Page::Monitor2
                        | Page::Main => app.mixer(ui),
                        Page::Stage => app.stage_page(ui),
                        Page::MainOut => app.main_out_page(ui),
                        Page::Routing => app.routing_page(ui),
                        Page::Snapshots => app.snapshots_page(ui),
                        Page::Settings => app.settings_page(ui),
                    }
                });
                assert!(
                    !output.shapes.is_empty(),
                    "page {page:?} rendered no shapes for {language:?} at {size:?}"
                );
                output.textures_delta.clear();
                assert!(
                    app.store.queue.is_empty(),
                    "idle page {page:?} generated a device command for {language:?} at {size:?}"
                );
            }
        }
    }
}

#[test]
fn responsive_metrics_scale_fonts_controls_and_mixer_widths() {
    let compact = UiMetrics::calculate(Vec2::new(1040.0, 700.0), 1.0, 1.0);
    let enlarged = UiMetrics::calculate(Vec2::new(1040.0, 700.0), 1.0, 1.40);
    assert!(enlarged.body_font > compact.body_font);
    assert!(enlarged.control_height > compact.control_height);
    let compact_layout = LayoutMetrics::calculate(Vec2::new(650.0, 700.0), compact, 7);
    let wide_layout = LayoutMetrics::calculate(Vec2::new(1600.0, 900.0), enlarged, 7);
    assert!(compact_layout.mixer_scrolls);
    assert!(!wide_layout.mixer_scrolls);
    assert!(wide_layout.channel_width <= 146.0 * enlarged.ui_scale);
}

#[test]
fn interactive_visual_states_are_distinct() {
    let context = egui::Context::default();
    let metrics = UiMetrics::calculate(Vec2::new(1440.0, 920.0), 1.0, 1.0);
    configure_style(&context, metrics);
    let style = context.style_of(egui::Theme::Dark);
    assert_ne!(
        style.visuals.widgets.inactive.bg_fill,
        style.visuals.widgets.hovered.bg_fill
    );
    assert_ne!(
        style.visuals.widgets.hovered.bg_fill,
        style.visuals.widgets.active.bg_fill
    );
    assert_ne!(
        style.visuals.widgets.inactive.fg_stroke.color,
        style.visuals.widgets.noninteractive.fg_stroke.color
    );
    assert_eq!(
        style.visuals.widgets.inactive.bg_stroke.width,
        style.visuals.widgets.hovered.bg_stroke.width
    );
    assert_eq!(
        style.visuals.widgets.hovered.bg_stroke.width,
        style.visuals.widgets.active.bg_stroke.width
    );
    assert!((style.animation_time - 0.14).abs() < f32::EPSILON);
}

#[test]
fn language_switch_changes_all_representative_visible_strings() {
    let keys = [
        ("Connect", "连接"),
        ("MIXER", "混音器"),
        ("INPUT DETAIL", "输入详情"),
        ("MAIN OUT", "主输出"),
        ("FX ENGINE", "效果器"),
        ("Routing", "路由"),
        ("Stage View", "舞台视图"),
        ("Device Snapshots", "设备快照"),
        ("Preferences", "偏好设置"),
        ("Connection failed", "连接失败"),
        (
            "Wait for a complete FLOW 8 state sync before editing.",
            "请等待 FLOW 8 完成状态同步后再进行调整。",
        ),
    ];
    for (english, chinese) in keys {
        assert!(!english.is_empty());
        assert!(!chinese.is_empty());
        assert_ne!(
            Language::English.tr(english, chinese),
            Language::Chinese.tr(english, chinese)
        );
    }
}

#[test]
fn system_cjk_fallback_initialization_never_panics() {
    let context = egui::Context::default();
    configure_fonts(&context);
}

#[test]
fn settings_render_does_not_enqueue_unknown_or_disabled_writes() {
    let context = egui::Context::default();
    let mut app = Flow8App::from_context(&context);
    app.language = Language::Chinese;
    assert!(app.store.queue.is_empty());
    let mut output = context.run_ui(egui::RawInput::default(), |ui| app.settings_page(ui));
    assert!(!output.shapes.is_empty());
    assert!(app.store.queue.is_empty());
    output.textures_delta.clear();
}

#[test]
fn verbose_flow8_ble_filter_enables_production_log_capture() {
    assert!(flow8_ble_verbose_logging("flow8_ble=debug,flow8_gui=info"));
    assert!(flow8_ble_verbose_logging("flow8_ble=trace"));
    assert!(!flow8_ble_verbose_logging("flow8_ble=info"));
    assert!(!flow8_ble_verbose_logging("flow8_gui=debug"));
}
