// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The page: one NFO as a grid of characters, with its links, its blocks of art, the search matches and the
//! selection. No drawing here (the window does that). Ported from page.py of NfoGlow for Linux.
use crate::core::{block, clean_text, is_art, plain_lines, SQUARE};
use crate::decode::{decode, is_ansi, parse_ansi, Colors};

pub type Cell = (usize, usize); // (row, column)

#[derive(Default)]
pub struct Page {
    pub lines: Vec<Vec<char>>,
    pub colors: Option<Colors>, // ANSI art brings its own
    pub label: String,          // the encoding, as shown to the user
    pub cols: usize,
    pub links: Vec<Vec<(usize, usize, String)>>, // per row: (start, end, target)
    pub group: Vec<Option<(usize, usize)>>,      // per row: the run of rows with block graphics it belongs to (top, end)
    pub sel: Option<(Cell, Cell)>,
    pub matches: Vec<(usize, usize, usize)>, // (row, start, end)
    pub current: usize,
}

fn word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Web and e-mail addresses of a line as (start, end, target). An address starts only at the start of a word, else
/// one long line takes the square of its length.
fn links(line: &[char]) -> Vec<(usize, usize, String)> {
    let starts = |i: usize, s: &str| s.chars().enumerate().all(|(k, c)| line.get(i + k).is_some_and(|l| l.to_ascii_lowercase() == c));
    let local = |c: char| word(c) || ".+-".contains(c);
    let label = |mut k: usize| {
        while line.get(k).is_some_and(|c| word(*c) || *c == '-') {
            k += 1;
        }
        k
    };
    let (mut out, mut i) = (Vec::new(), 0);
    while i < line.len() {
        let mut end = i;
        if let Some(prefix) = ["http://", "https://", "www."].iter().find(|p| starts(i, p)) {
            // old NFOs shout: WWW.GROUP.COM
            let mut k = i + prefix.len();
            while line.get(k).is_some_and(|c| !c.is_whitespace() && !"<>\"'".contains(*c)) {
                k += 1;
            }
            if k > i + prefix.len() {
                end = k;
            }
        }
        if end == i && (i == 0 || !local(line[i - 1])) {
            let mut at = i;
            while line.get(at).is_some_and(|c| local(*c)) {
                at += 1;
            }
            if at > i && line.get(at) == Some(&'@') {
                let mut k = label(at + 1);
                let mut dots = 0;
                while k > at + 1 && line.get(k) == Some(&'.') && label(k + 1) > k + 1 {
                    k = label(k + 1);
                    dots += 1;
                }
                if dots > 0 {
                    end = k;
                }
            }
        }
        if end == i {
            i += 1;
            continue;
        }
        let mut stop = end;
        while stop > i && ".,;:!?)]}".contains(line[stop - 1]) {
            stop -= 1;
        }
        let url: String = line[i..stop].iter().collect();
        let web = ["http://", "https://", "www."].iter().any(|p| url.to_lowercase().starts_with(p));
        out.push((i, stop, if web { url } else { format!("mailto:{url}") }));
        i = end;
    }
    out
}

impl Page {
    pub fn new(lines: Vec<String>, colors: Option<Colors>, label: String) -> Self {
        let lines: Vec<Vec<char>> = lines.iter().map(|l| l.chars().collect()).collect();
        // each run of rows with block graphics is one group: the logo, a bar further down too. ■ alone is a bullet
        let logo: Vec<bool> = lines.iter().map(|l| l.iter().any(|&c| c != SQUARE && block(c).is_some())).collect();
        let mut group = vec![None; lines.len()];
        let mut top = None;
        for r in 0..=lines.len() {
            match (logo.get(r) == Some(&true), top) {
                (true, None) => top = Some(r),
                (false, Some(t)) => {
                    group[t..r].fill(Some((t, r)));
                    top = None;
                }
                _ => {}
            }
        }
        Page {
            cols: lines.iter().map(Vec::len).max().unwrap_or(0),
            links: lines.iter().map(|l| if colors.is_some() { Vec::new() } else { links(l) }).collect(),
            group,
            lines,
            colors,
            label,
            ..Default::default()
        }
    }

    /// Decode data into a page (ANSI art gets colours). text_only: without the art.
    pub fn fill(data: &[u8], enc: &str, text_only: bool) -> Self {
        let (text, label, width) = decode(data, enc);
        if is_ansi(&text) {
            let (lines, colors) = parse_ansi(&text, width);
            let label = format!("ANSI · {label}");
            if text_only {
                return Page::new(plain_lines(&clean_text(&lines, false)), None, label);
            }
            return Page::new(lines, Some(colors), label);
        }
        let lines = plain_lines(&text);
        // any line ending, also the \r alone of old Macs
        let rows = || text.replace("\r\n", "\n").split(['\n', '\r']).map(String::from).collect::<Vec<_>>();
        Page::new(if text_only { plain_lines(&clean_text(&rows(), false)) } else { lines }, None, label.into())
    }

    pub fn link_at(&self, r: usize, c: usize) -> Option<&str> {
        self.links.get(r)?.iter().find(|l| l.0 <= c && c < l.1).map(|l| l.2.as_str())
    }

    pub fn text(&self) -> Vec<String> {
        self.lines.iter().map(|l| l.iter().collect()).collect()
    }

