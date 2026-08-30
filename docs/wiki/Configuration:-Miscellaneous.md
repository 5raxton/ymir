This page documents all top-level options that don't otherwise have dedicated pages.

Here are all of these options at a glance:

```lua
return {
    spawn_at_startup = {
        { command = { "waybar" } },
        { command = { "alacritty" } },
    },
    spawn_sh_at_startup = {
        { command = "qs -c ~/source/qs/MyAwesomeShell" },
    },

    prefer_no_csd = true,

    screenshot_path = "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png",

    environment = {
        { name = "QT_QPA_PLATFORM", value = "wayland" },
        { name = "DISPLAY", value = ymir.null },
    },

    cursor = {
        xcursor_theme = "breeze_cursors",
        xcursor_size = 48,

        hide_when_typing = true,
        hide_after_inactive_ms = 1000,
    },

    overview = {
        zoom = 0.5,
        backdrop_color = "#262626",

        workspace_shadow = {
            -- off
            softness = 40,
            spread = 10,
            offset = { x = 0, y = 10 },
            color = "#00000050",
        },
    },

    xwayland_satellite = {
        -- off
        path = "xwayland-satellite",
    },

    clipboard = {
        disable_primary = true,
    },

    hotkey_overlay = {
        skip_at_startup = true,
        hide_not_bound = true,
    },

    config_notification = {
        disable_failed = true,
    },

    blur = {
        -- off
        passes = 3,
        offset = 3.0,
        noise = 0.02,
        saturation = 1.5,
    },
}
```

### `spawn-at-startup`

Add lines like this to spawn processes at ymir startup.

`spawn-at-startup` accepts a path to the program binary as the first argument, followed by arguments to the program.

