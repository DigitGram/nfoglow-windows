// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! Translations: English source strings, one .po file per language (the files of NfoGlow for Linux, built into the
//! program). Missing translations fall back to English.
use std::collections::HashMap;
use std::sync::RwLock;

/// (code, native name, the .po file)
pub const LANGUAGES: [(&str, &str, &str); 10] = [
    ("en", "English", ""),
    ("de", "Deutsch", include_str!("../po/de.po")),
    ("es", "Español", include_str!("../po/es.po")),
    ("fr", "Français", include_str!("../po/fr.po")),
    ("it", "Italiano", include_str!("../po/it.po")),
    ("ja", "日本語", include_str!("../po/ja.po")),
    ("pl", "Polski", include_str!("../po/pl.po")),
    ("pt_BR", "Português (Brasil)", include_str!("../po/pt_BR.po")),
    ("uk", "Українська", include_str!("../po/uk.po")),
    ("zh_CN", "简体中文", include_str!("../po/zh_CN.po")),
];

static CURRENT: RwLock<Option<(&'static str, HashMap<String, String>)>> = RwLock::new(None);

/// Parse a .po file into {msgid: msgstr}.
/// shortcut: no msgctxt and no plural forms (no message needs them yet); fuzzy and untranslated entries are skipped.
fn load_po(text: &str) -> HashMap<String, String> {
    let mut messages = HashMap::new();
    for block in text.split("\n\n") {
        let (mut id, mut translated, mut fuzzy) = (None::<String>, None::<String>, false);
        let mut field = &mut id;
        for line in block.lines().map(str::trim) {
            let quoted = match line.split_once(' ') {
                _ if line.starts_with("#,") => {
                    fuzzy |= line.contains("fuzzy");
                    continue;
                }
                Some(("msgid", rest)) => {
                    field = &mut id;
                    rest
                }
                Some(("msgstr", rest)) => {
                    field = &mut translated;
                    rest
                }
                _ if line.starts_with('"') => line,
                _ => continue,
            };
            // PO string escapes are JSON-compatible
            if let Ok(part) = serde_json::from_str::<String>(quoted) {
                field.get_or_insert_default().push_str(&part);
            }
        }
        // the header has an empty msgid: left out, or an empty text would show it
        if let (false, Some(id), Some(translated)) = (fuzzy, id.filter(|i| !i.is_empty()), translated.filter(|t| !t.is_empty())) {
            messages.insert(id, translated);
        }
    }
    messages
}

/// The language for a locale such as "de_DE.UTF-8" or "de-DE": that one, or one of the same tongue (pt_PT gets
/// pt_BR, zh_TW gets zh_CN: better than English), else English.
fn for_locale(locale: &str) -> &'static str {
    let name = locale.split(['.', '@']).next().unwrap_or("").replace('-', "_");
    let short = name.split('_').next().unwrap_or("");
    let code = |c: &str| LANGUAGES.iter().map(|l| l.0).find(|l| *l == c);
    code(&name).or(code(short)).or(LANGUAGES.iter().map(|l| l.0).find(|l| l.split('_').next() == Some(short))).unwrap_or("en")
}

/// Select a language code, or "auto" to follow the system. Returns the code now in use.
pub fn set_language(code: &str) -> &'static str {
    let code = if code == "auto" { for_locale(&sys_locale::get_locale().unwrap_or_default()) } else { for_locale(code) };
    let po = LANGUAGES.iter().find(|l| l.0 == code).map_or("", |l| l.2);
    *CURRENT.write().unwrap() = Some((code, load_po(po)));
    code
}

/// The code of the language in use.
pub fn current() -> &'static str {
    CURRENT.read().unwrap().as_ref().map_or("en", |c| c.0)
}

pub fn tr(message: &str) -> String {
    let current = CURRENT.read().unwrap();
    current.as_ref().and_then(|c| c.1.get(message)).cloned().unwrap_or_else(|| message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_translates_every_message() {
        let all = load_po(LANGUAGES[1].2).len();
        assert!(all > 240);
        for (code, _, po) in &LANGUAGES[1..] {
            let messages = load_po(po);
            assert!(messages.len() == all && messages.values().all(|m| !m.is_empty()), "{code}: {} of {all}", messages.len());
        }
        let de = load_po(LANGUAGES[1].2);
        assert_eq!((de["Dark"].as_str(), de["Cannot open {name}: {error}"].contains("{name}")), ("Dunkel", true));
        assert!(!de.contains_key("")); // no header
        let po = "msgid \"a\"\nmsgstr \"\"\n\"b \\\"c\\\"\\n\"\n\n#, fuzzy\nmsgid \"x\"\nmsgstr \"y\"\n\nmsgid \"u\"\nmsgstr \"\"\n";
        assert_eq!(load_po(po), HashMap::from([("a".to_string(), "b \"c\"\n".to_string())]));
    }

    #[test]
    fn the_language_follows_the_locale() {
        for (locale, code) in [("de_DE.UTF-8", "de"), ("de-AT", "de"), ("pt_PT", "pt_BR"), ("pt-BR", "pt_BR"), ("zh_TW", "zh_CN"), ("ja", "ja"),
            ("C", "en"), ("", "en"), ("nl_NL", "en"), ("en_US", "en"), ("uk_UA@x", "uk")] {
            assert_eq!(for_locale(locale), code, "{locale}");
        }
        // one test for the global: tests run side by side
        assert_eq!((set_language("de"), tr("Dark"), tr("no such message")), ("de", "Dunkel".to_string(), "no such message".to_string()));
        assert_eq!((set_language("en"), tr("Dark")), ("en", "Dark".to_string()));
    }
}
