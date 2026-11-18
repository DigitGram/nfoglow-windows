# Checking NfoGlow for Windows against NfoGlow for Linux

The Linux version (Python, `nfoglow`) is the reference. The Windows port must match it in what the NFO shows, the
effects, the openings, the colours and the options; the window (settings, menus) may look different.

## Replay test (`src/conformance.rs`)
Replays what was recorded from the Python original and compares it with this port:
core (settings, look codes, themes, text cleaning), decode (encodings, ANSI, forum text, SRR, files), overlays (tape loader,
DOS boot, opening durations), page (lines, links, logo groups, search, selection, copying), the glow, the colour cycling,
the neon flicker, the music beat and every message of the nine translations.

    NFOGLOW_CASES=<nfoglow-rust>/rust \
    NFOGLOW_PAGE_CASES=<page_cases.jsonl> NFOGLOW_I18N_CASES=<i18n_cases.json> \
    cargo test conformance -- --nocapture

The recordings come from the nfoglow-rust folder (`rust/*/cases.jsonl`, made with rust-anything); `page_cases.jsonl` is
made by `rust/tests/make_page_cases.py` there. Run cargo with a memory limit on small machines
(`systemd-run --user --scope -p MemoryMax=3G ...`).

## Known differences that are on purpose or accepted
* The standard font (Consolas, not DejaVu Sans Mono): text widths differ a little.
* The settings window, menus and sidebar follow Windows 11 instead of GNOME.
* Look codes are not byte-identical (another zlib), but each app reads the other's.
* The grain of the live CRT is another pattern; the stars and warp streaks fall at other places (another random source).
* The PDF export is one picture per page, its text cannot be selected.
* Extreme values (times beyond 10^9 seconds, look codes cut at an odd length) behave a little differently.
