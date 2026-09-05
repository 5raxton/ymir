use super::*;

fn dwindle_options() -> Options {
    Options {
        layout: ymir_config::Layout {
            default_column_display: ColumnDisplay::Dwindle,
            gaps: 16.0,
            default_column_width: Some(PresetSize::Proportion(0.5)),
            ..ymir_config::Layout::default()
        },
        ..Options::default()
    }
}

fn win(id: usize) -> TestWindowParams {
    TestWindowParams {
        bbox: Rectangle::from_size(Size::from((1024, 700))),
        ..TestWindowParams::new(id)
    }
}

fn add(layout: &mut Layout<TestWindow>, id: usize) {
    layout.add_window(
        TestWindow::new(win(id)),
        AddWindowTarget::Auto,
        None,
        None,
        false,
        false,
        ActivateWindow::Yes,
        None,
    );
    // Apply pending configure sizes like a real client would, then settle animations.
    for i in 0..=id {
        check_ops_on_layout(
            layout,
            [Op::Communicate(i), Op::Refresh { is_active: true }],
        );
    }
    check_ops_on_layout(layout, [Op::CompleteAnimations]);
}

fn pos_of(layout: &Layout<TestWindow>, id: usize) -> (f64, f64) {
    let pos = layout
        .active_workspace()
        .unwrap()
        .tiles_with_render_positions()
        .find(|(tile, ..)| *tile.window().id() == id)
        .unwrap()
        .1;
    (pos.x, pos.y)
}

/// Regressions for the four dwindle bugs: unpredictable movement with 3+ windows, consume/expel
/// going full-width instead of half-width, and switches between dwindle and scrolling getting
/// stuck (off-screen page, then unusable).
#[test]
fn dwindle_move_swaps_with_spatial_neighbor_without_teleporting_others() {
    let mut layout = check_ops_with_options(
        dwindle_options(),
        [Op::AddOutput(0), Op::CompleteAnimations],
    );
    for id in 0..4 {
        add(&mut layout, id);
    }
    check_ops_on_layout(&mut layout, [Op::FocusWindow(3)]);

    // dwindle tree: H{0, V{1, H{2,3}}}; win0 is the tall left column.
    let before: Vec<(f64, f64)> = (0..4).map(|id| pos_of(&layout, id)).collect();
    assert!(
        before[0].0 < before[1].0,
        "window 0 starts on the left of window 1"
    );

    // Moving win3 left swaps it with win2 (its direct divider neighbor): win2 lands exactly where
    // win3 was, win0/win1 must not move at all, and win3 takes win2's spot.
    check_ops_on_layout(&mut layout, [Op::FocusWindow(3)]);
    layout.move_window_left();
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);

    assert_eq!(
        *layout.focus().unwrap().id(),
        3,
        "focus stays on the moved window"
    );
    let after = pos_of(&layout, 3);
    assert!(
        after.0 < before[3].0,
        "window 3 moved left (x {}->{})",
        before[3].0,
        after.0
    );
    assert_eq!(after, before[2], "window 3 took window 2's old spot");
    assert_eq!(
        pos_of(&layout, 2),
        before[3],
        "window 2 took window 3's old spot"
    );
    assert_eq!(pos_of(&layout, 0), before[0], "window 0 untouched");
    assert_eq!(pos_of(&layout, 1), before[1], "window 1 untouched");

    // Moving win3 left again climbs to the next divider: it swaps with window 0 (the tall left
    // column); still only the two swapped windows move.
    layout.move_window_left();
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);
    let after2 = pos_of(&layout, 3);
    assert_eq!(after2, before[0], "window 3 reached window 0's old spot");
    assert_eq!(
        pos_of(&layout, 0),
        after,
        "window 0 took window 3's old spot"
    );
    assert_eq!(pos_of(&layout, 1), before[1], "window 1 still untouched");
    assert_eq!(
        pos_of(&layout, 2),
        before[3],
        "window 2 still at win3's start spot"
    );
}

