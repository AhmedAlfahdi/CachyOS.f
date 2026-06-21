# cachyos-dotfiles

**Backup and restore CachyOS + KDE Plasma dotfiles via GitHub — with per-file granularity.**

Two components:
- **`cachyos-dotfiles`** — Python CLI (single file, zero dependencies) for backup/restore/git operations
- **`cachyos-dotfiles-tui`** — Rust terminal UI (ratatui + crossterm) for interactive browsing, toggling, and operations with animated feedback

Default manifest tracks **79 files** across system configs, KDE Plasma 6, CachyOS-specific, and user dotfiles. Every single file can be individually enabled or disabled.

---

## Install

### TUI (recommended)

```bash
cargo install cachyos-dotfiles-tui
```

That's it. One command, no cloning, no building manually. Requires [Rust](https://rustup.rs).

### CLI script

```bash
# Clone the repo (needed for the Python CLI and manifest)
git clone https://github.com/YOU/CachyOS.f.git
cd CachyOS.f
chmod +x cachyos-dotfiles
```

---

## Quick Start

### First-time setup

```bash
# 1. Install the TUI
cargo install cachyos-dotfiles-tui

# 2. Clone the repo for the CLI and manifest
git clone https://github.com/YOU/CachyOS.f.git
cd CachyOS.f
chmod +x cachyos-dotfiles

# 3. Run the wizard (auto-starts if no config found)
cachyos-dotfiles-tui --wizard
```

The wizard will:
1. Check GitHub authentication (`gh auth status`)
2. Ask: backup or restore?
3. If backup: guide through creating a GitHub repo (or use existing)
4. Run `cachyos-dotfiles init` to set up manifest + clone repo
5. Offer to run an immediate backup
6. Launch the TUI

### Already set up?

```bash
cachyos-dotfiles-tui          # launch the TUI
cachyos-dotfiles backup        # backup via CLI
cachyos-dotfiles restore        # restore via CLI
```

---

## The TUI (`cachyos-dotfiles-tui`)

A keyboard-driven terminal interface built with [ratatui](https://ratatui.rs) + [crossterm](https://github.com/crossterm-rs/crossterm). Compiled to a single binary — no runtime dependencies.

### Layout

```
┌── cachyos-dotfiles ───────────────────────────────────────────────────┐
│ All │ System │ KDE │ CachyOS │ User │           🔍 filter text        │
├───────────────────────────────────────┬────────────────────────────────┤
│ Files (58/76)                         │ Details                       │
│                                       │                                │
│ ▶ ✓ /etc/pacman.conf  [sudo]         │ Path: /etc/pacman.conf         │
│   ✓ /etc/makepkg.conf [sudo]         │ Category: system               │
│   ✓ /etc/paru.conf     [sudo]        │ Status: ✓ Enabled              │
│   ✗ /etc/sddm.conf     [sudo]        │ Needs sudo: Yes                │
│   ✓ ~/.config/kdeglobals             │ ...                            │
│   ✓ ~/.config/kwinrc                 │                                │
│   ...                                │ ── preview ──                  │
│                                       │ [options]                      │
│                                       │ ...(file contents)             │
├───────────────────────────────────────┴────────────────────────────────┤
│ [clean]  Space:toggle  b:backup  r:restore  d:diff  h:help  q:quit  │
└──────────────────────────────────────────────────────────────────────┘
```

### Keyboard shortcuts

| Key | Action |
|---|---|
| `↑` / `↓` / `j` / `k` | Move cursor |
| `Space` / `Enter` | Toggle enabled/disabled for selected file |
| `1` `2` `3` `4` | Jump to category: System / KDE / CachyOS / User |
| `a` | Show all categories |
| `Tab` / `Shift+Tab` | Next / previous category |
| `!` `@` `#` `$` | Batch toggle all files in System / KDE / CachyOS / User |
| `/` | Filter files (type to search, `Esc` to clear) |
| `b` | **Backup** — spawns background thread, shows animated spinner, then result dialog |
| `r` | **Restore all** — dry-run in background, confirm dialog, then restore with spinner |
| `R` | **Restore selected** — copy single file from repo to system |
| `d` | **Diff** selected file vs repo copy |
| `h` | Help popup |
| `q` | Quit |

### Color legend

| Color | Meaning |
|---|---|
| 🟢 Green | Enabled file |
| 🔴 Red | Disabled file |
| 🟡 Yellow | Needs sudo (system files in `/etc/`) |
| 🟣 Magenta | Security-sensitive (SSH keys, wallet config) |

### Git repo status

The status bar shows the current state of your local dotfiles repo (at `~/.local/share/cachyos-dotfiles/repo/`):

```
[clean]    — no uncommitted changes
[3Δ]       — 3 changed files since last backup
[no repo]  — repo not initialized
```

### Progress feedback

When you press `b` (backup) or `r` (restore), the right panel switches to an animated working indicator:

```
┌ Working ────────────────────┐
│ ⠋ Backing up...            │
│                             │
│ Running... (please wait)    │
└─────────────────────────────┘
```

The spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) animates for a minimum of 500ms to ensure visibility, then a result dialog appears with `✓ Done` or `✗ Failed` plus the full command output.

