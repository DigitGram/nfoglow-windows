<p align="center">
  <img src="assets/nfoglow-256.png" width="96" height="96" alt="NfoGlow icon">
</p>

<h1 align="center">NfoGlow for Windows</h1>

<p align="center">
  <b>NFO and ANSI art, the way the scene meant it.</b> A viewer for <code>.nfo</code>, <code>.diz</code> and <code>.ans</code> files<br>
  on Windows 10 and 11, with exact pixels, glow, animations, effects and chiptunes.
</p>

<p align="center">
  <a href="https://github.com/DigitGram/nfoglow-windows/actions/workflows/windows.yml"><img src="https://github.com/DigitGram/nfoglow-windows/actions/workflows/windows.yml/badge.svg" alt="Build"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="License: GPL-3.0"></a>
  <a href="https://github.com/DigitGram/nfoglow-windows/releases/latest"><img src="https://img.shields.io/github/v/release/DigitGram/nfoglow-windows" alt="Latest release"></a>
</p>

<p align="center">
  <a href="#install"><b>Install</b></a> ·
  <a href="#keyboard">Keyboard</a> ·
  <a href="#verify-the-download">Verify the download</a> ·
  <a href="https://github.com/DigitGram/nfoglow">NfoGlow for Linux</a> ·
  <a href="#support">Support</a>
</p>

