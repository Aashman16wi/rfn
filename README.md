# rfn

A terminal file manager written in Rust, with a yazi-ish three-pane layout and
previews for as many file types as I could wire up.

This is a personal project and a work in progress. It is not published on
crates.io and there are no prebuilt binaries — build it from source.

```text
 rfn  ~ › projects › rfn                                                        12 items
╭ projects ─────────────╮ ╭ rfn ──────────────────────────────────╮ ╭ Preview ──────────────────────────────╮
│  󰉋 dotfiles/          │ │▍  Cargo.toml                     1.3K │ │ [package]                              │
│  󰉋 notes/             │ │  󰍔 README.md                      2.1K │ │ name = "rfn"                           │
│  󰉋 scratch/           │ │  󰉋 src/                                │ │ version = "0.1.0"                      │
│                       │ │  󰊢 .gitignore                     20B │ │                                        │
│                       │ │   rust-toolchain.toml            42B │ │                                        │
╰───────────────────────╯ ╰──────────────────────────────────────╯ ╰────────────────────────────────────────╯
 NORMAL   Cargo.toml  1.3K  2h  rw-r--r--                    preview   git   name↓   ?  help
```

## Features

- Three columns: parent directory, current directory, preview.
- Vim-style navigation and a search filter.
- Preview pane:
  - text and source files (200 lines)
  - images (PNG/JPEG/GIF/WebP/… through the terminal's graphics protocol)
  - SVG, PDF first page and video frames (rendered to a PNG thumbnail)
  - PDF text extraction when a thumbnail cannot be made
  - audio/video metadata via `ffprobe`
  - archive contents for zip, tar, tar.gz, tar.xz, tar.bz2 and tar.zst
- Git status markers per file.
- Tabs, each with its own directory, selection and search; restored across restarts.
- Visual selection for copying, cutting and deleting several files at once.
- Bulk rename through `$EDITOR`.
- Copy, cut, paste (asks before overwriting), rename, create, delete.
- Delete moves files to the system trash by default; trash browser (`T`) for restore.
- Persistent copy/move queue (`Q`) for background operations.
- Preview scroll (`J`/`K`), image zoom (`=`/`-`) and reset (`0`).
- Bookmarks (`m`/`M`): `m` marks the current directory, `M` opens the bookmark list.
- Directories load in the background, so a big folder never freezes the interface.
- Hidden-file toggle and sort by name/size/time/extension.
- Nerd Font icons; eight built-in themes: `tokyo-night` (default), `catppuccin-mocha`, `catppuccin-latte`, `gruvbox-dark`, `gruvbox-light`, `nord`, `dracula`, `rose-pine`, plus `custom` for your own colors. Use `rfn --themes` to list them, `Ctrl+t` to switch inside the app.

## Requirements

- Linux (it may work on macOS; I have not tested it).
- A Nerd Font in your terminal for the icons.
- Rust and Cargo to build.

Optional tools, used when present:

| Tool | Used for |
|------|----------|
| `rsvg-convert` | SVG thumbnails |
| `pdftoppm`, `pdftotext` | PDF thumbnails and text |
| `ffmpeg`, `ffprobe` | video thumbnails and media metadata |

Image previews use the Kitty, iTerm2 or Sixel protocol, depending on
`preview.image_backend`. On terminals without any of those, rfn falls back to
half-block images. If the preview looks wrong, set `image_backend` explicitly
in the config.

## Build and run

```bash
git clone https://github.com/Aashman16wi/rfn
cd rfn
cargo build --release --features "image-preview,pdf-preview,archive-preview"
./target/release/rfn
```

Or use the install script:

```bash
./install.sh  # builds with all features, installs to ~/.local/bin/rfn
```

To put it on your PATH manually:

```bash
install -Dm755 target/release/rfn ~/.local/bin/rfn
```

Run `rfn /some/path` to start elsewhere. With no argument it opens your home
directory.

## Configuration

rfn reads `~/.config/rfn/config.toml` if it exists. Every value has a default,
so the file is optional.

```bash
mkdir -p ~/.config/rfn
rfn --generate-config > ~/.config/rfn/config.toml
```

Colors accept `#rrggbb` strings. Keys are arrays, for example:

```toml
[ui]
show_icons = true
show_hidden = false
show_parent = true
theme = "tokyo-night"  # any built-in theme, or "custom" to use [ui.colors] below

[behavior]
confirm_delete = true
use_trash = true

[preview]
enabled = true
show_images = true
image_backend = "auto" # auto, kitty, iterm, sixel, chafa
max_preview_size = 1048576

[keys]
quit = ["q", "ctrl+c"]
toggle_preview = ["P"]
sort = ["s"]
```

## Keys

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` | Parent directory |
| `l` / `→` / `Enter` | Enter directory / open file |
| `H` / `Ctrl+h` | Go to parent |
| `~` | Go to home |
| `Home` / `G` / `End` | Top / bottom |
| `Ctrl+u` / `Ctrl+d` | Page up / down |
| `n` / `N` | New file / directory |
| `r` | Rename |
| `d` / `Delete` | Delete (asks first; moves to trash by default) |
| `y` / `x` / `p` | Copy / cut / paste |
| `v` | Start visual selection |
| `Space` | Mark / unmark a file |
| `Esc` | Clear selection or search |
| `b` | Bulk rename marked files in `$EDITOR` |
| `t` / `w` | New tab / close tab |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `1` – `9` | Jump to tab |
| `.` | Toggle hidden files |
| `P` | Toggle preview |
| `g` | Toggle git status |
| `R` / `Ctrl+r` | Refresh |
| `s` / `S` | Cycle sort / reverse |
| `/` | Search |
| `?` | Help |
| `q` / `Ctrl+c` | Quit |
| `m` | Bookmark current directory |
| `M` | Open bookmark list |
| `J` / `K` | Preview scroll down / up |
| `=` / `-` | Image zoom in / out |
| `0` | Reset preview (scroll and zoom) |
| `T` | Trash browser |
| `o` | Open with command |
| `Q` | Transfer queue (see below) |
| `Ctrl+t` | Change theme without restarting |

Mouse wheel scrolls and left click selects.

### Transfer queue

`y` / `x` yank and cut files, `p` turns them into transfer tasks that run in the
background, and `Q` opens the queue manager:

| Key | Action in queue |
|-----|-----------------|
| `j` / `k` | Move selection |
| `r` | Run pending / failed tasks |
| `c` | Clear finished tasks |
| `d` | Remove selected task |
| `u` / `o` | Move task up / down |
| `Esc` | Close |

Tasks show `PEND`, `RUN`, `DONE` or `FAIL` and survive a restart, so a failed
copy can be retried later.

### Trash browser

`T` opens the list of files that are in the system trash:

| Key | Action in trash |
|-----|-----------------|
| `j` / `k` | Move selection |
| `Enter` | Go to the folder the file was deleted from |
| `r` | Restore the file |
| `d` / `Delete` | Delete it forever (asks first) |
| `Esc` | Close |

### Bookmarks

`m` marks the current directory, `M` opens the list of bookmarks:

| Key | Action in bookmark list |
|-----|-------------------------|
| `j` / `k` | Move selection |
| `Enter` | Jump to the bookmark |
| `a` | Bookmark the current directory |
| `d` | Remove the bookmark |
| `Esc` | Close |

### Themes

Eight built-in themes ship with rfn: `tokyo-night` (default), `catppuccin-mocha`,
`catppuccin-latte`, `gruvbox-dark`, `gruvbox-light`, `nord`, `dracula` and
`rose-pine`. On top of that `custom` uses the colors from `[ui.colors]` in your
config, so you can write your own palette.

```bash
rfn --themes           # list themes with color previews, marks the active one
rfn --theme nord       # switch theme and save it to the config
rfn --theme custom     # use the colors from [ui.colors]
rfn --theme            # same as --themes
```

Inside the TUI, `Ctrl+t` opens a picker that applies a theme immediately and
saves the choice. The active theme is shown in the status bar, and the picker
marks it with `(active)`. The picker scrolls, so every theme stays reachable
on a short terminal.

Changing the theme only touches the `theme` line of your config: comments,
spacing and the order of the sections are left alone.

## Limitations

- Directory conflicts during paste are replaced, not merged.
- Renaming or creating a file never overwrites an existing one: rfn refuses
  and tells you, instead of replacing the old file.
- The UI is designed for terminals at least 100 columns wide; narrower
  terminals hide the parent column first.
- Only Linux is tested.

## License

MIT OR Apache-2.0.
