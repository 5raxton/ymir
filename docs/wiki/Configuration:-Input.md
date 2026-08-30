### Overview

In this section you can configure input devices like keyboard and mouse, and some input-related options.

There's a section for each device type: `keyboard`, `touchpad`, `mouse`, `trackpoint`, `trackball`, `tablet`, `touch`.
Settings in those sections will apply to every device of that type.
Currently, there's no way to configure specific devices individually (but that is planned).

All settings at a glance:

```lua
return {
    input = {
        keyboard = {
            xkb = {
                -- layout = "us"
                -- variant = "colemak_dh_ortho"
                -- options = "compose:ralt,ctrl:nocaps"
                -- model = ""
                -- rules = ""
                -- file = "~/.config/keymap.xkb"
            },

            -- repeat_delay = 600
            -- repeat_rate = 25
            -- track_layout = "global"
            numlock = true,
        },

        touchpad = {
            -- off = true
            tap = true,
            -- dwt = true
            -- dwtp = true
            -- drag = false
            -- drag_lock = true
            natural_scroll = true,
            -- accel_speed = 0.2
            -- accel_profile = "flat"
            -- scroll_factor = 1.0
            -- scroll_factor = { vertical = 1.0, horizontal = -2.0 }
            -- scroll_method = "two-finger"
            -- scroll_button = 273
            -- scroll_button_lock = true
            -- tap_button_map = "left-middle-right"
            -- click_method = "clickfinger"
            -- left_handed = true
            -- disabled_on_external_mouse = true
            -- middle_emulation = true
        },

        mouse = {
            -- off = true
            -- natural_scroll = true
            -- accel_speed = 0.2
            -- accel_profile = "flat"
            -- scroll_factor = 1.0
            -- scroll_factor = { vertical = 1.0, horizontal = -2.0 }
            -- scroll_method = "no-scroll"
            -- scroll_button = 273
            -- scroll_button_lock = true
            -- left_handed = true
            -- middle_emulation = true
        },

        trackpoint = {
            -- off = true
            -- natural_scroll = true
            -- accel_speed = 0.2
            -- accel_profile = "flat"
            -- scroll_method = "on-button-down"
            -- scroll_button = 273
            -- scroll_button_lock = true
            -- left_handed = true
            -- middle_emulation = true
        },

        trackball = {
            -- off = true
            -- natural_scroll = true
            -- accel_speed = 0.2
            -- accel_profile = "flat"
            -- scroll_method = "on-button-down"
            -- scroll_button = 273
            -- scroll_button_lock = true
            -- left_handed = true
            -- middle_emulation = true
        },

        tablet = {
            -- off = true
            map_to_output = "eDP-1",
            -- map_to_focused_output = true
            -- map_to_focused_window = true
            -- left_handed = true
            -- calibration_matrix = { 1.0, 0.0, 0.0, 0.0, 1.0, 0.0 }
        },

        touch = {
            -- off = true
            map_to_output = "eDP-1",
            -- calibration_matrix = { 1.0, 0.0, 0.0, 0.0, 1.0, 0.0 }
        },

        -- disable_power_key_handling = true
        -- warp_mouse_to_focus = true
        -- focus_follows_mouse = { max_scroll_amount = "0%" }
        -- workspace_auto_back_and_forth = true

        -- mod_key = "Super"
        -- mod_key_nested = "Alt"
    },
}
```

### Keyboard

#### Layout

In the `xkb` section, you can set layout, variant, options, model and rules.
These are passed directly to libxkbcommon, which is also used by most other Wayland compositors.
See the `xkeyboard-config(7)` manual for more information.

```lua
return {
    input = {
        keyboard = {
            xkb = {
                layout = "us",
                variant = "colemak_dh_ortho",
                options = "compose:ralt,ctrl:nocaps",
            },
        },
    },
}
```

> [!TIP]
>
> <sup>Since: 25.02</sup>
>
> Alternatively, you can directly set a path to a .xkb file containing an xkb keymap.
> This overrides all other xkb settings.
>
> ```lua
> return {
>     input = {
>         keyboard = {
>             xkb = {
>                 file = "~/.config/keymap.xkb",
>             },
>         },
>     },
> }
> ```

