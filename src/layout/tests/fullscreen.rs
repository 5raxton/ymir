use insta::assert_snapshot;

use super::*;

#[test]
fn fullscreen() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::FullscreenWindow(1),
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_window_in_column() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::ConsumeOrExpelWindowLeft { id: None },
        Op::SetFullscreenWindow {
            window: 2,
            is_fullscreen: false,
        },
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_view_offset_not_reset_on_removal() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::FullscreenWindow(0),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::ConsumeOrExpelWindowRight { id: None },
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_view_offset_not_reset_on_consume() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::FullscreenWindow(0),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::ConsumeWindowIntoColumn,
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_view_offset_not_reset_on_quick_double_toggle() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::FullscreenWindow(0),
        Op::FullscreenWindow(0),
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_view_offset_set_on_fullscreening_inactive_tile_in_column() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::ConsumeOrExpelWindowLeft { id: None },
        Op::FullscreenWindow(0),
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_view_offset_not_reset_on_gesture() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::FullscreenWindow(1),
        Op::ViewOffsetGestureBegin {
            output_idx: 1,
            workspace_idx: None,
            is_touchpad: true,
        },
        Op::ViewOffsetGestureEnd {
            is_touchpad: Some(true),
        },
    ];

    check_ops(ops);
}

#[test]
fn one_window_in_column_becomes_weight_1_after_fullscreen() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::ConsumeOrExpelWindowLeft { id: None },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::ConsumeOrExpelWindowLeft { id: None },
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::Communicate(2),
        Op::FocusWindowUp,
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::Communicate(1),
        Op::CloseWindow(0),
        Op::FullscreenWindow(1),
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_with_large_border() {
    let ops = [
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::FullscreenWindow(0),
        Op::Communicate(0),
        Op::FullscreenWindow(0),
    ];

    let options = Options {
        layout: ymir_config::Layout {
            border: ymir_config::Border {
                off: false,
                width: 10000.,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    check_ops_with_options(options, ops);
}

#[test]
fn fullscreen_to_windowed_fullscreen() {
    let ops = [
        Op::AddOutput(0),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::FullscreenWindow(0),
        Op::Communicate(0), // Make sure it goes into fullscreen.
        Op::ToggleWindowedFullscreen(0),
    ];

    check_ops(ops);
}

#[test]
fn windowed_fullscreen_to_fullscreen() {
    let ops = [
        Op::AddOutput(0),
        Op::AddWindow {
            params: TestWindowParams::new(0),
        },
        Op::FullscreenWindow(0),
        Op::Communicate(0),              // Commit fullscreen state.
        Op::ToggleWindowedFullscreen(0), // Switch is_fullscreen() to false.
        Op::FullscreenWindow(0),         // Switch is_fullscreen() back to true.
    ];

    check_ops(ops);
}

#[test]
fn move_pending_unfullscreen_window_out_of_active_column() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::FullscreenWindow(1),
        Op::Communicate(1),
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::ConsumeWindowIntoColumn,
        // Window 1 is now pending unfullscreen.
        // Moving it out should reset view_offset_before_fullscreen.
        Op::MoveWindowToWorkspaceDown(true),
    ];

    check_ops(ops);
}

#[test]
fn move_unfocused_pending_unfullscreen_window_out_of_active_column() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::FullscreenWindow(1),
        Op::Communicate(1),
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::ConsumeWindowIntoColumn,
        // Window 1 is now pending unfullscreen.
        // Moving it out should reset view_offset_before_fullscreen.
        Op::FocusWindowDown,
        Op::MoveWindowToWorkspace {
            window_id: Some(1),
            workspace_idx: 1,
        },
    ];

    check_ops(ops);
}

#[test]
fn interactive_resize_on_pending_unfullscreen_column() {
    let ops = [
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FullscreenWindow(2),
        Op::Communicate(2),
        Op::SetFullscreenWindow {
            window: 2,
            is_fullscreen: false,
        },
        Op::InteractiveResizeBegin {
            window: 2,
            edges: ResizeEdge::RIGHT,
        },
        Op::Communicate(2),
    ];

    check_ops(ops);
}

#[test]
fn interactive_move_unfullscreen_to_floating_stops_dnd_scroll() {
    let ops = [
        Op::AddOutput(3),
        Op::AddWindow {
            params: TestWindowParams {
                is_floating: true,
                ..TestWindowParams::new(4)
            },
        },
        // This moves the window to tiling.
        Op::SetFullscreenWindow {
            window: 4,
            is_fullscreen: true,
        },
        // This starts a DnD scroll since we're dragging a tiled window.
        Op::InteractiveMoveBegin {
            window: 4,
            output_idx: 3,
            px: 0.0,
            py: 0.0,
        },
        // This will cause the window to unfullscreen to floating, and should stop the DnD scroll
        // since we're no longer dragging a tiled window, but rather a floating one.
        Op::InteractiveMoveUpdate {
            window: 4,
            dx: 0.0,
            dy: 15035.31210741684,
            output_idx: 3,
            px: 0.0,
            py: 0.0,
        },
        Op::InteractiveMoveEnd { window: 4 },
    ];

    check_ops(ops);
}

