// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! From bytes to text: CP437 or UTF-8 (and NFOs broken by a wrong encoding), ANSI art with its colours and cursor
//! moves, and the NFO inside a ZIP or a pyReScene .srr. No drawing here. Ported from decode.py of NfoGlow for Linux.
use crate::core::{is_art, Rgb, DOS_GLYPHS, VGA};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub const NFO_SUFFIXES: [&str; 4] = [".nfo", ".diz", ".ans", ".asc"];
pub const MAX_MEMBER: u64 = 4 * 1024 * 1024; // largest NFO read; guards against zip bombs and videos dropped by mistake
pub const MAX_ROWS: usize = 10000; // ANSI cursor moves stop here, so a bad file cannot make millions of lines
pub const MAX_CELLS: usize = 2_000_000; // ANSI lines end here: a SAUCE width of 65535 times 10000 rows would fill the memory

// the upper halves (bytes 128..255) of the code pages, as Python decodes them
const CP437: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";
const CP850: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜø£Ø×ƒáíóúñÑªº¿®¬½¼¡«»░▒▓│┤ÁÂÀ©╣║╗╝¢¥┐└┴┬├─┼ãÃ╚╔╩╦╠═╬¤ðÐÊËÈıÍÎÏ┘┌█▄¦Ì▀ÓßÔÒõÕµþÞÚÛÙýÝ¯´\u{ad}±‗¾¶§÷¸°¨·¹³²■\u{a0}";
// Windows-1252 bytes 128..159; its five unused bytes stay as Latin-1
const CP1252: &str = "€\u{81}‚ƒ„…†‡ˆ‰Š‹Œ\u{8d}Ž\u{8f}\u{90}‘’“”•–—˜™š›œ\u{9d}žŸ";

fn from_page(data: &[u8], upper: &str) -> String {
    let upper: Vec<char> = upper.chars().collect();
    data.iter().map(|&b| if b < 128 { b as char } else { upper[b as usize - 128] }).collect()
}

fn latin1(data: &[u8]) -> String {
    data.iter().map(|&b| b as char).collect()
}

/// The NFO as the NFO plugins of forums want it pasted: its CP437 bytes read as Windows text, so █ comes out as Û
/// and ▀ as ß (what a text editor shows); the plugin turns that back into the art.
pub fn forum_text<S: AsRef<str>>(lines: &[S]) -> String {
    let text = lines.iter().map(AsRef::as_ref).collect::<Vec<_>>().join("\n");
    text.chars()
        .map(|ch| {
            let byte = match ch {
                _ if (ch as u32) < 128 => ch as u32,
                _ => match (DOS_GLYPHS.chars().position(|g| g == ch), CP437.chars().position(|g| g == ch)) {
                    (Some(i), _) if !matches!(i + 1, 9 | 10 | 13) => i as u32 + 1, // ♦ goes back to its byte 4; tab, LF and CR keep their bytes, their symbols are no CP437
                    (_, Some(i)) => i as u32 + 128,
                    (Some(_), _) => '?' as u32,
                    _ => '?' as u32,
                },
            };
            match byte {
                128..=159 => CP1252.chars().nth(byte as usize - 128).unwrap(),
                _ => char::from_u32(byte).unwrap(),
            }
        })
        .collect()
}

/// bytes -> (text, encoding label, SAUCE width).
/// Auto: UTF-8 when the bytes are valid UTF-8, else CP437 (the NFO standard).
pub fn decode(mut data: &[u8], enc: &str) -> (String, &'static str, Option<u16>) {
    let mut width = None;
    if data.len() >= 128 && data[data.len() - 128..].starts_with(b"SAUCE00") {
        let sauce = &data[data.len() - 128..];
        width = Some(u16::from_le_bytes([sauce[96], sauce[97]])).filter(|&w| w > 0); // TInfo1 = columns for ANSI
        data = &data[..data.len() - 128];
        if let Some(cut) = data.iter().rposition(|&b| b == 0x1a) {
            data = &data[..cut]; // EOF marker; only SAUCE comments follow it
        }
    }
    while let [rest @ .., 0x1a] = data {
        data = rest;
    }
    data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    if enc == "auto" || enc == "utf-8" {
        match std::str::from_utf8(data) {
            Ok(text) => {
                if let Some(fixed) = (enc == "auto").then(|| undo_double_encoding(text)).flatten() {
                    return (fixed, "CP437 ← UTF-8", width);
                }
                return (text.to_string(), "UTF-8", width);
            }
            Err(_) if enc == "utf-8" => return (String::from_utf8_lossy(data).into_owned(), "UTF-8", width),
            Err(_) => {}
        }
    }
    match enc {
        "cp850" => (from_page(data, CP850), "CP850", width),
        "latin-1" => (latin1(data), "Latin-1", width),
        _ => (from_page(data, CP437), "CP437", width),
    }
}

