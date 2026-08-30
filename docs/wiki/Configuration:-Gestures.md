### Overview

<sup>Since: 25.02</sup>

The `gestures` config section contains gesture settings.
For an overview of all ymir gestures, see the [Gestures](./Gestures.md) wiki page.

Here's a quick glance at the available settings along with their default values.

```lua
return {
    gestures = {
        dnd_edge_view_scroll = {
            trigger_width = 30,
            delay_ms = 100,
            max_speed = 1500,
        },

        dnd_edge_workspace_switch = {
            trigger_height = 50,
            delay_ms = 100,
            max_speed = 1500,
        },

        hot_corners = {
            -- off = true
            top_left = true,
            -- top_right = true
            -- bottom_left = true
            -- bottom_right = true
        },
    },
}
```

### `dnd-edge-view-scroll`

Scroll the tiling view when moving the mouse cursor against a monitor edge during drag-and-drop (DnD).
Also works on a touchscreen.

This will work for regular drag-and-drop (e.g. dragging a file from a file manager), and for window interactive move when targeting the tiling layout.

The options are:

- `trigger_width`: size of the area near the monitor edge that will trigger the scrolling, in logical pixels.
- `delay_ms`: delay in milliseconds before the scrolling starts.
Avoids unwanted scrolling when dragging things across monitors.
- `max_speed`: maximum scrolling speed in logical pixels per second.
The scrolling speed increases linearly as you move your mouse cursor from `trigger_width` to the very edge of the monitor.

```lua
return {
    gestures = {
        -- Increase the trigger area and maximum speed.
        dnd_edge_view_scroll = {
            trigger_width = 100,
            max_speed = 3000,
        },
    },
}
```

### `dnd-edge-workspace-switch`

<sup>Since: 25.05</sup>

Scroll the workspaces up/down when moving the mouse cursor against a monitor edge during drag-and-drop (DnD) while in the overview.
Also works on a touchscreen.

The options are:

- `trigger_height`: size of the area near the monitor edge that will trigger the scrolling, in logical pixels.
- `delay_ms`: delay in milliseconds before the scrolling starts.
Avoids unwanted scrolling when dragging things across monitors.
- `max_speed`: maximum scrolling speed; 1500 corresponds to one screen height per second.
The scrolling speed increases linearly as you move your mouse cursor from `trigger_width` to the very edge of the monitor.

```lua
return {
    gestures = {
        -- Increase the trigger area and maximum speed.
        dnd_edge_workspace_switch = {
            trigger_height = 100,
            max_speed = 3000,
        },
    },
}
```

### `hot-corners`

<sup>Since: 25.05</sup>

Put your mouse at the very top-left corner of a monitor to toggle the overview.
Also works during drag-and-dropping something.

`off` disables the hot corners.

```lua
return {
    gestures = {
        -- Disable the hot corners.
        hot_corners = {
            off = true,
        },
    },
}
```

<sup>Since: 25.11</sup> You can choose specific hot corners by name: `top_left`, `top_right`, `bottom_left`, `bottom_right`.
If no corners are explicitly set, the top-left corner will be active by default.

```lua
return {
    gestures = {
        -- Enable the top-right and bottom-right hot corners.
        hot_corners = {
            top_right = true,
            bottom_right = true,
        },
    },
}
```

You can also customize hot corners per-output [in the output config](./Configuration:-Outputs.md#hot-corners).