---

## The Python CLI (`cachyos-dotfiles`)

A single-file Python script (stdlib only — no pip packages). All operations are also available directly from the command line.

### Commands

| Command | Description |
|---|---|
| `init [--repo URL]` | Create config directory + default manifest, clone GitHub repo |
| `backup` | Copy enabled files → git commit → git push |
| `restore [--dry-run] [--yes]` | Pull repo → back up existing files → write to system |
| `list [--category X]` | Show all tracked files with enabled/disabled status |
| `enable <path>` | Enable a file for tracking (fuzzy match on filename or path) |
| `disable <path>` | Disable a file (skipped during backup/restore) |
| `diff [path]` | Show system vs repo differences |
| `status` | Git status of the local repo |
| `pkglist export\|import` | Export/import explicitly installed packages |

### Examples

```bash
# List all KDE files
./cachyos-dotfiles list --category kde

# Disable sensitive files
./cachyos-dotfiles disable kwalletrc
./cachyos-dotfiles disable .ssh/config
./cachyos-dotfiles disable kdeconnect

# Run a backup
./cachyos-dotfiles backup

# Dry-run a restore first
./cachyos-dotfiles restore --dry-run

# Actually restore (with confirmation prompt)
./cachyos-dotfiles restore

# Skip confirmation
./cachyos-dotfiles restore --yes

# Export package list
./cachyos-dotfiles pkglist export
```

---

## What's Tracked

The default manifest includes **79 files** across 4 categories. Every file can be individually toggled.

### System (10 files)

`/etc/pacman.conf`, `/etc/makepkg.conf`, `/etc/paru.conf`, `/etc/mkinitcpio.conf`, `/etc/locale.conf`, `/etc/vconsole.conf`, `/etc/environment`, `/etc/hostname`, `/etc/sddm.conf` (disabled by default), `/etc/default/grub` (disabled by default)

These are copied with `sudo` during restore.

### KDE Plasma 6 (47 files)

Everything that defines your desktop environment:

**Core:** `kdeglobals` (theme, colors, fonts, icons, cursor), `kwinrc` (window manager, effects, tiling), `kwinoutputconfig.json` (monitor layout, resolution, scale), `kwinrulesrc` (window-specific rules), `kglobalshortcutsrc` (all keyboard shortcuts), `plasma-org.kde.plasma.desktop-appletsrc` (panel/widget layout — critical for panel restore), `plasmarc` (wallpaper paths), `plasmashellrc`, `plasma-localerc`, `plasmanotifyrc`, `ksmserverrc` (login/logout behavior), `kded6rc` (daemon services)

**Apps:** `dolphinrc` (file manager), `konsolerc` (terminal profiles), `katerc` (text editor), `kcminputrc` (mouse/touchpad/keyboard), `klaunchrc`, `kxkbrc` (keyboard layout), `baloofilerc` (file indexing), `powermanagementprofilesrc` (suspend, brightness), `systemsettingsrc`, `spectaclerc` (screenshot tool), `okularrc`, `arkrc` (archive manager)

**Network/Bluetooth:** `plasma-nm` (NetworkManager), `bluedevilglobalrc` (disabled by default)

**GTK bridges:** `gtk-3.0/settings.ini`, `gtk-4.0/settings.ini` (for cross-toolkit consistency)

**Sensitive (disabled by default):** `kwalletrc`, `kdeconnect/`, `KDE/` (legacy)

### CachyOS-specific (2 files)

`~/.config/cachyos/` (directory), `~/.config/cachyos-hello.json` (Hello application state)

### User (20 files)

**Shell:** `.bashrc`, `~/.config/fish/`

**Terminal emulators:** `~/.config/kitty/`, `~/.config/alacritty/`, `~/.config/ghostty/`