/// CP437 NFOs that an editor read as Latin-1/Windows-1252 and saved as UTF-8 show "ÛÛÛ" instead of "███".
/// Returns the repaired text, or None when the text does not look like that.
fn undo_double_encoding(text: &str) -> Option<String> {
    if text.chars().any(|ch| (0x2500..=0x259F).contains(&(ch as u32))) {
        return None; // real block characters: proper UTF-8
    }
    let as_latin1 = |ch: char| u8::try_from(ch as u32).ok();
    let as_cp1252 = |ch: char| match ch as u32 {
        0x80..=0x9F => None,
        n @ 0..=0xFF => Some(n as u8),
        _ => CP1252.chars().position(|c| c == ch).map(|i| i as u8 + 128),
    };
    for bytes in [text.chars().map(as_latin1).collect::<Option<Vec<u8>>>(), text.chars().map(as_cp1252).collect()] {
        let Some(bytes) = bytes else { continue };
        let fixed = from_page(&bytes, CP437);
        if fixed.chars().filter(|&ch| is_art(ch)).count() >= 10 {
            return Some(fixed); // a few accented letters are not art
        }
    }
    None
}

pub fn is_ansi(text: &str) -> bool {
    text.contains("\x1b[")
}

fn color256(n: u32) -> Rgb {
    match n {
        0..=15 => VGA[n as usize],
        16..=231 => [(n - 16) / 36, (n - 16) / 6 % 6, (n - 16) % 6].map(|v| [0, 95, 135, 175, 215, 255][v as usize]),
        _ => [(8 + (n - 232).saturating_mul(10)).min(255) as u8; 3], // a number above 255 comes out white
    }
}

#[derive(Clone, Copy)]
enum Ink {
    Vga(usize),
    True(Rgb),
}

impl Ink {
    /// bright: bold for the text, blink for the background (like iCE colour)
    fn rgb(self, bright: bool) -> Rgb {
        match self {
            Ink::Vga(n) if bright && n < 8 => VGA[n + 8],
            Ink::Vga(n) => VGA[n],
            Ink::True(c) => c,
        }
    }
}

/// (foreground, background) of every cell of a line
pub type Colors = Vec<Vec<(Rgb, Rgb)>>;

