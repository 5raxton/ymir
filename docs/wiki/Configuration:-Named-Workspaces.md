### Overview

<sup>Since: 1.0.0</sup>

You can declare named workspaces at the top level of the config:

```lua
return {
    workspaces = {
        { name = "browser" },
        { name = "chat", open_on_output = "Some Company CoolMonitor 1234" },
    },
}
```

Contrary to normal dynamic workspaces, named workspaces always exist, even when they have no windows.
Otherwise, they behave like any other workspace: you can move them around, move to a different monitor, and so on.

Actions like `focus_workspace` or `move_column_to_workspace` can refer to workspaces by name.
Also, you can use an `open_on_workspace` window rule to make a window open on a specific named workspace:

```lua
return {
    workspaces = {
        -- Declare a workspace named "chat" that opens on the "DP-2" output.
        { name = "chat", open_on_output = "DP-2" },
    },

    -- Open Fractal on the "chat" workspace, if it runs at ymir startup.
    window_rules = {
        {
            match = { { at_startup = true, app_id = "^org\\.gnome\\.Fractal$" } },
            open_on_workspace = "chat",
        },
    },
}
```

Named workspaces initially appear in the order they are declared in the config file.
When editing the config while ymir is running, newly declared named workspaces will appear at the very top of a monitor.

If you delete some named workspace from the config, the workspace will become normal (unnamed), and if there are no windows on it, it will be removed (as any other normal workspace).
There's no way to give a name to an already existing workspace, but you can simply move windows that you want to a new, empty named workspace.

<sup>Since: 1.0.0</sup> `open_on_output` can now use monitor manufacturer, model, and serial.
Before, it could only use the connector name.

<sup>Since: 1.0.0</sup> You can use `set_workspace_name` and `unset_workspace_name` actions to change workspace names dynamically.

<sup>Since: 1.0.0</sup> Named workspaces no longer update/forget their original output when opening a new window on them (unnamed workspaces will keep doing that).
This means that named workspaces "stick" to their original output in more cases, reflecting their more permanent nature.
Explicitly moving a named workspace to a different monitor will still update its original output.

### Layout config overrides

<sup>Since: 1.0.0</sup>

You can customize layout settings for named workspaces with a `layout` table:

```lua
return {
    workspaces = {
        {
            name = "aesthetic",
            -- Layout config overrides just for this named workspace.
            layout = {
                gaps = 32,

                struts = {
                    left = 64,
                    right = 64,
                    bottom = 64,
                    top = 64,
                },

                border = {
                    on = true,
                    width = 4,
                },

                -- ...any other setting.
            },
        },
    },
}
```

It accepts all the same options as [the top-level `layout` table](./Configuration:-Layout.md), except:

- `empty_workspace_above_first`: this is an output-level setting, doesn't make sense on a workspace.
- `insert_hint`: currently we always draw these at the output level, so it's not customizable per-workspace.

In order to unset a flag, write it with `false`, e.g.:

```lua
return {
    layout = {
        -- Enabled globally.
        always_center_single_column = true,
    },

    workspaces = {
        {
            name = "uncentered",
            layout = {
                -- Unset on this workspace.
                always_center_single_column = false,
            },
        },
    },
}
```

### Non-creating layout tags for default workspaces

<sup>Since: 1.0.0</sup>

A workspace entry whose name is a plain number is a **non-creating layout rule**: instead of
creating a named workspace, it applies its `layout` table to the Nth *default* (unnamed)
workspace of every monitor, matching how you address workspaces with `focus_workspace N`.

```lua
return {
    workspaces = {
        -- The first default workspace of every monitor runs dwindle; nothing is created.
        { name = "1", layout = { default_column_display = "dwindle" } },

        -- The second default workspace gets wider gaps.
        { name = "2", layout = { gaps = 32 } },
    },
}
```

Rules like these never create a workspace called `"1"` or `"2"`. They apply to unnamed workspaces
by their 1-based position (default workspace number), and leave named workspaces alone. `open_on_output`,
if present on such an entry, is ignored.

Named workspaces configured with a *non-numeric* name (like `"browser"`) are unaffected: they keep
creating a real named workspace as described above.
