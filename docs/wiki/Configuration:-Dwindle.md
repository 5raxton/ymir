### Overview

Dwindle is ymir's default [column layout](./Configuration:-Layout.md#default-column-display): instead of stacking windows linearly inside a column, every column slot is a recursive **binary-split** container. When a new window opens, it splits the focused window in two — so the focused window keeps its half and shrinks into its corner, while the newly opened window takes the freed-up half. This mirrors the classic "dwindle" behavior found in tiling WMs like Hyprland, implemented inside ymir's scrollable-tiling paradigm.

Every column has exactly one display mode:

- `normal` — classic scrollable tiling (windows stacked vertically in a column);
- `tabbed` — tabbed windows (see [Tabs](./Tabs.md));
- `dwindle` — recursive binary-split tree of windows.

New columns are created in dwindle mode out of the box: the shipped config sets `default-column-display "dwindle"` in its `layout {}` section. A ready-made config is also provided at [`resources/dwindle-config.kdl`](https://github.com/5raxton/ymir/blob/main/resources/dwindle-config.kdl).

The rest of this page documents the dwindle actions, key bindings, and behavior. The general rules for switching display modes live in [`default-column-display`](./Configuration:-Layout.md#default-column-display).

### Switching the layout mode

- `switch-column-display` — toggle the focused column between dwindle and scrollable (`normal`). A tabbed column is switched to dwindle. Bound to `Mod+Shift+D` in the default config.
- `set-column-display` — set the display mode explicitly: `"normal"`, `"tabbed"` or `"dwindle"`.

```kdl
binds {
    // Toggle the focused column between dwindle and scrollable tiling.
    Mod+Shift+D { switch-column-display; }
    // Set the focused column to dwindle explicitly.
    Mod+Shift+E { set-column-display "dwindle"; }
}
```

The mode you pick with `switch-column-display` is remembered **per workspace for the rest of the session**: new windows and any new columns opened in that workspace keep following your last toggle until you cycle it again (or a per-workspace `layout` config is applied).

To change the *default* mode for all new columns, use [`default-column-display`](./Configuration:-Layout.md#default-column-display). You can also set it per-window with a [window rule](./Configuration:-Window-Rules.md#default-column-display), or per [output](./Configuration:-Outputs.md#layout-config-overrides) / [named workspace](./Configuration:-Named-Workspaces.md#layout-config-overrides) via layout config overrides.

To make a *default* workspace dwindle without creating a named workspace, use a numeric [layout tag](./Configuration:-Named-Workspaces.md#non-creating-layout-tags-for-default-workspaces):

```kdl
workspace "1" {
    layout {
        default-column-display "dwindle"
    }
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
| `toggle-split` | Flip the split orientation of the container holding the focused window: switch between splitting side-by-side and stacking top-to-bottom. Bound to `Mod+Space`. |
| `preselect` | Set a one-time split direction for the next window opened in the focused column: `top`, `bottom`, `left` or `right`. Bound to `Mod+Ctrl+Space` (preselects the `bottom`). |
| `promote-window` | Move the focused window to the head of the dwindle tree (its first, leftmost leaf). Bound to `Mod+Shift+Home`. |
| `swap-window-right` / `swap-window-left` | Swap the focused window with the window in the column to the right / left. |
| `move-window-left` / `move-window-right` | Swap the focused window with its spatially adjacent leaf in the dwindle tree. Bound to `Mod+Shift+Left`/`Mod+Shift+Right` (and `Mod+Shift+H`/`L`). |
| `consume-window-into-column` | Consume one window from the right into the focused column. Bound to `Mod+Comma`. |
| `expel-window-from-column` | In dwindle mode, expel the focused window out of the column into its own column to the right; in other modes, the bottom-most window. Bound to `Mod+Period`. |
| `consume-or-expel-window-left` / `consume-or-expel-window-right` | In dwindle mode, move the focused window spatially (same as `move-window-left`/`right`); in other column modes, consume the focused window into the nearby column on that side, or expel it back out. Bound to `Mod+[` and `Mod+]`. |

These actions can also be triggered over IPC with `ymir msg action <action> [args]` (see the [IPC](./IPC.md) page):

```sh
ymir msg action preselect right
ymir msg action set-column-display dwindle
ymir msg action switch-column-display
ymir msg action toggle-split
```

Example config binding the dwindle actions:

```kdl
binds {
    Mod+Space { toggle-split; }
    Mod+Ctrl+Space { preselect "bottom"; }
    Mod+Shift+Home { promote-window; }
}
```

### Navigation & movement

Inside a dwindle column, the ordinary column/window focus and movement keys navigate the split tree **spatially** and only fall back to the neighboring column when there is no dwindle leaf in that direction:

- `focus-column-left` / `focus-column-right` (e.g. `Mod+Left` or `Mod+H`) — focus the dwindle leaf to the left, or the column to the left when there is none. Likewise for `right`.
- `focus-window-down` / `focus-window-up` (`Mod+Down`/`Mod+Up` or `Mod+J`/`Mod+K`) — focus the dwindle leaf below / above.
- `move-window-left` / `move-window-right` (`Mod+Shift+Left`/`Right` or `Mod+Shift+H`/`L`) — swap the focused window with its spatial neighbor in the dwindle tree.
- `move-window-down` / `move-window-up` (`Mod+Ctrl+Down`/`Up` or `Mod+Ctrl+J`/`K`) — in dwindle mode, swap the focused window with the leaf spatially below / above; in other modes, move it vertically as usual.
- `move-column-left` / `move-column-right` (`Mod+Ctrl+Left`/`Right`, etc.) — move the focused window to the spatially adjacent leaf on that side, falling back to moving the whole column.

Because of this, the usual vim-style `H`/`J`/`K`/`L` keys keep working as expected inside a dwindle column — they simply navigate the binary tree instead of a linear stack, and left/right can step out to the surrounding scrollable columns.

### Full-width columns

A dwindle column spans the whole work area width (like Hyprland's dwindle), so its split tree partitions the entire screen instead of being confined to a narrow scrollable strip. Multiple dwindle columns can still coexist on a workspace — each one occupies the full work area and is placed on the strip like any other full-width column.