/// ANSI art -> (lines, colors). Handles SGR colours (16, 256, true colour, bold = bright, blink = bright background
/// like iCE colour) and cursor movement.
/// shortcut: no scroll regions or line wrapping modes, ANSI art rarely uses them; upgrade when a file needs them.
pub fn parse_ansi(text: &str, width: Option<u16>) -> (Vec<String>, Colors) {
    let width = width.unwrap_or(80).max(1) as usize;
    let mut grid: HashMap<(usize, usize), (char, Rgb, Rgb)> = HashMap::new();
    let (mut r, mut c, mut saved) = (0usize, 0usize, (0usize, 0usize));
    let (mut fg, mut bg, mut bold, mut blink, mut inverse) = (Ink::Vga(7), Ink::Vga(0), false, false, false);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        i += 1;
        // an escape sequence: ESC [ digits ; ? and one letter
        let mut end = i + 1;
        while ch == '\x1b' && chars.get(i) == Some(&'[') && chars.get(end).is_some_and(|c| c.is_ascii_digit() || *c == ';' || *c == '?') {
            end += 1;
        }
        if !(ch == '\x1b' && chars.get(i) == Some(&'[') && chars.get(end).is_some_and(char::is_ascii_alphabetic)) {
            match ch {
                '\n' => (r, c) = (r + 1, 0),
                '\r' => c = 0,
                '\t' => c = (width - 1).min((c / 8 + 1) * 8),
                _ if ch >= ' ' => {
                    if c >= width {
                        (r, c) = (r + 1, 0);
                    }
                    let (f, b) = (fg.rgb(bold), bg.rgb(blink));
                    grid.insert((r, c), if inverse { (ch, b, f) } else { (ch, f, b) });
                    c += 1;
                }
                _ => {}
            }
            continue;
        }
        let params: String = chars[i + 1..end].iter().filter(|c| **c != '?').collect();
        let cmd = chars[end];
        // 9 digits at most: longer numbers would not fit, and no screen is that large
        let num = |p: &str| if p.is_empty() { 0 } else { p.get(..9).unwrap_or(p).parse::<usize>().unwrap_or(0) };
        let mut args: Vec<usize> = if end > i + 1 { params.split(';').map(num).collect() } else { Vec::new() };
        i = end + 1;
        let n = args.first().copied().unwrap_or(1).max(1);
        match cmd {
            'm' => {
                if args.is_empty() {
                    args.push(0);
                }
                let mut k = 0;
                while k < args.len() {
                    let a = args[k];
                    match a {
                        0 => (fg, bg, bold, blink, inverse) = (Ink::Vga(7), Ink::Vga(0), false, false, false),
                        1 => bold = true,
                        22 => bold = false,
                        5 | 6 => blink = true,
                        25 => blink = false,
                        7 => inverse = true,
                        27 => inverse = false,
                        30..=37 => fg = Ink::Vga(a - 30),
                        39 => fg = Ink::Vga(7),
                        40..=47 => bg = Ink::Vga(a - 40),
                        49 => bg = Ink::Vga(0),
                        90..=97 => fg = Ink::Vga(a - 90 + 8),
                        100..=107 => bg = Ink::Vga(a - 100 + 8),
                        38 | 48 if k + 1 < args.len() => {
                            let col = if args[k + 1] == 5 && k + 2 < args.len() {
                                k += 2;
                                Some(color256(args[k] as u32))
                            } else if args[k + 1] == 2 && k + 4 < args.len() {
                                k += 4;
                                Some([args[k - 2], args[k - 1], args[k]].map(|v| v.min(255) as u8))
                            } else {
                                None
                            };
                            match (col, a) {
                                (Some(col), 38) => fg = Ink::True(col),
                                (Some(col), _) => bg = Ink::True(col),
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    k += 1;
                }
            }
            'A' => r = r.saturating_sub(n),
            'B' => r = (r + n).min(MAX_ROWS - 1),
            'C' => c = (width - 1).min(c + n),
            'D' => c = c.saturating_sub(n),
            'H' | 'f' => {
                r = args.first().map_or(0, |a| a.saturating_sub(1).min(MAX_ROWS - 1));
                c = args.get(1).map_or(0, |a| a.saturating_sub(1));
            }
            's' => saved = (r, c),
            'u' => (r, c) = saved,
            _ => {}
        }
    }
    let mut widths: HashMap<usize, usize> = HashMap::new();
    for &(y, x) in grid.keys() {
        let w = widths.entry(y).or_default();
        *w = (*w).max(x + 1);
    }
    let (mut lines, mut colors) = (Vec::new(), Vec::new());
    let mut left = MAX_CELLS;
    for y in 0..widths.keys().max().map_or(0, |m| m + 1) {
        let w = widths.get(&y).copied().unwrap_or(0);
        if w > left {
            break; // 25 times a full 80-column ANSI: only a file made to blow up gets here
        }
        left -= w;
        let cells: Vec<_> = (0..w).map(|x| grid.get(&(y, x)).copied().unwrap_or((' ', VGA[7], VGA[0]))).collect();
        lines.push(cells.iter().map(|t| t.0).collect());
        colors.push(cells.iter().map(|t| (t.1, t.2)).collect());
    }
    (lines, colors)
}

/// The member to show: the first .nfo, else .diz/.ans/.asc; None when there is none.
fn best<'a>(names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    // by its ending, not by an extension: a member named ".nfo" has none
    let rank = |n: &str| NFO_SUFFIXES.iter().position(|s| n.to_lowercase().ends_with(s));
    names.filter(|n| rank(n).is_some()).min_by_key(|n| (rank(n), n.to_lowercase()))
}

/// The bytes of a file, at most MAX_MEMBER: a video dropped by mistake must not fill the memory.
fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    file.take(MAX_MEMBER + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_MEMBER {
        return Err("file too large".into());
    }
    Ok(data)
}

/// (name, bytes) of the files a pyReScene .srr stores (the NFO, SFV, ...).
/// An SRR is a RAR without its contents: an SRR header block (type 0x69), then one "stored file" block (0x6A) per
/// file, then the RAR headers, which hold no files. Each block starts with CRC (2), type (1), flags (2) and header
/// size (2); a stored file adds its size (4), the name length (2) and the name.
pub fn srr_files(data: &[u8]) -> Result<Vec<(String, &[u8])>, String> {
    if !data.starts_with(b"iii") {
        return Err("not an SRR file".into());
    }
    let le = |at: usize, n: usize| data.get(at..at + n).map_or(0, |b| b.iter().rev().fold(0usize, |v, &b| v << 8 | b as usize));
    let part = |at: usize, n: usize| &data[at.min(data.len())..(at.saturating_add(n)).min(data.len())]; // a cut-off file gives less
    let (mut files, mut pos) = (Vec::new(), 0);
    while pos + 7 <= data.len() {
        let (kind, size) = (data[pos + 2], le(pos + 5, 2));
        if size < 7 || (kind != 0x69 && kind != 0x6A) {
            break;
        }
        if kind == 0x6A {
            let (length, name_len) = (le(pos + 7, 4), le(pos + 11, 2));
            files.push((String::from_utf8_lossy(part(pos + 13, name_len)).into_owned(), part(pos + size, length)));
            pos += length;
        }
        pos += size;
    }
    Ok(files)
}

fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// (bytes, display name) of an NFO file, or of the NFO inside a ZIP or a pyReScene .srr (the first .nfo, else
/// .diz/.ans/.asc). The error is a message for the window: unreadable files and archives without an NFO.
pub fn read(path: &Path) -> Result<(Vec<u8>, String), String> {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let low = file_name.to_lowercase();
    let no_nfo = || "no NFO inside".to_string();
    if low.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        // by_index_raw: name and size only, so an encrypted member is still listed (and reading it gives the message)
        let members: Vec<(usize, String)> = (0..zip.len())
            .filter_map(|i| {
                let f = zip.by_index_raw(i).ok()?;
                (f.size() <= MAX_MEMBER).then_some((i, f.name().ok()?.into_owned()))
            })
            .collect();
        let name = best(members.iter().map(|m| m.1.as_str())).ok_or_else(no_nfo)?;
        let index = members.iter().find(|m| m.1 == name).unwrap().0;
        let mut data = Vec::new();
        // encrypted, a packing method the library lacks, damaged or cut-off data: all end here as a message.
        // take(): the size in the header may lie
        zip.by_index(index).map_err(|e| e.to_string())?.take(MAX_MEMBER + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
        if data.len() as u64 > MAX_MEMBER {
            return Err("file too large".into());
        }
        Ok((data, format!("{file_name} › {}", base_name(name))))
    } else if low.ends_with(".srr") {
        let data = read_file(path)?;
        let files = srr_files(&data)?;
        let name = best(files.iter().map(|f| f.0.as_str())).ok_or_else(no_nfo)?;
        let member = files.iter().find(|f| f.0 == name).unwrap().1.to_vec();
        Ok((member, format!("{file_name} › {}", base_name(name))))
    } else {
        Ok((read_file(path)?, file_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn code_pages_are_complete() {
        assert_eq!((CP437.chars().count(), CP850.chars().count(), CP1252.chars().count()), (128, 128, 32));
    }

    #[test]
    fn decode_cp437_and_utf8() {
        assert_eq!(decode(b"\xdb\xdb\xb0 hi", "auto"), ("██░ hi".into(), "CP437", None));
        assert_eq!(decode("█ hi".as_bytes(), "auto"), ("█ hi".into(), "UTF-8", None));
        assert_eq!(decode(b"\xef\xbb\xbfhi", "auto").0, "hi");
        assert_eq!(decode(b"\x9b\xe4", "cp850"), ("øõ".into(), "CP850", None));
        assert_eq!(decode(b"\xe4", "latin-1"), ("ä".into(), "Latin-1", None));
        assert_eq!(decode(b"a\xffb", "utf-8"), ("a\u{fffd}b".into(), "UTF-8", None));
    }

    fn sauce(width: u16) -> Vec<u8> {
        let mut s = b"SAUCE00".to_vec();
        s.resize(128, b' ');
        s[96..98].copy_from_slice(&width.to_le_bytes());
        s
    }

    #[test]
    fn decode_strips_sauce_and_reads_width() {
        let (text, _label, width) = decode(&[b"art\x1aCOMNTcomment".as_slice(), &sauce(160)].concat(), "auto");
        assert_eq!((text.as_str(), width), ("art", Some(160)));
    }

    #[test]
    fn ansi_colours_and_cursor() {
        let (lines, colors) = parse_ansi("\x1b[1;31mA\x1b[0m\x1b[2CB\r\n\x1b[44mC", None);
        assert_eq!(lines, ["A  B", "C"]);
        assert_eq!(colors[0][0].0, VGA[9]); // bold red = bright red
        assert_eq!(colors[1][0].1, VGA[4]); // blue background
        assert_eq!(parse_ansi("\x1b[38;2;255;0;0mX", None).1[0][0].0, [255, 0, 0]);
        assert_eq!(parse_ansi("\x1b[7mX\x1b[mY\x1b[38;5;196mZ", None).1[0], [(VGA[0], VGA[7]), (VGA[7], VGA[0]), ([255, 0, 0], VGA[0])]);
        assert_eq!(parse_ansi("ab\x1b[sX\x1b[1;1HY\x1b[uZ\x1b[?25l", None).0, ["YbZ"]); // Z lands on the saved place of X
        assert_eq!(parse_ansi("a\x1b\x07b\x1b[", None).0, ["ab["]); // a lone ESC is no sequence
    }

    #[test]
    fn hostile_ansi_stays_small() {
        // a SAUCE width of 65535 and a cursor jump to the right edge on 10000 rows: 140 KB that once meant 655
        // million cells. The lines stop at MAX_CELLS.
        let data = [b"\x1b[B\x1b[65534CX".repeat(10000), b"\x1a".to_vec(), sauce(65535)].concat();
        let (text, _label, width) = decode(&data, "auto");
        let (lines, colors) = parse_ansi(&text, width);
        assert!(width == Some(65535) && !lines.is_empty() && lines.iter().map(|l| l.chars().count()).sum::<usize>() <= MAX_CELLS);
        assert_eq!(colors.len(), lines.len());
        assert_eq!(parse_ansi(&format!("\x1b[{}mX", "9".repeat(5000)), None).0, ["X"]); // a number too long
        assert_eq!(parse_ansi("\x1b[38;5;300mA\x1b[38;2;999;0;0mB", None).1[0][1].0, [255, 0, 0]); // colours above 255
        assert_eq!(parse_ansi("A\x1b[5000000BX\x1b[9999999;1HY", None).0.len(), MAX_ROWS); // huge cursor jumps
        assert_eq!(parse_ansi("\x1b[1;999999999HX", None).0, ["", "X"]); // past the right edge: the next line
    }

    #[test]
    fn double_encoded_cp437_is_repaired() {
        let art: Vec<u8> = [vec![0xdb; 12], b"\xb0\xb1\xb2 Title: x".to_vec()].concat(); // CP437 bytes
        let broken = latin1(&art); // read as Latin-1, saved as UTF-8
        let (text, label, _w) = decode(broken.as_bytes(), "auto");
        assert!(text.starts_with(&format!("{}░▒▓", "█".repeat(12))) && label == "CP437 ← UTF-8");
        assert_eq!(decode("Café ñandú".as_bytes(), "auto"), ("Café ñandú".into(), "UTF-8", None)); // real accents stay
        // read as Windows-1252: byte 0x9b (¢ in CP437) became ›, which Latin-1 does not have
        let broken = format!("›{}", "Û".repeat(10));
        assert_eq!(decode(broken.as_bytes(), "auto").0, format!("¢{}", "█".repeat(10)));
    }

    #[test]
    fn forum_text_is_the_file_read_as_windows_text() {
        // █ as Û, ▀ as ß, ü (0x81) kept
        let lines = [from_page(b"\xdb\xdf\xdc\xb0 Gr\x81\xe1e \xf9", CP437)];
        assert_eq!(forum_text(&lines), "ÛßÜ° Gr\u{81}áe ù");
        assert_eq!(forum_text(&["♦ x ♦", "€"]), "\x04 x \x04\n?"); // a forum gets the bytes the file had
    }

    fn zip_file(path: &Path, members: &[(&str, &[u8])], method: zip::CompressionMethod) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in members {
            z.start_file(*name, zip::write::SimpleFileOptions::default().compression_method(method)).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nfoglow-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn zip() {
        let dir = temp_dir("zip");
        let z = dir.join("release.zip");
        let stored = zip::CompressionMethod::Stored;
        zip_file(&z, &[("sample/file_id.diz", b"diz"), ("Release.NFO", b"\xdb nfo")], stored);
        assert_eq!(read(&z).unwrap(), (b"\xdb nfo".to_vec(), "release.zip › Release.NFO".into())); // .nfo wins over .diz
        zip_file(&z, &[("dir/.nfo", b"dot")], stored); // a name that is only the ending
        assert_eq!(read(&z).unwrap(), (b"dot".to_vec(), "release.zip › .nfo".into()));
        zip_file(&z, &[("readme.txt", b"x")], stored);
        assert_eq!(read(&z).unwrap_err(), "no NFO inside");
        std::fs::write(dir.join("broken.zip"), b"not a zip").unwrap();
        assert!(read(&dir.join("broken.zip")).is_err());
        std::fs::write(dir.join("movie.nfo"), vec![0; MAX_MEMBER as usize + 1]).unwrap(); // too big to be an NFO
        assert_eq!(read(&dir.join("movie.nfo")).unwrap_err(), "file too large");
        assert!(read(&dir.join("missing.nfo")).is_err());
        // damaged packed data is a message, not a crash
        zip_file(&z, &[("group.nfo", &b"x".repeat(5000))], zip::CompressionMethod::Deflated);
        let mut data = std::fs::read(&z).unwrap();
        let i = data.windows(9).position(|w| w == b"group.nfo").unwrap() + 9;
        data[i + 5..i + 15].fill(0xff);
        std::fs::write(&z, data).unwrap();
        assert!(read(&z).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn block(kind: u8, flags: u16, head: &[u8], data: &[u8]) -> Vec<u8> {
        let size = (7 + head.len()) as u16;
        [&[0, 0, kind][..], &flags.to_le_bytes(), &size.to_le_bytes(), head, data].concat()
    }

    #[test]
    fn srr() {
        let stored = |name: &[u8], data: &[u8]| {
            block(0x6A, 0x8000, &[&(data.len() as u32).to_le_bytes()[..], &(name.len() as u16).to_le_bytes(), name].concat(), data)
        };
        let app = b"pyReScene 0.7";
        let mut srr = block(0x69, 1, &[&(app.len() as u16).to_le_bytes()[..], app].concat(), b"");
        srr[..2].copy_from_slice(b"ii"); // an SRR starts with "iii"
        srr.extend(stored(b"group-release.sfv", b"file 12345678"));
        srr.extend(stored(b"group-release.nfo", b"\xdb\xdb NFO"));
        srr.extend(block(0x71, 0x8000, b"\x10\x00\x00\x00\x05\x00part1", b""));
        assert_eq!(srr_files(&srr).unwrap(), [("group-release.sfv".to_string(), &b"file 12345678"[..]), ("group-release.nfo".to_string(), &b"\xdb\xdb NFO"[..])]);
        assert!(srr_files(&srr[..srr.len() - 30]).is_ok() && srr_files(b"Rar!").is_err()); // cut off: no crash
        let dir = temp_dir("srr");
        std::fs::write(dir.join("Group-Release.srr"), &srr).unwrap();
        assert_eq!(read(&dir.join("Group-Release.srr")).unwrap(), (b"\xdb\xdb NFO".to_vec(), "Group-Release.srr › group-release.nfo".into()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
