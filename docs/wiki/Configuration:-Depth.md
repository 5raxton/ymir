### Overview

Depth-queue (`depth`) is a [column display mode](./Configuration:-Layout.md#default-column-display) that turns a column into a physical stack of paper cards with perspective depth-sorting. The **focused window sits at the "apex" card**: full column width, fully opaque, interactive. The rest of the queue fans into a **top and a bottom deck** behind it — each card squashed, tilted and faded (down to `min_opacity`) toward the far edge, with a hardware-accelerated background blur behind the fan and per-card shadows.

Every column has exactly one display mode:

- `normal` — classic scrollable tiling (windows stacked vertically in a column);
- `tabbed` — tabbed windows (see [Tabs](./Tabs.md));
- `dwindle` — recursive binary-split tree of windows (see [Dwindle](./Configuration:-Dwindle.md));
- `depth` — the depth-sorted card stack described on this page.

The general rules for switching display modes live in [`default-column-display`](./Configuration:-Layout.md#default-column-display).

### Switching the layout mode

- `switch_column_display` — toggle the focused column through the display modes (`normal` → `dwindle` → `depth` → `tabbed`).
- `set_column_display` — set the display mode explicitly: `"normal"`, `"tabbed"`, `"dwindle"` or `"depth"`.

```lua
return {
    binds = {
        -- Set the focused column to the depth card stack explicitly.
        { key = "Mod+Shift+Q", action = { name = "set_column_display", display = "depth" } },
    },
}
```

The mode you pick with `switch_column_display` is remembered **per workspace for the rest of the session**, exactly like the other display modes. To make a *default* workspace start in depth mode without creating a named workspace, use a numeric [layout tag](./Configuration:-Named-Workspaces.md#non-creating-layout-tags-for-default-workspaces):

```lua
return {
    workspaces = {
        { name = "2", layout = { default_column_display = "depth" } },
    },
}
```

### Depth actions

These actions only affect columns in depth mode:

| Action | Effect |
| --- | --- |
| `focus_card_up` / `focus_card_down` | Focus the card above / below the apex in the queue (the deck cards re-fan around the new apex). |
| `push_to_queue` | Move the focused window to the far end of the depth queue; the next window slides into the apex slot. |
| `pull_to_apex` | Promote a window to the apex immediately: `pull_to_apex` promotes the focused window, `pull_to_apex <id>` the window with the given id. The queue springs into the new order. |
| `cycle_queue_depth` | Alt+Tab-style cycler: wrap to the next card in the queue, focusing it at the apex. |
| `toggle_queue_cover` | Toggle "cover" mode: the whole deck fans out so every card peeks at once, instead of only the handful nearest the apex. |

All of these can also be triggered over IPC with `ymir msg action <action> [args]` (see the [IPC](./IPC.md) page):

```sh
ymir msg action set-column-display depth
ymir msg action focus-card-down
ymir msg action push-to-queue
ymir msg action pull-to-apex 42
ymir msg action cycle-queue-depth
ymir msg action toggle-queue-cover
```

Example config binding the depth actions:

```lua
return {
    binds = {
        { key = "Mod+Shift+Q", action = { name = "set_column_display", display = "depth" } },
        { key = "Mod+Down", action = { name = "focus_card_down" } },
        { key = "Mod+Up", action = { name = "focus_card_up" } },
        { key = "Mod+Shift+S", action = { name = "push_to_queue" } },
        { key = "Mod+Ctrl+P", action = { name = "pull_to_apex" } },
        { key = "Mod+Tab", action = { name = "cycle_queue_depth" } },
        { key = "Mod+Shift+C", action = { name = "toggle_queue_cover" } },
    },
}
```

### How the deck renders

The apex card uses the normal window rendering path (frame, border and focus ring intact). Every deck card is drawn as a squashed preview:

- each card's **near edge** (the one touching the apex) is stretched with a wide-lens remap, while the surface compresses toward the far edge — the fake perspective is driven by `perspective_tilt`;
- cards **fade** toward their far edge down to `min_opacity` (the apex is always fully opaque);
- a backdrop **blur** (radius `blur_radius`, `0` disables it) softens whatever sits behind the whole fan;
- each card casts a small **shadow** according to `card_shadow`, so the decks read as physically stacked paper.

Focus transitions spring-animate: the whole queue re-fans with a smooth "shuffle" whose feel is tuned by `focus_shuffle`. A rapid chain of `focus_card_*` inputs mid-shuffle chains the next re-fan without teleports.

### `depth_queue` config

All settings live under `layout.depth_queue` (defaults shipped in the default config):

| Key | Default | Meaning |
| --- | --- | --- |
| `card_height_ratio` | `0.62` | Height of the apex card relative to the column height (0..1). |
| `top_deck_size` / `bottom_deck_size` | `2` / `2` | Number of cards visible in the top / bottom deck fan. |
| `gap` | `12` | Vertical gap between consecutive cards in the decks. |
| `deck_bleed` | `24` | How far a deck card bleeds past the working area edge. |
| `min_opacity` | `0.35` | Opacity of the farthest (most occluded) card in a deck. |
| `blur_radius` | `18` | Blur radius applied to the backdrop behind the decks (`0` disables it). |
| `card_shadow` | `{ offset = {x=0, y=10}, blur = 24 }` | Shadow cast by each deck card. |
| `perspective_tilt` | `7.0` | Perspective tilt of the deck fans, in degrees (`0` disables the tilt). |
| `focus_shuffle` | spring `{ 0.62, 750, 0.0001 }` | Spring driving the focus shuffle between cards. |