This is the official Windows port of [NfoGlow](https://github.com/DigitGram/nfoglow), rewritten in Rust. It shows an NFO
the same way as NfoGlow for Linux, with the same effects, themes and options, and reads the same look codes; only the
window follows the style of Windows 11.

Inspired by the Windows classic [iNFekt](https://github.com/syndicodefront/infekt).

Most text viewers draw the block characters `█ ▀ ▄ ░ ▒ ▓` with a font, which leaves gaps and seams in the art.
NfoGlow paints them as exact pixels, optionally with a soft glow, and keeps the text crisp.

## Contents

- [What you get](#what-you-get)
- [Install](#install)
- [Verify the download](#verify-the-download)
- [Privacy](#privacy)
- [Keyboard](#keyboard)
- [Command line](#command-line)
- [Differences from NfoGlow for Linux](#differences-from-nfoglow-for-linux)
- [Translating](#translating)
- [Building](#building)
- [How it was made](#how-it-was-made)
- [Support](#support)
- [Licence](#licence)

## What you get

- **Clean art.** Block graphics as exact pixels, without seams.
- **Smooth edges, for the pixel haters.** One switch turns the steps of the block art into straight diagonals and round curves, anti-aliased, and lets the ░▒▓ shading fade softly. Off by default, because some of us like our pixels.
- **Effects (F9).** Neon glow, phosphor glow, colour ramps, scanlines, shadow mask and aperture grille, colour fringe, a live CRT and the IBM VGA font. The window grows by the sidebar instead of covering the NFO.
- **Styles.** Classic, Soft, Neon, Tube, Retrowave and Demoscene set everything with one click; every effect can still be tuned on its own.
- **Demoscene mode.** Colour cycling, wobbling logo, stars, sine scroller, sparkles and a glow that pulses to the music, plus one of eight opening effects (modem build-up, C64 loader, DOS boot, CRT power-on, code rain and more). Off by default.
- **Random look.** Roll a new combination with one click, keep it or get a fresh one at every start. Ctrl+Z undoes any change.
- **Music.** Nine built-in chiptunes by Stennish, or your own folder of MOD, XM, S3M, IT, OGG, MP3, FLAC and WAV (e.g. from [The Mod Archive](https://modarchive.org)). Off by default.
- **Reads old NFOs right.** CP437 out of the box, broken UTF-8 repaired, CP850 and Latin-1 on request. ANSI art in 16, 256 and true colour, SAUCE included.
- **Handy.** Open straight from a ZIP or ReScene `.srr`, browse a release folder with ← and →, reload on file change, search (Ctrl+F), clickable links, zoom, text-only view (Ctrl+T) and copying without the art.
- **Themes.** Light, Dark, Black & White, Amber, Terminal, Paper, C64 and Custom, with Consolas, Cascadia, Lucida Console, Courier New or JetBrains Mono. The first start follows the light or dark mode of Windows.
- **Share your look.** Copy look puts one line of text on the clipboard, Paste look applies it, also between Windows and Linux. No paths or file lists are shared.
- **Save as PNG or PDF** (Ctrl+S), also from the command line.
- **Ten languages**, the same as NfoGlow for Linux.

## Install

Windows 10 or 11, 64-bit.

1. Download `NfoGlow-Setup-1.0.0.exe` from the [latest release](https://github.com/DigitGram/nfoglow-windows/releases/latest).
2. Run it. Choose **Only for me** (no administrator needed) or **For all users** (Program Files, asks for administrator
   rights). Opening `.nfo .diz .ans .asc .srr` files with NfoGlow is on by default, a desktop shortcut is off.

**The setup is not signed yet.** Windows SmartScreen may warn about an unknown publisher: click **More info**, then
**Run anyway**. You can check that the file is the one built here, see [Verify the download](#verify-the-download).

Installing a newer release over an existing one keeps your settings. If NfoGlow is running, the setup asks you to close
it first.

Silent install, for example for a script: `NfoGlow-Setup-1.0.0.exe /S /CurrentUser` (or `/AllUsers` from an
administrator prompt).

**Uninstall** in Settings → Apps → Installed apps → NfoGlow. It asks whether to delete your settings too (No is the
default); they are in `%APPDATA%\NfoGlow`.

## Verify the download

Every release is built by [GitHub Actions](https://github.com/DigitGram/nfoglow-windows/actions/workflows/windows.yml)
from the tagged commit; the log of that run is public. The release has a `SHA256SUMS.txt` with the checksums of the
setup and of `nfoglow.exe`. In PowerShell, in your Downloads folder:

```powershell
Get-FileHash .\NfoGlow-Setup-1.0.0.exe -Algorithm SHA256
```

The hash must be the same as the line for `NfoGlow-Setup-1.0.0.exe` in `SHA256SUMS.txt`.

## Privacy

NfoGlow makes no network connections of its own. A click on a link in an NFO hands it to your web browser. The list of
**recent files** is kept only in your settings file (`%APPDATA%\NfoGlow\settings.json`) on your computer. Turn it off
under Settings → General → Remember recent files; that also clears the list.

## Keyboard

| Keys | Action |
| --- | --- |
| Ctrl+O | Open |
| ← → | Previous, next NFO in the folder |
| Space, Shift+Space | Page down, up |
| F5, Ctrl+R | Reload |
| Ctrl+F, Enter, Shift+Enter | Search, next, previous match |
| Ctrl+T | Art or text only |
| F9 | Effects sidebar |
| Ctrl+Z | Undo the last change to the look |
| Ctrl+M | Music on or off |
| Ctrl+C, Ctrl+A | Copy (the selection, or everything), select all |
| Ctrl+Plus, Ctrl+Minus, Ctrl+0 | Zoom in, out, back to the default zoom |
| Ctrl+S | Save as PNG |
| Ctrl+, | Preferences |
| Ctrl+? | All keyboard shortcuts |
| Ctrl+W, Ctrl+Q | Close the window, quit |

## Command line

```
nfoglow FILE                        open a file
nfoglow --export FILE OUT.png       save FILE as an image, in the look of your settings, without a window
nfoglow --export FILE OUT.pdf       or as a PDF
```

`nfoglow.exe` is in the folder you installed to (for "Only for me": `%LOCALAPPDATA%\Programs\NfoGlow`).

## Differences from NfoGlow for Linux

- The text in a PDF cannot be selected; the page is one picture.
- Music: no `.mptm` and `.opus` files.
- The settings are not shared: each app keeps its own. A look code moves a look from one to the other.

## Translating

The translations are the `.po` files of NfoGlow for Linux, copied into `po/` and built into the program. German is
maintained by the author; all other languages were generated by an AI assistant and have not been reviewed by native
speakers. Please send corrections to [NfoGlow for Linux](https://github.com/DigitGram/nfoglow/issues/new/choose) (the
"Translation fix" form); both apps get them from there.

## Building

    cargo test
    cargo run -- some.nfo
    cargo run -- --export some.nfo some.png     (or .pdf; no window, in the look of the settings)

The setup (NSIS, "only for me" or "for all users"; Start menu, optional desktop shortcut, optional "open NFO files
with NfoGlow" for .nfo .diz .ans .asc .srr) is made from the release build:

    cargo build --release
    makensis -DVERSION=1.0.0 installer/nfoglow.nsi        # target/NfoGlow-Setup-1.0.0.exe

On Linux the same works with the Windows target (rustup target add x86_64-pc-windows-gnu, and the packages
gcc-mingw-w64-x86-64 and nsis):

    cargo build --release --target x86_64-pc-windows-gnu
    makensis -DVERSION=1.0.0 -DEXE=../target/x86_64-pc-windows-gnu/release/nfoglow.exe installer/nfoglow.nsi

GitHub Actions (`.github/workflows/windows.yml`) tests and builds both on Windows for every push, and attaches them to a
published release.

How the port is kept in step with NfoGlow for Linux: [CONFORMANCE.md](CONFORMANCE.md).

### How the code is laid out

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

## How it was made

NfoGlow is developed and maintained by DigitGram. The project is shaped by its requirements, design decisions and ongoing development.

AI-assisted tools were used during implementation and to support code quality improvements, testing, hardening and translations. All changes are reviewed and tested by the author, who is responsible for the final result.

The code is open, the tests are in `src/` (`cargo test`), and issues and pull requests are welcome.

## Support

NfoGlow is free and stays free. If it is useful to you, you can support its development on [GitHub Sponsors](https://github.com/sponsors/DigitGram) or [Liberapay](https://liberapay.com/DigitGram). Donations are voluntary and unlock nothing.

## Licence

Copyright (C) 2026 DigitGram. NfoGlow is free software: you can redistribute it and/or modify it under the terms of the
[GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or
(at your option) any later version. It comes without any warranty.

The bundled DOS font `assets/PxPlus_IBM_VGA_8x16.ttf` is from The Ultimate Oldschool PC Font Pack by VileR
(<https://int10h.org/oldschool-pc-fonts/>), CC BY-SA 4.0, see `assets/LICENSE.txt`. The chiptunes in `assets/music` are
by Stennish, CC BY 4.0, see `assets/music/README.md`. Tracker modules are played with
[xmrsplayer](https://codeberg.org/sbechet/xmrsplayer) (MIT). The licences of all libraries in the program are in
`THIRD-PARTY-LICENSES.txt`, which the setup also installs.