#[test]
fn interactive_move_restore_to_floating_animates_view_offset() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        // Toggle window 1 to floating.
        Op::FocusWindow(1),
        Op::ToggleWindowFloating { id: None },
        // Fullscreen window 1 - it moves to scrolling with restore_to_floating = true.
        Op::FullscreenWindow(1),
        Op::Communicate(1),
        Op::CompleteAnimations,
    ];

    let mut layout = check_ops(ops);

    // Verify window 1 is in scrolling and has restore_to_floating = true.
    let scrolling = layout.active_workspace().unwrap().scrolling();
    let tile1 = scrolling.tiles().find(|t| *t.window().id() == 1).unwrap();
    assert!(
        tile1.restore_to_floating,
        "window 1 should have restore_to_floating = true"
    );

    let ops = [
        // Start interactive move on window 1.
        Op::InteractiveMoveBegin {
            window: 1,
            output_idx: 1,
            px: 100.,
            py: 100.,
        },
        // Update with a large delta to trigger the unmaximize.
        Op::InteractiveMoveUpdate {
            window: 1,
            dx: 1000.,
            dy: 1000.,
            output_idx: 1,
            px: 0.,
            py: 0.,
        },
    ];
    check_ops_on_layout(&mut layout, ops);

    // Window 1 should now be removed from the workspace (in the interactive move state).
    // Window 2 should be the only window in the scrolling space.
    let scrolling = layout.active_workspace().unwrap().scrolling();
    assert_eq!(scrolling.tiles().count(), 1);
    assert!(scrolling.tiles().next().unwrap().window().id() == &2);

    // The view offset should be animating to show window 2.
    assert!(scrolling.view_offset().is_animation_ongoing());
}

#[test]
fn unfullscreen_view_offset_not_reset_during_dnd_gesture() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
        Op::FullscreenWindow(3),
        Op::Communicate(3),
        Op::DndUpdate {
            output_idx: 1,
            px: 0.0,
            py: 0.0,
        },
        Op::FullscreenWindow(3),
        Op::Communicate(3),
    ];

    check_ops(ops);
}

#[test]
fn dwindle_fullscreen_only_fullscreens_the_focused_window() {
    let options = Options {
        layout: ymir_config::Layout {
            default_column_display: ymir_ipc::ColumnDisplay::Dwindle,
            ..Default::default()
        },
        ..Default::default()
    };
    let layout = check_ops_with_options(
        options,
        [
            Op::AddOutput(0),
            Op::AddWindow {
                params: TestWindowParams::new(0),
            },
            Op::AddWindow {
                params: TestWindowParams::new(1),
            },
            Op::AddWindow {
                params: TestWindowParams::new(2),
            },
            Op::FullscreenWindow(1),
            Op::Communicate(1),
            Op::CompleteAnimations,
        ],
    );

    let win = |id: usize| {
        layout
            .windows()
            .find(|(_mon, w)| w.id() == &id)
            .map(|(_mon, w)| w)
            .unwrap_or_else(|| panic!("window {id} not found"))
    };

    // Only the focused window is fullscreen.
    assert!(win(1).pending_sizing_mode().is_fullscreen());
    assert!(!win(0).pending_sizing_mode().is_fullscreen());
    assert!(!win(2).pending_sizing_mode().is_fullscreen());

    // The fullscreened window covers the whole monitor, the others keep their tiled size.
    assert_eq!(win(1).requested_size(), Some(Size::from((1280, 720))));

    // The dwindle siblings are still tiled half-width.
    for id in [0, 2] {
        let size = win(id).requested_size().unwrap();
        assert!(
            size.w < 1280,
            "sibling window {id} should not span the monitor"
        );
    }
}