> [!NOTE]
>
> <sup>Since: 25.08</sup>
>
> If the `xkb` section is empty (like it is by default), ymir will fetch xkb settings from systemd-localed at `org.freedesktop.locale1` over D-Bus.
> This way, for example, system installers can dynamically set the ymir keyboard layout.
> You can see this layout in `localectl` and change it with `localectl set-x11-keymap`, for example:
>
> ```sh
> $ localectl set-x11-keymap "us" "" "colemak_dh_ortho" "compose:ralt,ctrl:nocaps"
> $ localectl
> System Locale: LANG=en_US.UTF-8
>                LC_NUMERIC=ru_RU.UTF-8
>                LC_TIME=ru_RU.UTF-8
>                LC_MONETARY=ru_RU.UTF-8
>                LC_PAPER=ru_RU.UTF-8
>                LC_MEASUREMENT=ru_RU.UTF-8
>     VC Keymap: us-colemak_dh_ortho
>    X11 Layout: us
>   X11 Variant: colemak_dh_ortho
>   X11 Options: compose:ralt,ctrl:nocaps
> ```
>
> By default, `localectl` will set the TTY keymap to the closest match of the XKB keymap.
> You can prevent that with a `--no-convert` flag, for example: `localectl set-x11-keymap --no-convert "us,ru"`.
>
> These settings are picked up by some other programs too, like GDM.

When using multiple layouts, ymir can remember the current layout globally (the default) or per-window.
You can control this with the `track_layout` option.

- `global`: layout change is global for all windows.
- `window`: layout is tracked for each window individually.

```lua
return {
    input = {
        keyboard = {
            track_layout = "global",
        },
    },
}
```

#### Repeat

Delay is in milliseconds before the keyboard repeat starts.
Rate is in characters per second.

```lua
return {
    input = {
        keyboard = {
            repeat_delay = 600,
            repeat_rate = 25,
        },
    },
}
```

#### Num Lock

<sup>Since: 25.05</sup>

Set the `numlock` flag to turn on Num Lock automatically at startup.

You might want to disable (comment out) `numlock` if you're using a laptop with a keyboard that overlays Num Lock keys on top of regular keys.

```lua
return {
    input = {
        keyboard = {
            numlock = true,
        },
    },
}
```

### Pointing Devices

Most settings for the pointing devices are passed directly to libinput.
Other Wayland compositors also use libinput, so it's likely you will find the same settings there.
For flags like `tap`, omit them or comment them out to disable the setting.

A few settings are common between input devices:

- `off`: if set, no events will be sent from this device.

A few settings are common between `touchpad`, `mouse`, `trackpoint`, and `trackball`:

- `natural_scroll`: if set, inverts the scrolling direction.
- `accel_speed`: pointer acceleration speed, valid values are from `-1.0` to `1.0` where the default is `0.0`.
- `accel_profile`: can be `adaptive` (the default) or `flat` (disables pointer acceleration).
- `scroll_method`: when to generate scroll events instead of pointer motion events, can be `no-scroll`, `two-finger`, `edge`, or `on-button-down`.
  The default and supported methods vary depending on the device type.
- `scroll_button`: <sup>Since: 0.1.10</sup> the button code used for the `on-button-down` scroll method. You can find it in `libinput debug-events`.
- `scroll_button_lock`: <sup>Since: 25.08</sup> when enabled, the button does not need to be held down. Pressing once engages scrolling, pressing a second time disengages it, and double click acts as single click of the the underlying button.
- `left_handed`: if set, changes the device to left-handed mode.
- `middle_emulation`: emulate a middle mouse click by pressing left and right mouse buttons at once.

Settings specific to `touchpad`s:

