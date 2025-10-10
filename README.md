# Tmux API and plugin manager

This workspace contains Rust crate that wraps tmux commands and a simple plugin
manager for tmux.

## Plux - the plugin manager

Plux is a very simple tmux plugin manager. Aim is to provide modern plugin
management. To use the plugin manager, specify `plux.toml` file with desired
plugins, and add `run-shell plux` to your tmux configuration.

Plux needs to know two paths to do the work:

- `@plux_toml_path` - where the `plux.toml` file is located. By default the
  `$HOME/.config/tmux/plux.toml` is used.
- `@plux_plugins_path` - directory that will contain installed plugins. By
  default the `$HOME/.config/tmux/plux/` directory is used.

### Installation

You can install plux with using [cargo](https://doc.rust-lang.org/cargo/):

`cargo install plux`

After installation, add this line to your `~/.tmux.conf`:

```
run-shell plux
```

Then reload your tmux configuration with `tmux source-file ~/.tmux.conf`.

On first run, plux will automatically:
- Create `~/.config/tmux/` directory if it doesn't exist
- Create `~/.config/tmux/plux/` plugins directory if it doesn't exist
- Create `~/.config/tmux/plux.toml` with example configuration if it doesn't exist

After that, edit `~/.config/tmux/plux.toml` to add your desired plugins and reload tmux.

### Plugin specification

Plugins are specified in a single `plux.toml` file, with following syntax:

```toml
[plugins]
# you can specify the url of a plugin directly, default git branch will be used
# (e.g. 'main')
tmux-ssh-split = "https://github.com/pschmitt/tmux-ssh-split"

# or you can specify full plugin spec
# with branch:
tmux-fingers = { rul = "https://github.com/Morantron/tmux-fingers", branch = "feature-xyz" }
# with tag:
smart-splits = { url = "https://github.com/mrjones2014/smart-splits.nvim", tag = "v2.0.3"}
# with commit hash:
tmux-sensible = { url = "https://github.com/tmux-plugins/tmux-sensible", commit = "<commit hash>"}
```

### Plux compatibility

There are multiple parts to plugin installation:

1. Download plugin - The first part is basically just downloading plugin files.
   This is done with `git clone` command.
2. Use version - The second part is determining what version needs to be used
   and switching to that version.
3. Starting the plugin - Plux retains a little bit of backwards compatibility
   with [tpm](https://github.com/tmux-plugins/tpm), full compatibility is not
   guaranteed. From my (limited) understanding, tpm simply runs all `.tmux`
   files in plugins directory. Plux does that as well, with one exception. If
   plugin contains a top level `plux_start.tmux` file, then this file is sourced
   from tmux (e.g. `tmux source <path-to-plugin>/plux_start.tmux`). Otherwise
   all `.tmux` files are executed (e.g. `tmux run-shell <path-to-file>.tmux`).