This option works the same way as the [`spawn` key binding action](./Configuration:-Key-Bindings.md#spawn), so please read about all its subtleties there.

```lua
return {
    spawn_at_startup = {
        { command = { "waybar" } },
        { command = { "alacritty" } },
    },
}
```

Note that running ymir as a systemd session supports xdg-desktop-autostart out of the box, which may be more convenient to use.
Thanks to this, apps that you configured to autostart in GNOME will also "just work" in ymir, without any manual `spawn-at-startup` configuration.

### `spawn-sh-at-startup`

<sup>Since: 25.08</sup>

Add lines like this to run shell commands at ymir startup.

The argument is a single string that is passed verbatim to `sh`.
You can use shell variables, pipelines, `~` expansion and everything else as expected.

See detailed description in the docs for the [`spawn-sh` key binding action](./Configuration:-Key-Bindings.md#spawn-sh).

```lua
return {
    spawn_sh_at_startup = {
        -- Pass all arguments in the same string.
        { command = "qs -c ~/source/qs/MyAwesomeShell" },
    },
}
```

### `prefer-no-csd`

This flag will make ymir ask the applications to omit their client-side decorations.

If an application will specifically ask for CSD, the request will be honored.
Additionally, clients will be informed that they are tiled, removing some rounded corners.

With `prefer_no_csd` set, applications that negotiate server-side decorations through the xdg-decoration protocol will have focus ring and border drawn around them *without* a solid colored background.

> [!NOTE]
> Unlike most other options, changing `prefer_no_csd` will not entirely affect already running applications.
> It will make some windows rectangular, but won't remove the title bars.
> This mainly has to do with ymir working around a [bug in SDL2](https://github.com/libsdl-org/SDL/issues/8173) that prevents SDL2 applications from starting.
>
> Restart applications after changing `prefer_no_csd` in the config to fully apply it.

```lua
return {
    prefer_no_csd = true,
}
```

### `screenshot-path`

Set the path where screenshots are saved.
A `~` at the front will be expanded to the home directory.

The path is formatted with `strftime(3)` to give you the screenshot date and time.

Ymir will create the last folder of the path if it doesn't exist.

```lua
return {
    screenshot_path = "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png",
}
```

Note that in the Lua config `screenshot_path` only accepts a path string; there is currently no way to disable saving screenshots to disk from the config.

### `environment`

Override environment variables for processes spawned by ymir.

```lua
return {
    environment = {
        -- Set a variable like this:
        -- { name = "QT_QPA_PLATFORM", value = "wayland" },

        -- Remove a variable by using ymir.null as the value:
        -- { name = "DISPLAY", value = ymir.null },
    },
}
```

Note that these variables do not propagate to the systemd global environment, so tools and applications started by systemd do not see them.
In particular, if you start a desktop shell like DankMaterialShell through systemd, then use its built-in application launcher, the apps won't see these environment variables.

If you want all processes to see the environment variables, you can set them in your login shell config instead (i.e. `~/.bash_profile`).
The `ymir-session` shell script runs through the login shell and imports all environment variables to systemd before starting ymir.
Keep in mind that all compositors will see variables set in the login shell, not just ymir.

### `cursor`

Change the theme and size of the cursor as well as set the `XCURSOR_THEME` and `XCURSOR_SIZE` environment variables.

```lua
return {
    cursor = {
        xcursor_theme = "breeze_cursors",
        xcursor_size = 48,
    },
}
```

#### `hide-when-typing`

<sup>Since: 0.1.10</sup>

If set, hides the cursor when pressing a key on the keyboard.

> [!NOTE]
> This setting might interfere with games running in Wine in native Wayland mode that use mouselook, such as first-person games.
> If your character's point of view jumps down when you press a key and move the mouse simultaneously, try disabling this setting.

```lua
return {
    cursor = {
        hide_when_typing = true,
    },
}
```

#### `hide-after-inactive-ms`

<sup>Since: 0.1.10</sup>

If set, the cursor will automatically hide once this number of milliseconds passes since the last cursor movement.

```lua
return {
    cursor = {
        -- Hide the cursor after one second of inactivity.
        hide_after_inactive_ms = 1000,
    },
}
```

### `overview`

<sup>Since: 25.05</sup>

Settings for the [Overview](./Overview.md).

#### `zoom`

Control how much the workspaces zoom out in the overview.
`zoom` ranges from 0 to 0.75 where lower values make everything smaller.

```lua
return {
    overview = {
        -- Make workspaces four times smaller than normal in the overview.
        zoom = 0.25,
    },
}
```

#### `backdrop-color`

Set the backdrop color behind workspaces in the overview.
The backdrop is also visible between workspaces when switching.

The alpha channel for this color will be ignored.

```lua
return {
    overview = {
        -- Make the backdrop light.
        backdrop_color = "#777777",
    },
}
```

You can also set the color per-output [in the output config](./Configuration:-Outputs.md#backdrop-color).

#### `workspace-shadow`

Control the shadow behind workspaces visible in the overview.

Settings here mirror the normal [`shadow` config in the layout section](./Configuration:-Layout.md#shadow), so check the documentation there.

Workspace shadows are configured for a workspace size normalized to 1080 pixels tall, then zoomed out together with the workspace.
Practically, this means that you'll want bigger spread, offset, and softness compared to window shadows.

```lua
return {
    overview = {
        -- Disable workspace shadows in the overview.
        workspace_shadow = {
            off = true,
        },
    },
}
```

### `xwayland-satellite`

<sup>Since: 25.08</sup>

Settings for integration with [xwayland-satellite](https://github.com/Supreeeme/xwayland-satellite).

When a recent enough xwayland-satellite is detected, ymir will create the X11 sockets and set `DISPLAY`, then automatically spawn `xwayland-satellite` when an X11 client tries to connect.
If Xwayland dies, ymir will keep watching the X11 socket and restart `xwayland-satellite` as needed.
This is very similar to how built-in Xwayland works in other compositors.

`off` disables the integration: ymir won't create an X11 socket and won't set the `DISPLAY` environment variable.

`path` sets the path to the `xwayland-satellite` binary.
By default, it's just `xwayland-satellite`, so it's looked up like any other non-absolute program name.

```lua
return {
    xwayland_satellite = {
        -- Use a custom build of xwayland-satellite.
        path = "~/source/rs/xwayland-satellite/target/release/xwayland-satellite",
    },
}
```

### `clipboard`

<sup>Since: 25.02</sup>

Clipboard settings.

Set the `disable_primary` flag to disable the primary clipboard (middle-click paste).
Toggling this flag will only apply to applications started afterward.

```lua
return {
    clipboard = {
        disable_primary = true,
    },
}
```

### `hotkey-overlay`

Settings for the "Important Hotkeys" overlay.

#### `skip-at-startup`

Set the `skip_at_startup` flag if you don't want to see the hotkey help at ymir startup.

```lua
return {
    hotkey_overlay = {
        skip_at_startup = true,
    },
}
```

#### `hide-not-bound`

<sup>Since: 25.08</sup>

By default, ymir will show the most important actions even if they aren't bound to any key, to prevent confusion.
Set the `hide_not_bound` flag if you want to hide all actions not bound to any key.

```lua
return {
    hotkey_overlay = {
        hide_not_bound = true,
    },
}
```

You can customize which binds the hotkey overlay shows using the [`hotkey_overlay_title` property](./Configuration:-Key-Bindings.md#custom-hotkey-overlay-titles).

### `config-notification`

<sup>Since: 25.08</sup>

Settings for the config created/failed notification.

Set the `disable_failed` flag to disable the "Failed to parse the config file" notification.
For example, if you have a custom one.

```lua
return {
    config_notification = {
        disable_failed = true,
    },
}
```

### `blur`

<sup>Since: 1.0.0</sup>

Blur configuration that affects all background blur.

See the [window effects page](./Window-Effects.md) for an overview of background effects.

```lua
return {
    -- These are the default values:
    blur = {
        -- off
        passes = 3,
        offset = 3.0,
        noise = 0.02,
        saturation = 1.5,
    },
}
```

#### `off`

By default, blur is available on request by a window or layer surface (via the `ext-background-effect` protocol).
You can also enable it manually with the `blur` background effect [window](./Configuration:-Window-Rules.md#background-effect) or [layer](./Configuration:-Layer-Rules.md#background-effect) rule.

Setting the `off` flag will disable all blur, both requested by the window, and configured in window rules.

```lua
return {
    blur = {
        off = true,
    },
}
```

#### `passes` and `offset`

`passes` controls the number of downsample/upsample passes for dual kawase blur.
More passes produce a larger, smoother blur, but cost more GPU resources.

`offset` is the pixel offset multiplier for each pass.
Offset `1` is the original dual kawase blur.
Larger values produce a smoother blur, at no additional GPU cost.

However, setting `offset` too big will produce visual artifacts.
You will need to increase `passes` to be able to use a bigger `offset` without artifacts.

When configuring blur, try increasing `offset` first (since it doesn't cause any extra GPU load) until you start getting artifacts.
Then, if you still need smoother blur, increase `passes` by 1.
Keep doing this until you get the desired visuals. 

```lua
return {
    blur = {
        passes = 3,
        offset = 3.0,
    },
}
```

#### `noise`

Amount of noise to add on top of the blur.

This is helpful to reduce color banding artifacts.

```lua
return {
    blur = {
        noise = 0.02,
    },
}
```

#### `saturation`

Color saturation applied to the blurred background.

Values above `1` increase saturation; values below `1` reduce it.

```lua
return {
    blur = {
        saturation = 1.5,
    },
}
```
