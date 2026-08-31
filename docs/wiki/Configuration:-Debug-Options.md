### Overview

Ymir has several options that are only useful for debugging, or are experimental and have known issues.
They are not meant for normal use.

> [!CAUTION]
> These options are **not** covered by the [config breaking change policy](./Configuration:-Introduction.md#breaking-change-policy).
> They can change or stop working at any point with little notice.

Here are all the options at a glance:

```lua
return {
    debug = {
        preview_render = "screencast",
        -- preview_render = "screen-capture",
        enable_overlay_planes = true,
        disable_cursor_plane = true,
        disable_direct_scanout = true,
        restrict_primary_scanout_to_matching_format = true,
        force_disable_connectors_on_resume = true,
        render_drm_device = "/dev/dri/renderD129",
        ignored_drm_devices = { "/dev/dri/renderD128", "/dev/dri/renderD130" },
        force_pipewire_invalid_modifier = true,
        dbus_interfaces_in_non_session_instances = true,
        wait_for_frame_completion_before_queueing = true,
        emulate_zero_presentation_time = true,
        disable_resize_throttling = true,
        disable_transactions = true,
        keep_laptop_panel_on_when_lid_is_closed = true,
        disable_monitor_names = true,
        strict_new_window_focus_policy = true,
        honor_xdg_activation_with_invalid_serial = true,
        skip_cursor_only_updates_during_vrr = true,
        deactivate_unfocused_windows = true,
        disable_10bit_output = true,
    },

    binds = {
        { key = "Mod+Shift+Ctrl+T", action = { name = "toggle_debug_tint" } },
        { key = "Mod+Shift+Ctrl+O", action = { name = "debug_toggle_opaque_regions" } },
        { key = "Mod+Shift+Ctrl+D", action = { name = "debug_toggle_damage" } },
    },
}
```

### `preview-render`

Make ymir render the monitors the same way as for a screencast or a screen capture.

Useful for previewing the `block-out-from` window rule.

```lua
return {
    debug = {
        preview_render = "screencast",
        -- preview_render = "screen-capture",
    },
}
```

### `enable-overlay-planes`

Enable direct scanout into overlay planes.
May cause frame drops during some animations on some hardware (which is why it is not the default).

Direct scanout into the primary plane is always enabled.

```lua
return {
    debug = {
        enable_overlay_planes = true,
    },
}
```

### `disable-cursor-plane`

Disable the use of the cursor plane.
The cursor will be rendered together with the rest of the frame.

Useful to work around driver bugs on specific hardware.

```lua
return {
    debug = {
        disable_cursor_plane = true,
    },
}
```

### `disable-direct-scanout`

Disable direct scanout to both the primary plane and the overlay planes.

```lua
return {
    debug = {
        disable_direct_scanout = true,
    },
}
```

### `restrict-primary-scanout-to-matching-format`

Restricts direct scanout to the primary plane to when the window buffer exactly matches the composition swapchain format.

This flag may prevent unexpected bandwidth changes when going between composition and scanout.
The plan is to make it default in the future, when we implement a way to tell the clients the composition swapchain format.
As is, it may prevent some clients (mpv on my machine) from scanning out to the primary plane.

```lua
return {
    debug = {
        restrict_primary_scanout_to_matching_format = true,
    },
}
```

### `force-disable-connectors-on-resume`

<sup>Since: 1.0.0</sup>

Force-disables all outputs upon resuming ymir (TTY switch or waking up from suspend).
This causes a modeset/screen blank on all outputs.

If ymir rendering is corrupted, or monitors don't light up after a TTY switch, you can try this flag.

```lua
return {
    debug = {
        force_disable_connectors_on_resume = true,
    },
}
```

### `render-drm-device`

Override the DRM device that ymir will use for all rendering.

You can set this to make ymir use a different primary GPU than the default one.

```lua
return {
    debug = {
        render_drm_device = "/dev/dri/renderD129",
    },
}
```

### `ignore-drm-device`

<sup>Since: 1.0.0</sup>

List DRM devices that ymir will ignore.
Useful for GPU passthrough when you don't want ymir to open a certain device.

```lua
return {
    debug = {
        ignored_drm_devices = { "/dev/dri/renderD128", "/dev/dri/renderD130" },
    },
}
```

### `force-pipewire-invalid-modifier`

<sup>Since: 1.0.0</sup>

Forces PipeWire screencasting to use the invalid modifier, even when DRM offers more modifiers.

Useful for testing the invalid modifier code path that is hit by drivers that don't support modifiers.

```lua
return {
    debug = {
        force_pipewire_invalid_modifier = true,
    },
}
```

### `dbus-interfaces-in-non-session-instances`

Make ymir create its D-Bus interfaces even if it's not running as a `--session`.

Useful for testing screencasting changes without having to relogin.

The main ymir instance will *not* currently take back the interfaces when you close the test instance, so you will need to relogin in the end to make screencasting work again.

```lua
return {
    debug = {
        dbus_interfaces_in_non_session_instances = true,
    },
}
```

### `wait-for-frame-completion-before-queueing`

Wait until every frame is done rendering before handing it over to DRM.

Useful for diagnosing certain synchronization and performance problems.

```lua
return {
    debug = {
        wait_for_frame_completion_before_queueing = true,
    },
}
```

### `emulate-zero-presentation-time`

Emulate zero (unknown) presentation time returned from DRM.

This is a thing on NVIDIA proprietary drivers, so this flag can be used to test that ymir doesn't break too hard on those systems.

```lua
return {
    debug = {
        emulate_zero_presentation_time = true,
    },
}
```

### `disable-resize-throttling`

<sup>Since: 1.0.0</sup>

Disable throttling resize events sent to windows.

By default, when resizing quickly (e.g. interactively), a window will only receive the next size once it has made a commit for the previously requested size.
This is required for resize transactions to work properly, and it also helps certain clients which don't batch incoming resizes from the compositor.

Disabling resize throttling will send resizes to windows as fast as possible, which is potentially very fast (for example, on a 1000 Hz mouse).

```lua
return {
    debug = {
        disable_resize_throttling = true,
    },
}
```

### `disable-transactions`

<sup>Since: 1.0.0</sup>

Disable transactions (resize and close).

By default, windows which must resize together, do resize together.
For example, all windows in a column must resize at the same time to maintain the combined column height equal to the screen height, and to maintain the same window width.

Transactions make ymir wait until all windows finish resizing before showing them all on screen in one, synchronized frame.
For them to work properly, resize throttling shouldn't be disabled (with the previous debug flag).

```lua
return {
    debug = {
        disable_transactions = true,
    },
}
```

### `keep-laptop-panel-on-when-lid-is-closed`

<sup>Since: 1.0.0</sup>

By default, ymir will disable the internal laptop monitor when the laptop lid is closed.
This flag turns off this behavior and will leave the internal laptop monitor on.

```lua
return {
    debug = {
        keep_laptop_panel_on_when_lid_is_closed = true,
    },
}
```

### `disable-monitor-names`

<sup>Since: 1.0.0</sup>

Disables the make/model/serial monitor names, as if ymir fails to read them from the EDID.

Use this flag to work around a crash when connecting two monitors with matching make/model/serial.

```lua
return {
    debug = {
        disable_monitor_names = true,
    },
}
```

### `strict-new-window-focus-policy`

<sup>Since: 1.0.0</sup>

Disables heuristic automatic focusing for new windows.
Only windows that activate themselves with a valid xdg-activation token will be focused.

```lua
return {
    debug = {
        strict_new_window_focus_policy = true,
    },
}
```

### `honor-xdg-activation-with-invalid-serial`

<sup>Since: 1.0.0</sup>

Widely-used clients such as Discord and Telegram make fresh xdg-activation tokens upon clicking on their tray icon or on their notification.
Most of the time, these fresh tokens will have invalid serials, because the app needs to be focused to get a valid serial, and if the user clicks on a tray icon or a notification, it is usually because the app *isn't* focused, and the user wants to focus it.

By default, ymir ignores xdg-activation tokens with invalid serials, to prevent windows from randomly stealing focus.
This debug flag makes ymir honor such tokens, making the aforementioned widely-used apps get focus when clicking on their tray icon or notification.

Use the [`on-xdg-activate` window rule](./Configuration:-Window-Rules.md#on-xdg-activate) to control what ymir does for individual windows when it accepts an xdg-activation request.

Amusingly, clicking on a notification sends the app a perfectly valid activation token from the notification daemon, but these apps seem to simply ignore it.
Maybe in the future these apps/toolkits (Electron, Qt) are fixed, making this debug flag unnecessary.

```lua
return {
    debug = {
        honor_xdg_activation_with_invalid_serial = true,
    },
}
```

### `skip-cursor-only-updates-during-vrr`

<sup>Since: 1.0.0</sup>

Skips redrawing the screen from cursor input while variable refresh rate is active.

Useful for games where the cursor isn't drawn internally to prevent erratic VRR shifts in response to cursor movement.

Note that the current implementation has some issues, for example when there's nothing redrawing the screen (like a game), the rendering will appear to completely freeze (since cursor movements won't cause redraws).

```lua
return {
    debug = {
        skip_cursor_only_updates_during_vrr = true,
    },
}
```

### `deactivate-unfocused-windows`

<sup>Since: 1.0.0</sup>

Some clients (notably, Chromium- and Electron-based, like Teams or Slack) erroneously use the Activated xdg window state instead of keyboard focus for things like deciding whether to send notifications for new messages, or for picking where to show an IME popup.
Ymir keeps the Activated state on unfocused workspaces and invisible tiled windows (to reduce unwanted animations), surfacing bugs in these applications.

Set this debug flag to work around these problems.
It will cause ymir to drop the Activated state for all unfocused windows.

```lua
return {
    debug = {
        deactivate_unfocused_windows = true,
    },
}
```

### `disable-10bit-output`

<sup>Since: 1.0.0</sup>

By default, ymir will try to output a 10-bit color format to the monitor (before falling back to 8-bit).
However, this can currently cause problems on some Intel + NVIDIA mixed-GPU setups: the screen doesn't light up, or displays only white, etc.

Until this is fixed in Smithay, you can disable 10-bit color formats by setting this debug flag.

```lua
return {
    debug = {
        disable_10bit_output = true,
    },
}
```

### Key Bindings

These are not debug options, but rather key bindings.

#### `toggle-debug-tint`

Tints all surfaces green, unless they are being directly scanned out.

Useful to check if direct scanout is working.

```lua
return {
    binds = {
        { key = "Mod+Shift+Ctrl+T", action = { name = "toggle_debug_tint" } },
    },
}
```

#### `debug-toggle-opaque-regions`

<sup>Since: 1.0.0</sup>

Tints regions marked as opaque with blue and the rest of the render elements with red.

Useful to check how Wayland surfaces and internal render elements mark their parts as opaque, which is a rendering performance optimization.

```lua
return {
    binds = {
        { key = "Mod+Shift+Ctrl+O", action = { name = "debug_toggle_opaque_regions" } },
    },
}
```

#### `debug-toggle-damage`

<sup>Since: 1.0.0</sup>

Tints damaged regions with red.

```lua
return {
    binds = {
        { key = "Mod+Shift+Ctrl+D", action = { name = "debug_toggle_damage" } },
    },
}
```
