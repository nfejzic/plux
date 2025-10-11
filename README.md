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

#### From crates.io

You can install plux with using [cargo](https://doc.rust-lang.org/cargo/):

```bash
cargo install plux
```

#### From source

Alternatively, you can build and install from source:

```bash
# Clone the repository
git clone https://github.com/nfejzic/plux.git
cd plux

# Build in release mode
cargo build --release

# Link the binary to your PATH
ln -sf $PWD/target/release/plux /usr/local/bin/plux
```

#### Configuration

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

### Plugin installation process

Plugin installation consists of three phases:

1. **Download** - Clones the plugin repository using `git clone --depth 1` for efficiency
2. **Version selection** - Fetches tags and checks out the requested version (tag, branch, or commit)
3. **Execution** - Loads the plugin into tmux using one of two methods (see compatibility below)

### TPM compatibility

Plux maintains backward compatibility with [TPM](https://github.com/tmux-plugins/tpm) plugins while providing enhanced functionality for plux-aware plugins. Plugin execution works as follows:

#### Execution modes:

- **Plux-aware plugins**: If a `plux.tmux` file exists in the plugin's root directory, it is sourced directly via `tmux source-file <path>/plux.tmux`. This allows plugins to set tmux options and bindings in the current tmux context.

- **TPM-compatible plugins**: If no `plux.tmux` file exists, all `*.tmux` files in the plugin directory are executed in the background via `tmux run-shell -b <path>/*.tmux`. This matches TPM behavior and works with existing TPM plugins.

#### Migration from TPM:

Existing TPM users can switch to plux with minimal changes:
1. Install plux and add `run-shell plux` to your `~/.tmux.conf`
2. Convert your TPM plugin list to `plux.toml` format (see Plugin specification above)
3. Remove TPM from your configuration
4. Reload tmux - your existing plugins should work unchanged
