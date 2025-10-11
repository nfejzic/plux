# PLUX

Minimal tmux plugin management, powered by Rust.
This workspace also includes **murus**, a small Rust API wrapping a few tmux commands.

## Quick Start

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

### Setup

1. Add this line to your `~/.tmux.conf`:

```bash
run-shell plux
```

2. Reload tmux:

```bash
tmux source-file ~/.tmux.conf
```

3. On first run, plux auto-creates:
   - `~/.config/tmux/plux.toml` - plugin specification file
   - `~/.config/tmux/plux/` - plugins directory

4. Edit `~/.config/tmux/plux.toml` to add your plugins (see below), then reload tmux again.

## Plugin Specification

Add plugins to `~/.config/tmux/plux.toml`:

```toml
[plugins]
# Simple: plugin name = GitHub URL (uses default branch)
tmux-ssh-split = "https://github.com/pschmitt/tmux-ssh-split"

# With specific version:
tmux-sensible = { url = "https://github.com/tmux-plugins/tmux-sensible", tag = "v2.0.3" }
tmux-fingers = { url = "https://github.com/Morantron/tmux-fingers", branch = "feature-xyz" }
some-plugin = { url = "https://github.com/user/repo", commit = "<commit-hash>" }
```

## Configuration

### Custom Paths

Override default paths in your `~/.tmux.conf`:

```bash
set -g @plux_toml_path "~/custom/path/plux.toml"
set -g @plux_plugins_path "~/custom/path/plugins/"
```

**Defaults:**
- Config: `~/.config/tmux/plux.toml`
- Plugins: `~/.config/tmux/plux/`

## TPM Migration

Switching from [TPM](https://github.com/tmux-plugins/tpm):

1. Install plux (see above)
2. Replace `run-shell '~/.tmux/plugins/tpm/tpm'` with `run-shell plux` in `~/.tmux.conf`
3. Convert plugin list to `plux.toml` format
4. Remove TPM
5. Reload tmux

**Compatibility:** Plux works with existing TPM plugins. If a plugin provides `plux.tmux`, it's sourced via `source-file`; otherwise all `*.tmux` files are executed via `run-shell -b`.

## How It Works

Plux manages plugins in three steps:

1. **Clone** - Downloads plugins using `git clone --depth 1`
2. **Version** - Checks out specified tag/branch/commit
3. **Load** - Sources/executes plugin files in tmux

---

