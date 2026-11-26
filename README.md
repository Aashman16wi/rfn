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
- Copy, cut, paste, rename, create, delete.
- Hidden-file toggle and sort by name/size/time/extension.
- Tokyo Night-ish colors, Nerd Font icons.

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
cargo build --release
./target/release/rfn
```

To put it on your PATH:

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
| `d` / `Delete` | Delete (asks first) |
| `y` / `x` / `p` | Copy / cut / paste |
| `.` | Toggle hidden files |
| `P` | Toggle preview |
| `g` | Toggle git status |
| `R` / `Ctrl+r` | Refresh |
| `s` / `S` | Cycle sort / reverse |
| `/` | Search |
| `?` | Help |
| `q` / `Ctrl+c` | Quit |

Mouse wheel scrolls and left click selects.

## Limitations

- No tabs, visual selection, bulk rename, or background task queue yet.
- Delete is permanent; there is no trash.
- Copy/paste does not ask before overwriting an existing file.
- The preview cache lives in `/tmp/rfn-previews` and is not cleaned up
  automatically.
- The UI is designed for terminals at least 100 columns wide; narrower
  terminals hide the parent column first.

## License

MIT OR Apache-2.0.
