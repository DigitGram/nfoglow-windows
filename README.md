# NfoGlow for Windows

The Windows version of [NfoGlow](https://github.com/DigitGram/nfoglow), the NFO and ANSI art viewer. Written in Rust,
for Windows 10 and 11.

Work in progress. There: reading files (CP437, UTF-8, ANSI art, ZIP, SRR), themes, the six styles, glow, colour
ramp, colour cycling, pulse, neon flicker, wobble, gleam, sparkle, starfield, sine scroller, CRT screen with the living
tube, text glow, the DOS font, the eight effects on opening, a random look, search, selection and copying, links,
browsing a folder with the arrow keys, zoom, recent files, look codes with undo, music (the built-in chiptunes or a folder of
tracker modules MOD, XM, S3M, IT and OGG, MP3, FLAC, WAV), ten languages, PNG and PDF export, the colour fringe and the shadow mask or aperture grille of the tube, a new look
at every start, a free choice of monospace font, reading the file again when it changes. Still to come: selectable text in the PDF.

## Building

    cargo test
    cargo run -- some.nfo
    cargo run -- --export some.nfo some.png     (or .pdf; no window, in the look of the settings)

The setup (NSIS, per user, no administrator; Start menu, optional desktop shortcut, optional "open NFO files
with NfoGlow" for .nfo .diz .ans .asc .srr) is made from the release build:

    cargo build --release
    makensis -DVERSION=0.1.0 installer/nfoglow.nsi        # target/NfoGlow-Setup-0.1.0.exe

On Linux the same works with the Windows target (rustup target add x86_64-pc-windows-gnu, and the packages
gcc-mingw-w64-x86-64 and nsis):

    cargo build --release --target x86_64-pc-windows-gnu
    makensis -DVERSION=0.1.0 -DEXE=../target/x86_64-pc-windows-gnu/release/nfoglow.exe installer/nfoglow.nsi

GitHub Actions (`.github/workflows/windows.yml`) tests and builds both on Windows for every push and tag.

Look codes (`nfoglow:1:…`) are the same in both apps.

## How the code is laid out

Rust with egui; everything is in `src/`:

| File | What it does |
| --- | --- |
| `main.rs` | The window: loading, browsing, search, zoom, keys, menus, the look and its undo |
| `core.rs` | Settings file, look codes, themes and palettes, the block-character table |
| `decode.rs` | Bytes to text: CP437 and UTF-8, broken encodings, ANSI, ZIP and `.srr` |
| `page.rs` | The page: lines, links, blocks of art, search matches, the selection |
| `view.rs` | Drawing the page: blocks, glow, colour ramp and the effects on the art |
| `post.rs` | The view's own render pass: colour fringe, tube masks, the CRT power-on, openings over the stars |
| `fx.rs` | What moves around the page: stars, scroller, the live CRT, the effects on opening |
| `export.rs` | The page as PNG or PDF |
| `music.rs` | The music player (rodio; tracker modules through xmrsplayer) and the beat for the pulse |
| `i18n.rs` | Translations from `po/*.po` (the files of NfoGlow for Linux) |
| `ui.rs` | The building blocks of the window: switches, rows, cards, sections, swatches, icons |
| `sidebar.rs`, `settings.rs` | The effects sidebar and the settings window |
| `about.rs` | The About window and its artwork |

## Licence

GPL-3.0-or-later, © 2026 DigitGram. The bundled DOS font `assets/PxPlus_IBM_VGA_8x16.ttf` is from The Ultimate
Oldschool PC Font Pack by VileR (<https://int10h.org/oldschool-pc-fonts/>), CC BY-SA 4.0, see `assets/LICENSE.txt`. The chiptunes in `assets/music` are by Stennish, CC BY 4.0, see `assets/music/README.md`. Tracker modules are played with
[xmrsplayer](https://codeberg.org/sbechet/xmrsplayer) (MIT).