- `tap`: tap-to-click.
- `dwt`: disable-when-typing.
- `dwtp`: disable-when-trackpointing.
- `drag`: <sup>Since: 25.05</sup> can be `true` or `false`, controls if tap-and-drag is enabled.
- `drag_lock`: <sup>Since: 25.02</sup> if set, lifting the finger off for a short time while dragging will not drop the dragged item. See the [libinput documentation](https://wayland.freedesktop.org/libinput/doc/latest/tapping.html#tap-and-drag).
- `tap_button_map`: can be `left-right-middle` or `left-middle-right`, controls which button corresponds to a two-finger tap and a three-finger tap.
- `click_method`: can be `button-areas` or `clickfinger`, changes the [click method](https://wayland.freedesktop.org/libinput/doc/latest/clickpad-softbuttons.html).
- `disabled_on_external_mouse`: do not send events while external pointer device is plugged in.

Settings specific to `touchpad` and `mouse`:

- `scroll_factor`: <sup>Since: 0.1.10</sup> scales the scrolling speed by this value.

    <sup>Since: 25.08</sup> You can also override horizontal and vertical scroll factor separately like so: `scroll_factor = { horizontal = 2.0, vertical = -1.0 }`

Settings specific to `tablet` and `touch`:

- `calibration_matrix`: set to six floating point numbers to change the calibration matrix. See the [`LIBINPUT_CALIBRATION_MATRIX` documentation](https://wayland.freedesktop.org/libinput/doc/latest/device-configuration-via-udev.html) for examples.
    - <sup>Since: 25.02</sup> for `tablet`
    - <sup>Since: 25.11</sup> for `touch`

Tablets and touchscreens are absolute pointing devices that can be mapped to a specific output like so:

```lua
return {
    input = {
        tablet = {
            map_to_output = "eDP-1",
        },
        touch = {
            map_to_output = "eDP-1",
        },
    },
}
```

Valid output names are the same as the ones used for output configuration.

<sup>Since: 0.1.7</sup> When a tablet is not mapped to any output, it will map to the union of all connected outputs, without aspect ratio correction.

Settings specific to `tablet`:

- `map_to_focused_output`: <sup>Since: 26.04</sup> will map the tablet to the focused output, takes precedence over `map_to_output`.

- `map_to_focused_window`: <sup>Since: next release</sup> will map the tablet to the focused window's geometry, takes precedence over `map_to_focused_output` and `map_to_output`.
Falls back to those when no window is focused (for example, in the overview).

    When the tablet is also mapped to a specific output via `map_to_output`, the `map_to_focused_window` flag will map the tablet to the active window on that output.
    If the tablet isn't mapped to any specific output, it will map the tablet to the current focused window regardless of where it is.

### General Settings

These settings are not specific to a particular input device.

#### `disable_power_key_handling`

By default, ymir will take over the power button to make it sleep instead of power off.
Set this if you would like to configure the power button elsewhere (i.e. `logind.conf`).

```lua
return {
    input = {
        disable_power_key_handling = true,
    },
}
```

#### `warp_mouse_to_focus`

Makes the mouse warp to newly focused windows.

Does not make the cursor visible if it had been hidden.

```lua
return {
    input = {
        warp_mouse_to_focus = true,
    },
}
```

By default, the cursor warps *separately* horizontally and vertically.
I.e. if moving the mouse only horizontally is enough to put it inside the newly focused window, then the mouse will move only horizontally, and not vertically.

<sup>Since: 25.05</sup> You can customize this with the `mode` property.

- `mode = "center-xy"`: warps by both X and Y coordinates together.
So if the mouse was anywhere outside the newly focused window, it will warp to the center of the window.
- `mode = "center-xy-always"`: warps by both X and Y coordinates together, even if the mouse was already somewhere inside the newly focused window.

```lua
return {
    input = {
        warp_mouse_to_focus = { mode = "center-xy" },
    },
}
```

#### `focus_follows_mouse`

Focuses windows and outputs automatically when moving the mouse over them.

```lua
return {
    input = {
        focus_follows_mouse = true,
    },
}
```

<sup>Since: 0.1.8</sup> You can optionally set `max_scroll_amount`.
Then, focus_follows_mouse won't focus a window if it will result in the view scrolling more than the set amount.
The value is a percentage of the working area width.

```lua
return {
    input = {
        -- Allow focus-follows-mouse when it results in scrolling at most 10% of the screen.
        focus_follows_mouse = { max_scroll_amount = "10%" },
    },
}
```

```lua
return {
    input = {
        -- Allow focus-follows-mouse only when it will not scroll the view.
        focus_follows_mouse = { max_scroll_amount = "0%" },
    },
}
```

#### `workspace_auto_back_and_forth`

Normally, switching to the same workspace by index twice will do nothing (since you're already on that workspace).
If this flag is enabled, switching to the same workspace by index twice will switch back to the previous workspace.

Ymir will correctly switch to the workspace you came from, even if workspaces were reordered in the meantime.

```lua
return {
    input = {
        workspace_auto_back_and_forth = true,
    },
}
```

#### `mod_key`, `mod_key_nested`

<sup>Since: 25.05</sup>

Customize the `Mod` key for [key bindings](./Configuration:-Key-Bindings.md).
Only valid modifiers are allowed, e.g. `Super`, `Alt`, `Mod3`, `Mod5`, `Ctrl`, `Shift`.

By default, `Mod` is equal to `Super` when running ymir on a TTY, and to `Alt` when running ymir as a nested winit window.

> [!NOTE]
> There are a lot of default bindings with Mod, none of them "make it through" to the underlying window.
> You probably don't want to set `mod_key` to Ctrl or Shift, since Ctrl is commonly used for app hotkeys, and Shift is used for, well, regular typing.

```lua
return {
    input = {
        -- Switch the mod keys around: use Alt normally, and Super inside a nested window.
        mod_key = "Alt",
        mod_key_nested = "Super",
    },
}
```
