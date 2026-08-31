### Overview

Key bindings are declared in the `binds` list of the config.

> [!NOTE]
> This is one of the few sections that *does not* get automatically filled with defaults if you omit it, so make sure to copy it from the default config.

Each bind maps a hotkey to one action.
For example:

```lua
return {
    binds = {
        { key = "Mod+Left", action = { name = "focus_column_left" } },
        { key = "Super+Alt+L", action = { name = "spawn", command = { "swaylock" } } },
    },
}
```

The hotkey consists of modifiers separated by `+` signs, followed by an XKB key name in the end.

Valid modifiers are:

- `Ctrl` or `Control`;
- `Shift`;
- `Alt`;
- `Super` or `Win`;
- `ISO_Level3_Shift` or `Mod5`—this is the AltGr key on certain layouts;
- `ISO_Level5_Shift`: can be used with an xkb lv5 option like `lv5:caps_switch`;
- `Mod`.

`Mod` is a special modifier that is equal to `Super` when running ymir on a TTY, and to `Alt` when running ymir as a nested winit window.
This way, you can test ymir in a window without causing too many conflicts with the host compositor's key bindings.
For this reason, most of the default keys use the `Mod` modifier.

<sup>Since: 1.0.0</sup> You can customize the `Mod` key [in the `input` section of the config](./Configuration:-Input.md#mod_key-mod_key_nested).

> [!TIP]
> To find an XKB name for a particular key, you may use a program like [`wev`](https://git.sr.ht/~sircmpwn/wev).
>
> Open it from a terminal and press the key that you want to detect.
> In the terminal, you will see output like this:
>
> ```
> [14:     wl_keyboard] key: serial: 757775; time: 44940343; key: 113; state: 1 (pressed)
>                       sym: Left         (65361), utf8: ''
> [14:     wl_keyboard] key: serial: 757776; time: 44940432; key: 113; state: 0 (released)
>                       sym: Left         (65361), utf8: ''
> [14:     wl_keyboard] key: serial: 757777; time: 44940753; key: 114; state: 1 (pressed)
>                       sym: Right        (65363), utf8: ''
> [14:     wl_keyboard] key: serial: 757778; time: 44940846; key: 114; state: 0 (released)
>                       sym: Right        (65363), utf8: ''
> ```
>
> Here, look at `sym: Left` and `sym: Right`: these are the key names.
> I was pressing the left and the right arrow in this example.
>
> Keep in mind that binding shifted keys requires spelling out Shift and the unshifted version of the key, according to your XKB layout.
> For example, on the US QWERTY layout, <kbd>&lt;</kbd> is on <kbd>Shift</kbd> + <kbd>,</kbd>, so to bind it, you spell out something like `Mod+Shift+Comma`.
>
> As another example, if you've configured the French [BÉPO](https://en.wikipedia.org/wiki/B%C3%89PO) XKB layout, your <kbd>&lt;</kbd> is on <kbd>AltGr</kbd> + <kbd>«</kbd>.
> <kbd>AltGr</kbd> is `ISO_Level3_Shift`, or equivalently `Mod5`, so to bind it, you spell out something like `Mod+Mod5+guillemotleft`.
>
> When resolving latin keys, ymir will search for the *first* configured XKB layout that has the latin key.
> So for example with US QWERTY and RU layouts configured, US QWERTY will be used for latin binds.

<sup>Since: 1.0.0</sup> Binds will repeat by default (i.e. holding down a bind will make it trigger repeatedly).
You can disable that for specific binds with `["repeat"] = false`:

```lua
return {
    binds = {
        { key = "Mod+T", ["repeat"] = false, action = { name = "spawn", command = { "alacritty" } } },
    },
}
```

Binds can also have a cooldown, which will rate-limit the bind and prevent it from repeatedly triggering too quickly.

```lua
return {
    binds = {
        { key = "Mod+T", cooldown_ms = 500, action = { name = "spawn", command = { "alacritty" } } },
    },
}
```

This is mostly useful for the scroll bindings.

### Scroll Bindings

You can bind mouse wheel scroll ticks using the following syntax.
These binds will change direction based on the `natural_scroll` setting.

```lua
return {
    binds = {
        { key = "Mod+WheelScrollDown", cooldown_ms = 150, action = { name = "focus_workspace_down" } },
        { key = "Mod+WheelScrollUp",   cooldown_ms = 150, action = { name = "focus_workspace_up" } },
        { key = "Mod+WheelScrollRight", action = { name = "focus_column_right" } },
        { key = "Mod+WheelScrollLeft",  action = { name = "focus_column_left" } },
    },
}
```

Similarly, you can bind touchpad scroll "ticks".
Touchpad scrolling is continuous, so for these binds it is split into discrete intervals based on distance travelled.

These binds are also affected by touchpad's `natural_scroll`, so these example binds are "inverted", since ymir has `natural_scroll` enabled for touchpads by default.

```lua
return {
    binds = {
        { key = "Mod+TouchpadScrollDown", action = { name = "spawn", command = { "wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "0.02+" } } },
        { key = "Mod+TouchpadScrollUp",   action = { name = "spawn", command = { "wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "0.02-" } } },
    },
}
```

Both mouse wheel and touchpad scroll binds will prevent applications from receiving any scroll events when their modifiers are held down.
For example, if you have a `Mod+WheelScrollDown` bind, then while holding `Mod`, all mouse wheel scrolling will be consumed by ymir.

### Mouse Click Bindings

<sup>Since: 1.0.0</sup>

You can bind mouse clicks using the following syntax.

```lua
return {
    binds = {
        { key = "Mod+MouseLeft",    action = { name = "close_window" } },
        { key = "Mod+MouseRight",   action = { name = "close_window" } },
        { key = "Mod+MouseMiddle",  action = { name = "close_window" } },
        { key = "Mod+MouseForward", action = { name = "close_window" } },
        { key = "Mod+MouseBack",    action = { name = "close_window" } },
    },
}
```

Mouse clicks operate on the window that was focused at the time of the click, not the window you're clicking.

Note that binding `Mod+MouseLeft` or `Mod+MouseRight` will override the corresponding gesture (moving or resizing the window).

### Custom Hotkey Overlay Titles

<sup>Since: 1.0.0</sup>

The hotkey overlay (the Important Hotkeys dialog) shows a hardcoded list of binds.
You can customize this list using the `hotkey_overlay_title` property.

To add a bind to the hotkey overlay, set the property to the title that you want to show:
```lua
return {
    binds = {
        { key = "Mod+Shift+S", hotkey_overlay_title = "Toggle Dark/Light Style", action = { name = "spawn", command = { "some-script.sh" } } },
    },
}
```

Binds with custom titles are listed after the hardcoded binds and before non-customized Spawn binds.

To remove a hardcoded bind from the hotkey overlay, set the property to `ymir.null`:
```lua
return {
    binds = {
        { key = "Mod+Q", hotkey_overlay_title = ymir.null, action = { name = "close_window" } },
    },
}
```

> [!TIP]
> When multiple key combinations are bound to the same action:
> - If any of the binds has a custom hotkey overlay title, ymir will show that bind.
> - Otherwise, if any of the binds has a null title, ymir will hide the bind.
> - Otherwise, ymir will show the first key combination.

Custom titles support [Pango markup](https://docs.gtk.org/Pango/pango_markup.html):

```lua
return {
    binds = {
        { key = "Mod+Shift+S", hotkey_overlay_title = "<b>Toggle</b> <span foreground='red'>Dark</span>/Light Style", action = { name = "spawn", command = { "some-script.sh" } } },
    },
}
```

![Custom markup example.](https://github.com/user-attachments/assets/2a2ba914-bfa7-4dfa-bb5e-49839034765d)

### Actions

Every action that you can bind is also available for programmatic invocation via `ymir msg action`.
Run `ymir msg action` to get a full list of actions along with their short descriptions.

Here are a few actions that benefit from more explanation.

#### `spawn`

Run a program.

`spawn` accepts a path to the program binary as the first argument, followed by arguments to the program.
For example:

```lua
return {
    binds = {
        -- Run alacritty.
        { key = "Mod+T", action = { name = "spawn", command = { "alacritty" } } },

        -- Run `wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.1+`.
        { key = "XF86AudioRaiseVolume", action = { name = "spawn", command = { "wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "0.1+" } } },
    },
}
```

> [!TIP]
>
> <sup>Since: 1.0.0</sup>
>
> Spawn bindings have a special `allow_when_locked = true` property that makes them work even while the session is locked:
>
> ```lua
> return {
>     binds = {
>         -- This mute bind will work even when the session is locked.
>         { key = "XF86AudioMute", allow_when_locked = true, action = { name = "spawn", command = { "wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "toggle" } } },
>     },
> }
> ```

For `spawn`, ymir *does not* use a shell to run commands, which means that you need to manually separate arguments.
See [`spawn-sh`](#spawn-sh) below for an action that uses a shell.

```lua
return {
    binds = {
        -- Correct: every argument is in its own quotes.
        { key = "Mod+T", action = { name = "spawn", command = { "alacritty", "-e", "/usr/bin/fish" } } },

        -- Wrong: will interpret the whole `alacritty -e /usr/bin/fish` string as the binary path.
        { key = "Mod+D", action = { name = "spawn", command = { "alacritty -e /usr/bin/fish" } } },

        -- Wrong: will pass `-e /usr/bin/fish` as one argument, which alacritty won't understand.
        { key = "Mod+Q", action = { name = "spawn", command = { "alacritty", "-e /usr/bin/fish" } } },
    },
}
```

This also means that you cannot expand environment variables or `~`.
If you need this, you can run the command through a shell manually.

```lua
return {
    binds = {
        -- Wrong: no shell expansion here. These strings will be passed literally to the program.
        { key = "Mod+T", action = { name = "spawn", command = { "grim", "-o", "$MAIN_OUTPUT", "~/screenshot.png" } } },

        -- Correct: run this through a shell manually so that it can expand the arguments.
        -- Note that the entire command is passed as a SINGLE argument,
        -- because the shell will do its own argument splitting by whitespace.
        { key = "Mod+D", action = { name = "spawn", command = { "sh", "-c", "grim -o $MAIN_OUTPUT ~/screenshot.png" } } },

        -- You can also use a shell to run multiple commands,
        -- use pipes, process substitution, and so on.
        { key = "Mod+Q", action = { name = "spawn", command = { "sh", "-c", "notify-send clipboard \"$(wl-paste)\"" } } },
    },
}
```

As a special case, ymir will expand `~` to the home directory *only* at the beginning of the program name.

```lua
return {
    binds = {
        -- This will work: one ~ at the very beginning.
        { key = "Mod+T", action = { name = "spawn", command = { "~/scripts/do-something.sh" } } },
    },
}
```

#### `spawn-sh`

<sup>Since: 1.0.0</sup>

Run a command through the shell.

The argument is a single string that is passed verbatim to `sh`.
You can use shell variables, pipelines, `~` expansion, and everything else as expected.

```lua
return {
    binds = {
        -- Works with spawn_sh: all arguments in the same string.
        { key = "Mod+D", action = { name = "spawn_sh", command = "alacritty -e /usr/bin/fish" } },

        -- Works with spawn_sh: shell variable ($MAIN_OUTPUT), ~ expansion.
        { key = "Mod+T", action = { name = "spawn_sh", command = "grim -o $MAIN_OUTPUT ~/screenshot.png" } },

        -- Works with spawn_sh: process substitution.
        { key = "Mod+Q", action = { name = "spawn_sh", command = "notify-send clipboard \"$(wl-paste)\"" } },

        -- Works with spawn_sh: multiple commands.
        { key = "Super+Alt+S", action = { name = "spawn_sh", command = "pkill orca || exec orca" } },
    },
}
```

`spawn-sh "some command"` is equivalent to `spawn "sh" "-c" "some command"`—it's just a less confusing shorthand.
Keep in mind that going through the shell incurs a tiny performance penalty compared to directly `spawn`ing some binary.

Using `sh` is hardcoded, consistent with other compositors.
If you want a different shell, write it out using `spawn`, e.g. `spawn "fish" "-c" "some fish command"`.

#### `quit`

Exit ymir after showing a confirmation dialog to avoid accidentally triggering it.

```lua
return {
    binds = {
        { key = "Mod+Shift+E", action = { name = "quit" } },
    },
}
```

If you want to skip the confirmation dialog, set the flag like so:

```lua
return {
    binds = {
        { key = "Mod+Shift+E", action = { name = "quit", skip_confirmation = true } },
    },
}
```

#### `do-screen-transition`

<sup>Since: 1.0.0</sup>

Freeze the screen for a brief moment then crossfade to the new contents.

```lua
return {
    binds = {
        { key = "Mod+Return", action = { name = "do_screen_transition" } },
    },
}
```

This action is mainly useful to trigger from scripts changing the system theme or style (between light and dark for example).
It makes transitions like this, where windows change their style one by one, look smooth and synchronized.

For example, using the GNOME color scheme setting:

```shell
ymir msg action do-screen-transition
dconf write /org/gnome/desktop/interface/color-scheme "\"prefer-dark\""
```

By default, the screen is frozen for 250 ms to give windows time to redraw, before the crossfade.
You can set this delay like this:

```lua
return {
    binds = {
        { key = "Mod+Return", action = { name = "do_screen_transition", delay_ms = 100 } },
    },
}
```

Or, in scripts:

```shell
ymir msg action do-screen-transition --delay-ms 100
```

#### `toggle-window-rule-opacity`

<sup>Since: 1.0.0</sup>

Toggle the opacity window rule of the focused window.
This only has an effect if the window's opacity window rule is already set to semitransparent.

```lua
return {
    binds = {
        { key = "Mod+O", action = { name = "toggle_window_rule_opacity" } },
    },
}
```

#### `screenshot`, `screenshot-screen`, `screenshot-window`

Actions for taking screenshots.

- `screenshot`: opens the built-in interactive screenshot UI.
- `screenshot_screen`, `screenshot_window`: takes a screenshot of the focused screen or window respectively.

The screenshot is both stored to the clipboard and saved to disk, according to the [`screenshot_path` option](./Configuration:-Miscellaneous.md#screenshot-path).

In the Lua config there is no per-bind way to disable saving to disk or to control the pointer; these are managed globally (saving is governed by `screenshot_path`) and inside the interactive screenshot UI (where pressing <kbd>Ctrl</kbd><kbd>C</kbd> copies to the clipboard without writing to disk, and pressing <kbd>P</kbd> toggles the pointer):

```lua
return {
    binds = {
        { key = "Ctrl+Print", action = { name = "screenshot_screen" } },
        { key = "Alt+Print",  action = { name = "screenshot_window" } },
    },
}
```

In the interactive screenshot UI, pressing <kbd>Ctrl</kbd><kbd>C</kbd> will copy the screenshot to the clipboard without writing it to disk.

The pointer is hidden by default on screenshots (you can still show it by pressing <kbd>P</kbd> in the interactive UI):

```lua
return {
    binds = {
        -- The pointer will be hidden by default
        -- (you can still show it by pressing P).
        { key = "Print", action = { name = "screenshot" } },

        -- The pointer will be hidden on the screenshot.
        { key = "Ctrl+Print", action = { name = "screenshot_screen" } },
    },
}
```

On window screenshots the pointer is only included if the window is currently receiving pointer input (usually this means the pointer is on top of the window):

```lua
return {
    binds = {
        -- The pointer will be visible on the screenshot
        -- if it's on top of the window.
        { key = "Alt+Print", action = { name = "screenshot_window" } },
    },
}
```

#### `toggle-keyboard-shortcuts-inhibit`

<sup>Since: 1.0.0</sup>

Applications such as remote-desktop clients and software KVM switches may request that ymir stops processing its keyboard shortcuts so that they may, for example, forward the key presses as-is to a remote machine.
`toggle_keyboard_shortcuts_inhibit` is an escape hatch that toggles the inhibitor.
It's a good idea to bind it, so a buggy application can't hold your session hostage.

```lua
return {
    binds = {
        { key = "Mod+Escape", action = { name = "toggle_keyboard_shortcuts_inhibit" } },
    },
}
```

You can also make certain binds ignore inhibiting with the `allow_inhibiting = false` property.
They will always be handled by ymir and never passed to the window.

```lua
return {
    binds = {
        -- This bind will always work, even when using a virtual machine.
        { key = "Super+Alt+L", allow_inhibiting = false, action = { name = "spawn", command = { "swaylock" } } },
    },
}
```

#### Dwindle actions

The dwindle layout mode adds its own actions: `switch_column_display`, `set_column_display`, `toggle_split`, `preselect`, `promote_window`, `move_window_left`/`move_window_right`, `consume_window_into_column`, `expel_window_from_column` and `swap_window_right`/`swap_window_left`.
The default config binds them to `Mod+Shift+D`, `Mod+Space`, `Mod+Ctrl+Space`, `Mod+Shift+Home`, `Mod+Shift+Left`/`Right`, `Mod+Comma` and `Mod+Period`.
See the [Dwindle](./Configuration:-Dwindle.md) page for the full documentation and examples.