**Editors:** `~/.config/nvim/` (Neovim + LazyVim), `~/.config/nvim/lazy-lock.json` (plugin version lock), `~/.config/micro/` (disabled)

**System monitors:** `~/.config/btop/`

**Version control:** `.gitconfig`

**Desktop:** `~/.config/user-dirs.dirs`, `~/.config/mimeapps.list`, `~/.config/fontconfig/`

**Security-sensitive (disabled by default):** `~/.ssh/config`, `~/.ssh/known_hosts`

---

## How It Works

### Backup flow

1. Read `~/.config/cachyos-dotfiles/manifest.json` — get all tracked files
2. Filter to only `enabled: true` entries
3. Copy each enabled file into the local git repo at `~/.local/share/cachyos-dotfiles/repo/`
   - User files (`~/.config/...`) → `repo/home/.config/...`
   - System files (`/etc/...`) → `repo/root/etc/...`
4. Export `pacman -Qqe` (explicitly installed packages) into `pkglist.txt`
5. Copy manifest.json into repo for reference
6. `git add -A` → `git commit` with timestamp + hostname → `git push origin`

### Restore flow

1. Pull latest from GitHub
2. For each enabled file:
   - Back up existing system file to `~/.config/cachyos-dotfiles/backups/<timestamp>/`
   - Copy from repo to system path
   - System files (`/etc/*`) use `sudo cp`
3. Offer to install packages from `pkglist.txt` via `sudo pacman -S --needed -`

### No secrets leak

- `~/.ssh/` is disabled by default
- `kwalletrc` is disabled by default (doesn't contain wallet data, but disabled for paranoia)
- `kdeconnect/` is disabled by default (contains device IDs)
- Review the manifest with `list` before your first backup

---

## File Locations

| Item | Path |
|---|---|
| **TUI binary** (after install) | `~/.cargo/bin/cachyos-dotfiles-tui` |
| **CLI script** | wherever you cloned the repo, e.g. `~/CachyOS.f/cachyos-dotfiles` |
| **Config** | `~/.config/cachyos-dotfiles/config.json` |
| **Manifest** | `~/.config/cachyos-dotfiles/manifest.json` |
| **Local git repo** | `~/.local/share/cachyos-dotfiles/repo/` |
| **Restore backups** | `~/.config/cachyos-dotfiles/backups/<timestamp>/` |

---

## Requirements

### Python CLI
- Python 3.8+ (stdlib only — no pip packages)
- `git`
- `pacman` (for package list export, Arch/CachyOS only)
- `gh` CLI (for GitHub auth — uses HTTPS, no SSH keys needed)

### Rust TUI
- Rust toolchain ([rustup](https://rustup.rs))
- Install: `cargo install cachyos-dotfiles-tui`
- Binary has zero runtime dependencies

---

## Build from Source (for contributors)

```bash
git clone https://github.com/YOU/CachyOS.f.git
cd CachyOS.f/cachyos-dotfiles-tui-rs

# Build and run directly
cargo build --release
./target/release/cachyos-dotfiles-tui

# Or install from local source
cargo install --path .
```

---

## Tips

- **After restoring KDE configs**, restart Plasma: `systemctl --user restart plasma-plasmashell` or log out and back in
- **Wallpaper paths** in `plasmarc` may reference external drives — update paths after restore if needed
- **Monitor layout** (`kwinoutputconfig.json`) contains display UUIDs that change between installs — KWin usually handles this gracefully
- **Run backup regularly** before system updates: press `b` in the TUI or run `./cachyos-dotfiles backup`
- **Add to cron/systemd timer** for automated backups: `0 20 * * * cd ~/CachyOS.f && ./cachyos-dotfiles backup`
- **No sudo needed for backup** — system files in `/etc/` are world-readable. Sudo is only needed during restore.
- **The TUI looks for the CLI** in three places: same directory, `~/CachyOS.f/cachyos-dotfiles`, or `./cachyos-dotfiles`

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| "Manifest not found" | Run `cachyos-dotfiles-tui --wizard` or `./cachyos-dotfiles init` |
| "CLI: No such file" | The TUI can't find the Python script. Run from the project root (`~/CachyOS.f/`) |
| TUI doesn't start | Terminal must be ≥ 24 rows. Try `export TERM=xterm-256color` |
| "Permission denied (publickey)" | Use HTTPS instead of SSH. Run `gh auth login` for HTTPS auth |
| Backup hangs | First backup may be slow if pushing a large initial commit. The spinner shows it's working |
| Restore asks for password | System files need sudo. The prompt appears in the terminal behind the TUI — check there |
