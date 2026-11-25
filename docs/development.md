# Developing NfoGlow for Windows

## How it works

Rust with egui and eframe (wgpu). The page is laid out and drawn by `view.rs` through the egui painter; what only the
graphics card can do (colour fringe, tube masks, the CRT power-on) is the view's own render pass in `post.rs`. The PNG and
PDF export (`export.rs`) draws the same page on the processor, so it works without a window:
`nfoglow --export FILE OUT.png`. Everything that moves runs on one clock, at 30 frames a second, and stops while nothing
moves. Settings are a JSON file in `%APPDATA%\NfoGlow`, in the same format as NfoGlow for Linux, so look codes
(`nfoglow:1:...`) work in both. The translations are the `.po` files of NfoGlow for Linux in `po/`, built into the program.

NfoGlow for Linux is the reference: what an NFO shows, the effects and the options must match it, the window itself
follows the style of Windows 11. How that is checked: [CONFORMANCE.md](../CONFORMANCE.md).

## Building and trying it

    cargo test
    cargo run -- some.nfo
    cargo run -- --export some.nfo some.png     (or .pdf; no window, in the look of the settings)

The setup (NSIS, "only for me" or "for all users"; Start menu, optional desktop shortcut, optional "open NFO files
with NfoGlow" for .nfo .diz .ans .asc .srr) is made from the release build:

    cargo build --release
    makensis -DVERSION=1.1.4 installer/nfoglow.nsi        # target/NfoGlow-Setup-1.1.4.exe

On Linux the same works with the Windows target (rustup target add x86_64-pc-windows-gnu, and the packages
gcc-mingw-w64-x86-64 and nsis):

    cargo build --release --target x86_64-pc-windows-gnu
    makensis -DVERSION=1.1.4 -DEXE=../target/x86_64-pc-windows-gnu/release/nfoglow.exe installer/nfoglow.nsi

The release also carries the portable program: `nfoglow.exe` of the release build, as `NfoGlow-Portable-<version>.exe`
(the workflow copies it).

GitHub Actions (`.github/workflows/windows.yml`) tests and builds both on Windows for every push, and attaches them to a
published release.

How the port is kept in step with NfoGlow for Linux: [CONFORMANCE.md](CONFORMANCE.md).

## Project layout

Rust with egui; everything is in `src/`:

| File | What it does |
| --- | --- |
| `main.rs` | The window: loading, browsing, search, zoom, keys, menus, the look and its undo |
| `core.rs` | Settings file, look codes, themes and palettes, the block-character table |
| `decode.rs` | Bytes to text: CP437 and UTF-8, broken encodings, ANSI, ZIP and `.srr` |
| `page.rs` | The page: lines, links, blocks of art, search matches, the selection |
| `view.rs` | Drawing the page: blocks, glow, colour ramp and the effects on the art |
| `smooth.rs` | Smooth edges for the block art (Scale2x, blur, steep cut) |
| `post.rs` | The view's own render pass: colour fringe, tube masks, the CRT power-on, openings over the stars |
| `fx.rs` | What moves around the page: stars, scroller, the live CRT, the effects on opening |
| `export.rs` | The page as PNG or PDF |
| `music.rs` | The music player (rodio; tracker modules through xmrsplayer) and the beat for the pulse |
| `i18n.rs` | Translations from `po/*.po` (the files of NfoGlow for Linux) |
| `ui.rs` | The building blocks of the window: switches, rows, cards, sections, swatches, icons |
| `sidebar.rs`, `settings.rs` | The effects sidebar and the settings window |
| `about.rs` | The About window and its artwork |
