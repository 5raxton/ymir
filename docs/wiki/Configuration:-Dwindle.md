### Overview

Dwindle is ymir's default [column layout](./Configuration:-Layout.md#default-column-display): instead of stacking windows linearly inside a column, every column slot is a recursive **binary-split** container. When a new window opens, it splits the focused window in two — so the focused window keeps its half and shrinks into its corner, while the newly opened window takes the freed-up half. This mirrors the classic "dwindle" behavior found in tiling WMs like Hyprland, implemented inside ymir's scrollable-tiling paradigm.

Every column has exactly one display mode:

- `normal` — classic scrollable tiling (windows stacked vertically in a column);
- `dwindle` — recursive binary-split tree of windows;

New columns are created in dwindle mode out of the box: the shipped config sets `default_column_display = "dwindle"` in its `layout` table. A ready-made config is also provided at [`resources/dwindle-config.lua`](https://lab.braxton.onl/braxton/ymir/src/branch/main/resources/dwindle-config.lua).

The rest of this page documents the dwindle actions, key bindings, and behavior. The general rules for switching display modes live in [`default-column-display`](./Configuration:-Layout.md#default-column-display).

### Switching the layout mode

- `switch_column_display` — toggle the focused column between dwindle and scrollable (`normal`). Bound to `Mod+Shift+D` in the default config.
- `set_column_display` — set the display mode explicitly: `"normal"` or `"dwindle"`.

```lua
return {
    binds = {
        -- Toggle the focused column between dwindle and scrollable tiling.
        { key = "Mod+Shift+D", action = { name = "switch_column_display" } },
        -- Set the focused column to dwindle explicitly.
        { key = "Mod+Shift+E", action = { name = "set_column_display", display = "dwindle" } },
    },
}
```

The mode you pick with `switch_column_display` is remembered **per workspace for the rest of the session**: new windows and any new columns opened in that workspace keep following your last toggle until you cycle it again (or a per-workspace `layout` config is applied).

To change the *default* mode for all new columns, use [`default_column_display`](./Configuration:-Layout.md#default-column-display). You can also set it per-window with a [window rule](./Configuration:-Window-Rules.md#default-column-display), or per [output](./Configuration:-Outputs.md#layout-config-overrides) / [named workspace](./Configuration:-Named-Workspaces.md#layout-config-overrides) via layout config overrides.

To make a *default* workspace dwindle without creating a named workspace, use a numeric [layout tag](./Configuration:-Named-Workspaces.md#non-creating-layout-tags-for-default-workspaces):

```lua
return {
    workspaces = {
        { name = "1", layout = { default_column_display = "dwindle" } },
    },
}
```

### How new windows split

When a window opens in a dwindle column, the focused leaf's region is sliced into two:

- the focused window keeps the first (top-left) half and shrinks into its corner;
- the newly opened window takes the freed-up half.

The split direction is chosen like this:

1. If a split direction was preselected with `preselect` (below), that direction is used, and the preselection is consumed (it only applies to the *next* window).
2. Otherwise, the direction follows the current region's shape: wide regions split side-by-side (new window to the right), while tall or square regions stack (new window at the bottom).

New splits start at an equal 50/50 proportion. The engine clamps split ratios to the 10–90% range.

### Dwindle actions

These actions only affect columns in dwindle mode:

| Action | Effect |
| --- | --- |
| `toggle_split` | Flip the split orientation of the container holding the focused window: switch between splitting side-by-side and stacking top-to-bottom. Bound to `Mod+Space`. |
| `preselect` | Set a one-time split direction for the next window opened in the focused column: `top`, `bottom`, `left` or `right`. Bound to `Mod+Ctrl+Space` (preselects the `bottom`). |
| `promote_window` | Move the focused window to the head of the dwindle tree (its first, leftmost leaf). Bound to `Mod+Shift+Home`. |
| `swap_window_right` / `swap_window_left` | Swap the focused window with the window in the column to the right / left. |
| `move_window_left` / `move_window_right` | Swap the focused window with its spatially adjacent leaf in the dwindle tree. Bound to `Mod+Shift+Left`/`Mod+Shift+Right` (and `Mod+Shift+H`/`L`). |
| `consume_window_into_column` | Consume one window from the right into the focused column. Bound to `Mod+Comma`. |
| `expel_window_from_column` | In dwindle mode, expel the focused window out of the column into its own column to the right; in other modes, the bottom-most window. Bound to `Mod+Period`. |
| `consume_or_expel_window_left` / `consume_or_expel_window_right` | In dwindle mode, move the focused window spatially (same as `move_window_left`/`right`); in other column modes, consume the focused window into the nearby column on that side, or expel it back out. Bound to `Mod+[` and `Mod+]`. |

These actions can also be triggered over IPC with `ymir msg action <action> [args]` (see the [IPC](./IPC.md) page):

```sh
ymir msg action preselect right
ymir msg action set-column-display dwindle
ymir msg action switch-column-display
ymir msg action toggle-split
```

Example config binding the dwindle actions:

```lua
return {
    binds = {
        { key = "Mod+Space", action = { name = "toggle_split" } },
        { key = "Mod+Ctrl+Space", action = { name = "preselect", direction = "bottom" } },
        { key = "Mod+Shift+Home", action = { name = "promote_window" } },
    },
}
```

### Navigation & movement

Inside a dwindle column, the ordinary column/window focus and movement keys navigate the split tree **spatially** and only fall back to the neighboring column when there is no dwindle leaf in that direction:

- `focus_column_left` / `focus_column_right` (e.g. `Mod+Left` or `Mod+H`) — focus the dwindle leaf to the left, or the column to the left when there is none. Likewise for `right`.
- `focus_window_down` / `focus_window_up` (`Mod+Down`/`Mod+Up` or `Mod+J`/`Mod+K`) — focus the dwindle leaf below / above.
- `move_window_left` / `move_window_right` (`Mod+Shift+Left`/`Right` or `Mod+Shift+H`/`L`) — swap the focused window with its spatial neighbor in the dwindle tree.
- `move_window_down` / `move_window_up` (`Mod+Ctrl+Down`/`Up` or `Mod+Ctrl+J`/`K`) — in dwindle mode, swap the focused window with the leaf spatially below / above; in other modes, move it vertically as usual.
- `move_column_left` / `move_column_right` (`Mod+Ctrl+Left`/`Right`, etc.) — move the focused window to the spatially adjacent leaf on that side, falling back to moving the whole column.

Because of this, the usual vim-style `H`/`J`/`K`/`L` keys keep working as expected inside a dwindle column — they simply navigate the binary tree instead of a linear stack, and left/right can step out to the surrounding scrollable columns.

### Full-width columns

A dwindle column spans the whole work area width (like Hyprland's dwindle), so its split tree partitions the entire screen instead of being confined to a narrow scrollable strip. Multiple dwindle columns can still coexist on a workspace — each one occupies the full work area and is placed on the strip like any other full-width column.
