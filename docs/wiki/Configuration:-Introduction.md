### Per-Section Documentation

You can find documentation for various sections of the config on these wiki pages:

* [`input`](./Configuration:-Input.md)
* [`output`](./Configuration:-Outputs.md)
* [`binds`](./Configuration:-Key-Bindings.md)
* [`switch_events`](./Configuration:-Switch-Events.md)
* [`layout`](./Configuration:-Layout.md)
* [top-level options](./Configuration:-Miscellaneous.md)
* [`window_rules`](./Configuration:-Window-Rules.md)
* [`layer_rules`](./Configuration:-Layer-Rules.md)
* [`animations`](./Configuration:-Animations.md)
* [`gestures`](./Configuration:-Gestures.md)
* [`recent_windows`](./Configuration:-Recent-Windows.md)
* [`debug`](./Configuration:-Debug-Options.md)
* [`include_config`](./Configuration:-Include.md)

### Loading

Ymir will load configuration from `$XDG_CONFIG_HOME/ymir/config.lua` or `~/.config/ymir/config.lua`, falling back to `/etc/ymir/config.lua`.
If both of these files are missing, ymir will create `$XDG_CONFIG_HOME/ymir/config.lua` with the contents of [the default configuration file](https://github.com/5raxton/ymir/blob/main/resources/default-config.lua), which are embedded into the ymir binary at build time.
Please use the default configuration file as the starting point for your custom configuration.

The configuration is live-reloaded.
Simply edit and save the config file, and your changes will be applied.
This includes key bindings, output settings like mode, window rules, and everything else.

You can run `ymir validate` to parse the config and see any errors.

To use a different config file path, pass it in the `--config` or `-c` argument to `ymir`.

You can also set `$YMIR_CONFIG` to the path of the config file.
`--config` always takes precedence.
If `--config` or `$YMIR_CONFIG` doesn't point to a real file, the config will not be loaded.
If `$YMIR_CONFIG` is set to an empty string, it is ignored and the default config location is used instead.

### Syntax

The config is a Lua file that returns a single table describing the settings.
Each section is a Lua table, and keys are `snake_case` (kebab-case is also accepted).
For example:

```lua
return {
    input = {
        keyboard = { repeat_delay = 600 },
    },
}
```

#### Comments

Lines starting with `--` are comments; they are ignored.

You can also comment out an entire section by commenting out every line of it:

```lua
return {
    -- output = {
    --     -- Everything inside here is ignored.
    --     -- The display won't be turned off
    --     -- as the whole section is commented out.
    --     { name = "eDP-1", off = true },
    -- },
}
```

#### Flags

Toggle options in ymir are represented as `true`/`false` values.
Setting the key to `true` enables it, and omitting it (or setting it to `false`) disables it.
For example:

```lua
return {
    input = {
        -- "Focus follows mouse" is enabled.
        focus_follows_mouse = true,

        -- Other settings...
    },
}
```

```lua
return {
    input = {
        -- "Focus follows mouse" is commented out, so it is disabled.
        -- focus_follows_mouse = true,

        -- Other settings...
    },
}
```

#### Sections

Most sections cannot be repeated. For example:

```lua
return {
    input = {
        keyboard = {
            -- ...
        },

        touchpad = {
            -- ...
        },
    },
}
```

```lua,must-fail
-- This is NOT valid: the input section appears twice.
ymir.input {
    keyboard = {
        -- ...
    },
}

ymir.input {
    touchpad = {
        -- ...
    },
}
```

Exceptions are, for example, sections that configure different devices by name:

<!-- NOTE: this may break in the future -->
```lua
return {
    output = {
        { name = "eDP-1" },

        -- This is valid: this entry configures a different output.
        { name = "HDMI-A-1" },

        -- This is NOT valid: "eDP-1" already appeared above.
        -- It will either throw a config parsing error, or otherwise not work.
        { name = "eDP-1" },
    },
}
```

### Defaults

Omitting most of the sections of the config file will leave you with the default values for that section.
A notable exception is [`binds`](./Configuration:-Key-Bindings.md): they do not get filled with defaults, so make sure you do not erase this section.

### Breaking Change Policy

As a rule, ymir updates should not break existing config files.
(For example, the default config from ymir v0.1.0 still parses fine on v25.02 as I'm writing this.)

Exceptions can be made for parsing bugs.
For example, ymir used to accept multiple binds to the same key, but this was not intended and did not do anything (the first bind was always used).
A patch release changed ymir from silently accepting this to causing a parsing failure.
This is not a blanket rule, I will consider the potential impact of every breaking change like this before deciding to carry on with it.

Keep in mind that the breaking change policy applies only to ymir releases.
Commits between releases can and do occasionally break the config as new features are ironed out.
However, I do try to limit these, since several people are running git builds.
