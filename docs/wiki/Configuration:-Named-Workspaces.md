### Overview

<sup>Since: 0.1.6</sup>

You can declare named workspaces at the top level of the config:

```kdl
workspace "browser"

workspace "chat" {
    open-on-output "Some Company CoolMonitor 1234"
}
```

Contrary to normal dynamic workspaces, named workspaces always exist, even when they have no windows.
Otherwise, they behave like any other workspace: you can move them around, move to a different monitor, and so on.

Actions like `focus-workspace` or `move-column-to-workspace` can refer to workspaces by name.
Also, you can use an `open-on-workspace` window rule to make a window open on a specific named workspace:

```kdl
// Declare a workspace named "chat" that opens on the "DP-2" output.
workspace "chat" {
    open-on-output "DP-2"
}

// Open Fractal on the "chat" workspace, if it runs at ymir startup.
window-rule {
    match at-startup=true app-id=r#"^org\.gnome\.Fractal$"#
    open-on-workspace "chat"
}
```

Named workspaces initially appear in the order they are declared in the config file.
When editing the config while ymir is running, newly declared named workspaces will appear at the very top of a monitor.

If you delete some named workspace from the config, the workspace will become normal (unnamed), and if there are no windows on it, it will be removed (as any other normal workspace).
There's no way to give a name to an already existing workspace, but you can simply move windows that you want to a new, empty named workspace.

<sup>Since: 0.1.9</sup> `open-on-output` can now use monitor manufacturer, model, and serial.
Before, it could only use the connector name.

<sup>Since: 25.01</sup> You can use `set-workspace-name` and `unset-workspace-name` actions to change workspace names dynamically.

<sup>Since: 25.02</sup> Named workspaces no longer update/forget their original output when opening a new window on them (unnamed workspaces will keep doing that).
This means that named workspaces "stick" to their original output in more cases, reflecting their more permanent nature.
Explicitly moving a named workspace to a different monitor will still update its original output.

### Layout config overrides

<sup>Since: 25.11</sup>

You can customize layout settings for named workspaces with a `layout {}` block:

```kdl
workspace "aesthetic" {
    // Layout config overrides just for this named workspace.
    layout {
        gaps 32

        struts {
            left 64
            right 64
            bottom 64
            top 64
        }

        border {
            on
            width 4
        }

        // ...any other setting.
    }
}
```

It accepts all the same options as [the top-level `layout {}` block](./Configuration:-Layout.md), except:

- `empty-workspace-above-first`: this is an output-level setting, doesn't make sense on a workspace.
- `insert-hint`: currently we always draw these at the output level, so it's not customizable per-workspace.

In order to unset a flag, write it with `false`, e.g.:

```kdl
layout {
    // Enabled globally.
    always-center-single-column
}

workspace "uncentered" {
    layout {
        // Unset on this workspace.
        always-center-single-column false
    }
}
```

### Non-creating layout tags for default workspaces

<sup>Since: 25.12</sup>

A workspace entry whose name is a plain number is a **non-creating layout rule**: instead of
creating a named workspace, it applies its `layout {}` block to the Nth *default* (unnamed)
workspace of every monitor, matching how you address workspaces with `focus-workspace N`.

```kdl
// The first default workspace of every monitor runs dwindle; nothing is created.
workspace "1" {
    layout {
        default-column-display "dwindle"
    }
}

// The second default workspace gets wider gaps.
workspace "2" {
    layout {
        gaps 32
    }
}
```

Rules like these never create a workspace called `"1"` or `"2"`. They apply to unnamed workspaces
by their 1-based position (default workspace number), and leave named workspaces alone. `open-on-output`,
if present on such an entry, is ignored.

Named workspaces configured with a *non-numeric* name (like `"browser"`) are unaffected: they keep
creating a real named workspace as described above.