    pub fn selected_text(&self, text_only: bool) -> String {
        let Some((a, b)) = self.sel else { return String::new() };
        let ((r0, c0), (r1, c1)) = (a.min(b), a.max(b));
        let parts: Vec<String> = (r0..=r1.min(self.lines.len().saturating_sub(1)))
            .map(|r| {
                let line = &self.lines[r];
                let (from, to) = (if r == r0 { c0 } else { 0 }, if r == r1 { c1 + 1 } else { line.len() });
                line[from.min(line.len())..to.min(line.len())].iter().collect()
            })
            .collect();
        let raw = parts.iter().map(|p| p.trim_end()).collect::<Vec<_>>().join("\n");
        // only art selected: that is what was meant, not nothing
        Some(clean_text(&parts, true)).filter(|t| text_only && !t.is_empty()).unwrap_or(raw)
    }

    /// Case-insensitive search; fills self.matches.
    pub fn find(&mut self, query: &str) {
        let low = |c: &char| c.to_lowercase().next().unwrap_or(*c);
        let q: Vec<char> = query.chars().map(|c| low(&c)).collect();
        self.matches.clear();
        self.current = 0;
        if q.is_empty() {
            return;
        }
        for (r, line) in self.lines.iter().enumerate() {
            let line: Vec<char> = line.iter().map(low).collect();
            self.matches.extend(line.windows(q.len()).enumerate().filter(|(_, w)| *w == &q[..]).map(|(i, _)| (r, i, i + q.len())));
        }
    }

    /// The marked cells of rows lo..hi as (row, a, b): with "copy text only" just what gets copied.
    pub fn selection_rows(&self, lo: usize, hi: usize, text_only: bool) -> Vec<(usize, usize, usize)> {
        let Some((a, b)) = self.sel else { return Vec::new() };
        let ((r0, c0), (r1, c1)) = (a.min(b), a.max(b));
        let mut out = Vec::new();
        for r in lo.max(r0)..hi.min(r1 + 1).min(self.lines.len()) {
            let line = &self.lines[r];
            let (mut from, mut to) = (if r == r0 { c0 } else { 0 }, if r == r1 { c1 + 1 } else { line.len().max(1) });
            if text_only {
                // from the first to the last text character, and no row that "text only" leaves out
                if clean_text(&[line.iter().collect::<String>()], false).is_empty() {
                    continue;
                }
                let text: Vec<usize> = (from..to.min(line.len())).filter(|&i| !line[i].is_whitespace() && !is_art(line[i])).collect();
                let (Some(first), Some(last)) = (text.first(), text.last()) else { continue };
                (from, to) = (*first, last + 1);
            }
            out.push((r, from, to));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str) -> Page {
        Page::fill(text.as_bytes(), "auto", false)
    }

    #[test]
    fn email_and_web_links() {
        assert_eq!(page("mail: group@example.org.").link_at(0, 7), Some("mailto:group@example.org"));
        assert_eq!(page("mail: group@example.org.").link_at(0, 23), None); // the full stop is not part of it
        assert_eq!(page("httpd@example.org").link_at(0, 0), Some("mailto:httpd@example.org")); // starts like a web link
        let p = page("WWW.GROUP.COM or HTTP://GROUP.COM"); // old NFOs write them in capitals
        assert_eq!((p.link_at(0, 0), p.link_at(0, 20)), (Some("WWW.GROUP.COM"), Some("HTTP://GROUP.COM")));
        assert_eq!(page("see (https://x.org/a?b=1). a@b a@b.").links[0], [(5, 24, "https://x.org/a?b=1".to_string())]);
        let start = std::time::Instant::now();
        page(&"a".repeat(200_000)); // one long line: the search for addresses once took the square of its length
        assert!(start.elapsed().as_secs() < 5);
    }

    #[test]
    fn groups_selection_and_search() {
        let mut p = page("██ ║ Title: x ║\n\n■ bullet\n▓▓▓\n▀▀\n   more");
        assert_eq!(p.group, [Some((0, 1)), None, None, Some((3, 5)), Some((3, 5)), None]); // ■ alone is no logo
        p.sel = Some(((0, 14), (0, 0))); // dragged backwards
        assert_eq!((p.selected_text(true).as_str(), p.selected_text(false).as_str()), ("Title: x", "██ ║ Title: x ║"));
        assert_eq!(p.selection_rows(0, 6, true), [(0, 5, 13)]);
        p.sel = Some(((0, 0), (5, 3)));
        assert_eq!(p.selection_rows(0, 6, true), [(0, 5, 13), (2, 2, 8), (5, 3, 4)]);
        assert_eq!(p.selection_rows(0, 6, false).len(), 6);
        p.sel = Some(((3, 0), (3, 1)));
        assert_eq!(p.selected_text(true), "▓▓"); // a selection of only art copies that
        p.find("TITLE");
        assert_eq!(p.matches, [(0, 5, 10)]);
        p.find("");
        assert!(p.matches.is_empty());
        assert_eq!(Page::fill("██ ║ Title: x ║\n▓▓▓\n   more".as_bytes(), "auto", true).text(), ["Title: x", "", "more"]);
        assert_eq!(Page::fill("Title: x\rmore text".as_bytes(), "auto", true).text(), ["Title: x", "more text"]); // old Mac line ends
    }
}
