<sup>Since: 25.11</sup>

You can include other files at the top level of the config.

```lua
-- Some settings...

include_config("examples/colors.lua")

-- Some more settings...
return {}
```

Included files have the same structure as the main config file.
Settings from included files will be merged with the settings from the main config file.

Included config files can in turn include more files.
All included files are watched for changes, and the config live-reloads when any of them change.

You can include by filename or path.

* Relative to the current file: `other.lua` or `./other.lua`
* By absolute path: `/path/to/file.lua`
* <sup>Since: 26.04</sup> Home dir paths: `~/file.lua` expands to `/home/user/file.lua`

Includes work only at the top level of the config, as function calls before the returned table:

```lua,must-fail
include_config("examples/colors.lua")

return {
    layout = {
        -- NOT allowed: include inside some other section.
        include = "examples/colors.lua",
    },
}
```

### Merging

Settings from included files are merged with the settings from the main config file.
The whole config (every included file plus the main file) shares a single set of `ymir` sections,
so a **singular section** (like `layout` or `overview`) may only be declared once across the entire
config program — that is, once in total across the main file and all included files.
Put each singular section in whichever file is most convenient, and refer to it from there.

For example, this file provides its color settings:

```lua
-- examples/colors.lua
return {
    layout = {
        -- Does not affect gaps, border width, etc.
        -- Only changes colors as written.
        border = {
            active_color = "green",
        },

        focus_ring = {
            active_color = "blue",
        },
    },

    overview = {
        backdrop_color = "green",
    },
}
```

You can include it, using the settings it provides, and still declare other sections
(that don't collide) in the main config:

```lua
include_config("examples/colors.lua")

return {
    spawn_at_startup = {
        { command = { "waybar" } },
    },
}
```

The end result contains both `colors.lua`'s settings and the waybar startup command.

Declaring a singular section that an include already provided is an error:

```lua,must-fail
include_config("examples/colors.lua")

return {
    overview = {
        backdrop_color = "red",
    },
}
```

`overview` was already declared by `colors.lua`, so this would be a duplicate section.

#### Multipart sections

Multipart sections like `window_rules`, `output`, `workspaces`, or the startup commands are exempt from
the "declared once" rule. Each entry you write in any file is added, in order:

```lua
-- examples/laptop.lua
return {
    output = {
        { name = "eDP-1" },
    },
}
```

```lua
include_config("examples/laptop.lua")

return {
    output = {
        { name = "DP-2" },
    },
}

-- End result: both the eDP-1 (from laptop.lua) and DP-2 outputs are present.
```

#### Positionality

Entries from included files are inserted before the main config's entries.
This matters for multipart sections that are matched in order, like `window_rules`:

```lua
-- examples/rules.lua
return {
    window_rules = {
        {
            match = { { app_id = "Alacritty" } },
            open_maximized = false,
        },
    },
}
```

```lua
include_config("examples/rules.lua")

return {
    window_rules = {
        { open_maximized = true },
        {
            match = { { app_id = "firefox$" } },
            open_maximized = true,
        },
    },
}
```

Window rules get inserted in order: the included ones come first, followed by the main config's rules.
This is equivalent to the following config file:

```lua
return {
    window_rules = {
        -- Included from rules.lua.
        {
            match = { { app_id = "Alacritty" } },
            open_maximized = false,
        },

        { open_maximized = true },

        {
            match = { { app_id = "firefox$" } },
            open_maximized = true,
        },
    },
}
```

### Optional includes

<sup>Since: 26.04</sup>

By default, including a nonexistent file will cause an error.
You can allow nonexistent includes by passing an options table to `include_config` with `optional = true`:

```lua,must-fail
-- Won't fail if this file doesn't exist.
include_config("examples/missing-optional.lua", { optional = true })

-- Regular include, will fail if the file doesn't exist.
include_config("examples/missing-required.lua")
```

When an optional include file is missing, ymir will emit a warning in the logs on every config reload.
This reminds you that the file is missing while still loading the config successfully.

The optional file is still watched for changes, so if you create it later, the config will automatically reload and apply the new settings.

Note that `optional` only affects whether a missing file causes an error.
If the file exists but contains invalid syntax or other errors, those errors will still cause a parsing failure.

### Binds

`binds` is a singular section, so keep it in one file.
Within a `binds` table, a later bind with the same key overrides an earlier one:

```lua
return {
    binds = {
        { key = "Mod+T", action = { name = "spawn", command = { "alacritty" } } },

        -- Overrides the Mod+T bind above.
        { key = "Mod+T", action = { name = "spawn", command = { "foot" } } },
    },
}
```

### Flags

Most flags are `true`/`false` booleans and can be disabled with `false`:

```lua
return {
    -- Write "false" to explicitly disable prefer-no-csd.
    prefer_no_csd = false,
}
```

### Non-merging sections

Some sections describe a single combined structure rather than a list of values, for example
`struts` and `preset_column_widths` inside `layout`, individual subsections in `animations`,
and pointing device sections in `input`.
Because their containing section is singular, these can't be split across the main config and an include —
declare them in one file:

```lua
-- examples/struts.lua
return {
    layout = {
        struts = {
            left = 64,
            right = 64,
        },
    },
}
```

If you moved this to an included file, remove it from the main config so the `layout` section is still
declared only once.

### Border special case

There's one special case that differs between the main config and included configs.

Writing `layout = { border = {} }` in an included config does nothing (since no properties are changed).
However, writing the same in the main config will *enable* the border, i.e. it's equivalent to `layout = { border = { on = true } }`.

So, if you want to move your layout configuration from the main config to a separate file, remember to add `on = true` to the border table, for example:

```lua
-- examples/separate.lua
return {
    layout = {
        border = {
            -- Add this line:
            on = true,

            width = 4,
            active_color = "#ffc87f",
            inactive_color = "#505050",
        },
    },
}
```

The reason for this special case is that this is how it historically worked: back when I added borders, we didn't have any `on` flags, so I made writing the `border` section enable the border, with an explicit `off` to disable it.
It wouldn't be too problematic to change it, however the default config always had a pre-filled `layout = { border = { off = true } }` table with a note saying that commenting out the `off` is enough to enable the border.
Many people likely have this part of the default config embedded in their configs now, so changing how it works would just cause a lot of confusion.