#[test]
fn dwindle_fullscreen_remerges_into_dwindle_on_unfullscreen() {
    let options = Options {
        layout: ymir_config::Layout {
            default_column_display: ymir_ipc::ColumnDisplay::Dwindle,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut layout = check_ops_with_options(
        options,
        [
            Op::AddOutput(0),
            Op::AddWindow {
                params: TestWindowParams::new(0),
            },
            Op::AddWindow {
                params: TestWindowParams::new(1),
            },
            Op::AddWindow {
                params: TestWindowParams::new(2),
            },
        ],
    );

    let ops = [
        Op::FullscreenWindow(1),
        Op::Communicate(1),
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    let win = |id: usize| {
        layout
            .windows()
            .find(|(_mon, w)| w.id() == &id)
            .map(|(_mon, w)| w)
            .unwrap_or_else(|| panic!("window {id} not found"))
    };
    assert!(win(1).pending_sizing_mode().is_fullscreen());

    let ops = [
        Op::FullscreenWindow(1),
        Op::Communicate(1),
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    let win = |id: usize| {
        layout
            .windows()
            .find(|(_mon, w)| w.id() == &id)
            .map(|(_mon, w)| w)
            .unwrap_or_else(|| panic!("window {id} not found"))
    };

    // All three windows are back in the dwindle tree at normal tiled size.
    for id in [0, 1, 2] {
        assert!(
            win(id).pending_sizing_mode().is_normal(),
            "window {id} should no longer be fullscreen after unfullscreen"
        );
    }
    // All dwindle tiles are tiled half-width again (well under the monitor width).
    for id in [0, 1, 2] {
        let size = win(id).requested_size().unwrap();
        assert!(
            size.w < 1280,
            "window {id} should be tiled, not fullscreen width"
        );
    }
}

#[test]
fn scrolling_expel_creates_a_default_width_column() {
    let options = Options {
        layout: ymir_config::Layout {
            default_column_display: ymir_ipc::ColumnDisplay::Normal,
            default_column_width: Some(ymir_config::PresetSize::Proportion(0.5)),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut layout = check_ops_with_options(
        options,
        [
            Op::AddOutput(0),
            Op::AddWindow {
                params: TestWindowParams::new(0),
            },
            Op::AddWindow {
                params: TestWindowParams::new(1),
            },
            Op::AddWindow {
                params: TestWindowParams::new(2),
            },
            Op::ConsumeOrExpelWindowLeft { id: None },
            Op::ExpelWindowFromColumn,
            Op::CompleteAnimations,
        ],
    );

    // The expelled window gets its own column at the layout default (half monitor) width and is
    // not forced full width.
    let expelled = layout
        .windows()
        .find(|(_mon, w)| w.id() == &2)
        .map(|(_mon, w)| w.requested_size().unwrap())
        .unwrap();
    assert_eq!(expelled.w, 616);
}

#[test]
fn dwindle_expel_keeps_a_full_width_column() {
    let options = Options {
        layout: ymir_config::Layout {
            default_column_display: ymir_ipc::ColumnDisplay::Dwindle,
            ..Default::default()
        },
        ..Default::default()
    };
    let layout = check_ops_with_options(
        options,
        [
            Op::AddOutput(0),
            Op::AddWindow {
                params: TestWindowParams::new(0),
            },
            Op::AddWindow {
                params: TestWindowParams::new(1),
            },
            Op::AddWindow {
                params: TestWindowParams::new(2),
            },
            Op::ExpelWindowFromColumn,
            Op::CompleteAnimations,
        ],
    );

    // Dwindle expel intentionally yields a full-width page, not a half-width column.
    let expelled = layout
        .windows()
        .find(|(_mon, w)| w.id() == &2)
        .map(|(_mon, w)| w.requested_size().unwrap())
        .unwrap();
    assert_eq!(expelled.w, 1248);
}

#[test]
fn unfullscreen_view_offset_not_reset_during_gesture() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
        Op::FullscreenWindow(3),
        Op::Communicate(3),
        Op::ViewOffsetGestureBegin {
            output_idx: 1,
            workspace_idx: None,
            is_touchpad: false,
        },
        Op::FullscreenWindow(3),
        Op::Communicate(3),
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_view_offset_not_reset_during_ongoing_gesture() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
        Op::ViewOffsetGestureBegin {
            output_idx: 1,
            workspace_idx: None,
            is_touchpad: false,
        },
        Op::FullscreenWindow(3),
        Op::Communicate(3),
        Op::FullscreenWindow(3),
        Op::Communicate(3),
    ];

    check_ops(ops);
}

#[test]
fn unfullscreen_preserves_view_pos() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
    ];

    let mut layout = check_ops(ops);

    // View pos is looking at the first window.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"-16");

    let ops = [
        Op::FullscreenWindow(2),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    // View pos = width of first window + gap.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"116");

    let ops = [
        Op::FullscreenWindow(2),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    // View pos is back to showing the first window.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"-16");
}

#[test]
fn removing_only_fullscreen_tile_updates_view_offset() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::ConsumeOrExpelWindowLeft { id: None },
        Op::SetColumnDisplay(ColumnDisplay::Normal),
        Op::CompleteAnimations,
    ];

    let mut layout = check_ops(ops);

    // View pos with gap.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"-16");

    let ops = [
        Op::FullscreenWindow(2),
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    // View pos without gap because we went fullscreen.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"116");

    let ops = [
        Op::FullscreenWindow(2),
        // The active window responds, the other window doesn't yet.
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    // View pos is back on the first window: the unfullscreen step landed even though the other
    // tile was still fullscreen.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"-16");

    let ops = [
        // Expel the fullscreen window from the column, changing the column to non-fullscreen.
        Op::ConsumeOrExpelWindowRight { id: Some(1) },
        Op::CompleteAnimations,
    ];
    check_ops_on_layout(&mut layout, ops);

    // View pos should include gap now that the column is no longer fullscreen.
    // FIXME: currently, removing a tile doesn't cause the view offset to update.
    assert_snapshot!(layout.active_workspace().unwrap().scrolling().view_pos(), @"-132");
}
