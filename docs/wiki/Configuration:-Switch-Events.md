### Overview

<sup>Since: 1.0.0</sup>

Switch event bindings are declared in the `switch_events` section of the config.

Here are all the events that you can bind at a glance:

```lua
return {
    switch_events = {
        lid_close = { spawn = { "notify-send", "The laptop lid is closed!" } },
        lid_open = { spawn = { "notify-send", "The laptop lid is open!" } },
        tablet_mode_on = { spawn = { "bash", "-c", "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled true" } },
        tablet_mode_off = { spawn = { "bash", "-c", "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled false" } },
    },
}
```

The syntax is similar to key bindings.
Currently, only the [`spawn` action](./Configuration:-Key-Bindings.md#spawn) are supported.

> [!NOTE]
> In contrast to key bindings, switch event bindings are *always* executed, even when the session is locked.

### `lid-close`, `lid-open`

These events correspond to closing and opening of the laptop lid.

Note that ymir will already automatically turn the internal laptop monitor on and off in accordance with the laptop lid.

```lua
return {
    switch_events = {
        lid_close = { spawn = { "notify-send", "The laptop lid is closed!" } },
        lid_open = { spawn = { "notify-send", "The laptop lid is open!" } },
    },
}
```

### `tablet-mode-on`, `tablet-mode-off`

These events trigger when a convertible laptop goes into or out of tablet mode.
In tablet mode, the keyboard and mouse are usually inaccessible, so you can use these events to activate the on-screen keyboard.

> [!NOTE]
> The commands below are just examples, you will need to provide your own on-screen keyboard, such as [sysboard](https://github.com/System64fumo/sysboard) or [wvkbd](https://github.com/jjsullivan5196/wvkbd).

```lua
return {
    switch_events = {
        tablet_mode_on = { spawn = { "bash", "-c", "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled true" } },
        tablet_mode_off = { spawn = { "bash", "-c", "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled false" } },
    },
}
```