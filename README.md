# cachyos-dotfiles

**Backup and restore CachyOS + KDE Plasma dotfiles via GitHub — with per-file granularity.**

A single-file Python CLI that tracks all your CachyOS system configs, KDE Plasma 6 settings, and user dotfiles. Push them to a private GitHub repo, then pull them onto a clean install.

## Quick start

```bash
# 1. Make it executable
chmod +x cachyos-dotfiles

# 2. Create a private repo on GitHub (e.g. "cachyos-dotfiles")

# 3. Initialize (clones the repo + writes default manifest)
./cachyos-dotfiles init --repo git@github.com:YOU/cachyos-dotfiles.git

# 4. Review what's tracked
./cachyos-dotfiles list

# 5. Disable anything you don't want
./cachyos-dotfiles disable kwalletrc
./cachyos-dotfiles disable .ssh/config

# 6. Backup now
./cachyos-dotfiles backup
```

## On a fresh CachyOS install

```bash
# 1. Install git and the tool
sudo pacman -S git
curl -O https://raw.githubusercontent.com/YOU/cachyos-dotfiles/main/cachyos-dotfiles
chmod +x cachyos-dotfiles

# 2. Initialize with your repo
./cachyos-dotfiles init --repo git@github.com:YOU/cachyos-dotfiles.git

# 3. See what will be restored (dry run)
./cachyos-dotfiles restore --dry-run

# 4. Restore everything
./cachyos-dotfiles restore
```

## Commands

| Command | What it does |
|---|---|
| `init [--repo URL]` | Create config + manifest, clone GitHub repo |
| `backup` | Copy enabled files → git commit → push to GitHub |
| `restore [--dry-run] [--yes]` | Pull repo → backup existing files → apply to system |
| `list [--category X]` | Show all tracked files with enabled/disabled status |
| `enable <path>` | Enable a file for tracking |
| `disable <path>` | Disable a file (skip during backup/restore) |
| `diff [path]` | Show differences between system files and repo copies |
| `status` | Show git status of the local repo |
| `pkglist export\|import` | Export/import explicitly installed packages |

## What's tracked

The default manifest includes **76 files** across 4 categories:

### System (10 files)
`/etc/pacman.conf`, `/etc/makepkg.conf`, `/etc/paru.conf`, `/etc/mkinitcpio.conf`, `/etc/locale.conf`, `/etc/vconsole.conf`, `/etc/environment`, `/etc/hostname`, `/etc/sddm.conf`, `/etc/default/grub`

System files are restored with `sudo`. SDDM and GRUB are **disabled by default**.

### KDE Plasma 6 (47 files)
Everything that defines your desktop: `kdeglobals`, `kwinrc`, `kwinoutputconfig.json`, `kglobalshortcutsrc`, `plasma-org.kde.plasma.desktop-appletsrc` (panel/widget layout), `plasmarc` (wallpapers), `dolphinrc`, `konsolerc`, `katerc`, GTK theme bridges, and more.

Sensitive items like `kwalletrc` and `kdeconnect/` are **disabled by default**.

### CachyOS-specific (2 files)
`~/.config/cachyos/` and `~/.config/cachyos-hello.json`.

### User (17 files)
Shell configs (`.bashrc`, `fish/`), terminal emulators (`kitty/`, `alacritty/`, `ghostty/`), editors (`nvim/`), `.gitconfig`, XDG user dirs, MIME defaults, fontconfig.

`~/.ssh/*` is **never enabled by default** for security.

## Per-file control

Every single file can be toggled independently:

```bash
# Disable files you don't want
./cachyos-dotfiles disable kwalletrc
./cachyos-dotfiles disable bluedevilglobalrc
./cachyos-dotfiles disable .ssh/config

# Re-enable later
./cachyos-dotfiles enable kwinrc

# Filter by category
./cachyos-dotfiles list --category kde
./cachyos-dotfiles list --category system
```

Disabled files are completely skipped during backup and restore.

## How it works

**Backup flow:**
1. Reads the manifest — only processes files with `enabled: true`
2. Copies each enabled file into the local git repo (at `~/.local/share/cachyos-dotfiles/repo/`)
3. Repo structure: `home/.config/foo` for user files, `root/etc/foo` for system files
4. Exports `pacman -Qqe` package list into `pkglist.txt`
5. Copies the manifest itself into the repo for reference
6. `git add -A`, commits with timestamp + hostname, pushes to origin

**Restore flow:**
1. Pulls latest from GitHub
2. Uses repo manifest if available (keeps local enable/disable state)
3. For each enabled file: backs up existing system file to `~/.config/cachyos-dotfiles/backups/<timestamp>/`
4. Copies from repo to system path
5. System files (`/etc/*`) are copied with `sudo`
6. Offers to install packages from `pkglist.txt`

**No secrets leak:**
- `~/.ssh/` is disabled by default
- `kwalletrc` is disabled by default (doesn't contain wallet data, but disabled for paranoia)
- `kdeconnect/` is disabled by default
- Your `.gitconfig` is tracked (name, email, aliases) — review before pushing if concerned

## Requirements

- Python 3.8+ (stdlib only — no pip packages needed)
- `git` (for repo operations)
- `pacman` (for package list export)
- CachyOS / Arch Linux (uses pacman, paru config paths)
- GitHub repo with SSH or HTTPS access (`gh auth login` for HTTPS)

## File locations

| File | Path |
|---|---|
| CLI tool | wherever you put `cachyos-dotfiles` |
| Config | `~/.config/cachyos-dotfiles/config.json` |
| Manifest | `~/.config/cachyos-dotfiles/manifest.json` |
| Local repo | `~/.local/share/cachyos-dotfiles/repo/` |
| Restore backups | `~/.config/cachyos-dotfiles/backups/<timestamp>/` |

## Tips

- **After restoring KDE configs**, restart Plasma: `systemctl --user restart plasma-plasmashell` or log out and back in
- **Wallpaper paths** in `plasmarc` may reference external drives — you may need to update paths after restore
- **Monitor layout** (`kwinoutputconfig.json`) contains display UUIDs that change between installs — KWin usually handles this gracefully
- **Run `backup` regularly** before system updates: `./cachyos-dotfiles backup`
- **Add to cron/systemd timer** for automated backups: `0 20 * * * cd ~/cachyos-dotfiles && ./cachyos-dotfiles backup`