/// Switching a multi-column scrolling layout to dwindle must re-anchor the view so the single
/// dwindle page fills the work area, and toggling back must return to a usable scrolling layout --
/// the view must not stay stuck off-screen after repeated switches.
#[test]
fn dwindle_switch_reanchors_view_each_way() {
    let mut layout = check_ops_with_options(
        Options::default(),
        [Op::AddOutput(0), Op::CompleteAnimations],
    );
    for id in 0..3 {
        add(&mut layout, id);
    }
    // Welcome back to scrolling: three Fixed-width columns; window 2 in the rightmost column.
    let col_count_before = layout
        .active_workspace()
        .unwrap()
        .scrolling()
        .columns()
        .count();
    assert_eq!(col_count_before, 3);

    layout.switch_column_display();
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);
    let ws = layout.active_workspace().unwrap();

    // Toggling the (scrolling-default) column to dwindle consolidates everything into one page.
    assert_eq!(ws.scrolling().columns().count(), 1);
    // The dwindle page is anchored at the left edge, exactly like a fresh dwindle workspace.
    assert_eq!(ws.scrolling().view_pos(), -16.0);
    for id in 0..3 {
        let (x, _) = pos_of(&layout, id);
        assert!(
            (-1.0..1280.0).contains(&x),
            "window {id} is on screen (x={x:.1})"
        );
    }

    // Switching back to scrolling must not leave the strip scrolled to a stale offset.
    layout.switch_column_display();
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);
    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.scrolling().columns().count(), 1);
    assert_eq!(ws.scrolling().view_pos(), -16.0);
    for id in 0..3 {
        let (x, _) = pos_of(&layout, id);
        assert!(
            (-1.0..1280.0).contains(&x),
            "back in scrolling, window {id} on screen (x={x:.1})"
        );
    }

    // And dwindle again (the state that used to get stuck).
    layout.switch_column_display();
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);
    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.scrolling().columns().count(), 1);
    assert_eq!(ws.scrolling().view_pos(), -16.0);
    for id in 0..3 {
        let (x, _) = pos_of(&layout, id);
        assert!(
            (-1.0..1280.0).contains(&x),
            "dwindle again, window {id} on screen (x={x:.1})"
        );
    }
}

/// Consuming/expelling around a dwindle column must produce half-width columns, never a full-width
/// page. In dwindle, both actions are spatial swaps; leaving dwindle (expel) splits the tree into
/// two visible half-width scrolling columns.
#[test]
fn dwindle_consume_expel_keeps_half_width_columns() {
    let mut layout = check_ops_with_options(
        dwindle_options(),
        [Op::AddOutput(0), Op::CompleteAnimations],
    );
    add(&mut layout, 0);
    add(&mut layout, 1);
    add(&mut layout, 2);
    check_ops_on_layout(&mut layout, [Op::FocusWindow(0)]);

    // Dwindle tree: win0 = tall left column, win1/w2 = right column. consume_or_expel_right in
    // dwindle swaps win0 with win1 (its divider neighbor) instead of going full-width.
    let before = pos_of(&layout, 0);
    layout.consume_or_expel_window_right(None);
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);
    let after = pos_of(&layout, 0);
    assert!(
        after.0 > before.0,
        "win0 moved right (x {}->{})",
        before.0,
        after.0
    );
    let widths: Vec<i32> = (0..3)
        .map(|id| {
            layout
                .windows()
                .find(|(_, w)| w.id() == &id)
                .unwrap()
                .1
                .requested_size()
                .unwrap()
                .w
        })
        .collect();
    assert!(
        widths.iter().all(|&w| w < 1000),
        "no window is full-width after a dwindle swap (got {widths:?})"
    );

    // Leaving dwindle mode properly splits into half-width columns.
    layout.expel_from_column();
    check_ops_on_layout(&mut layout, [Op::CompleteAnimations]);
    let ws = layout.active_workspace().unwrap();
    assert_eq!(
        ws.scrolling().columns().count(),
        2,
        "expel leaves two columns"
    );
    // Both columns sit side-by-side on screen (a dwindle page would be the sole full-width one).
    let xs: Vec<f64> = (0..3).map(|id| pos_of(&layout, id).0).collect();
    assert!(
        xs.iter().any(|x| *x > 600.0) && xs.iter().any(|x| *x < 20.0),
        "two columns sit side by side, left and right of center (got {xs:?})"
    );
    let expanded: Vec<i32> = (0..3)
        .map(|id| {
            layout
                .windows()
                .find(|(_, w)| w.id() == &id)
                .unwrap()
                .1
                .requested_size()
                .unwrap()
                .w
        })
        .collect();
    assert!(
        expanded.iter().all(|&w| w < 1000),
        "no window is full-width after leaving dwindle (got {expanded:?})"
    );
}
