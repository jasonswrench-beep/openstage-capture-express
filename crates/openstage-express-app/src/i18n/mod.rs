// SPDX-License-Identifier: Apache-2.0
//! The language of the program's own words (spec A4, milestone 8): English, German and French. It is not the language of a guide, whose
//! step titles and exports have their own setting (`TitleLanguage`).
//!
//! The words are written in English in the code, and `tr("Save")` gives the German or French for them when the person has chosen one,
//! and the English itself when the words are not in the catalog, so a missing translation is never a missing word. A text with something
//! in it that changes (`trf("Exported {1} to {2}.", &[&n, &folder])`) names its places `{1}`, `{2}` in the order the English has them, and
//! a translation may put them in any order. What the program says in its status line is translated whole after it is made
//! ([`translate_text`]), by matching it against the catalog's texts that have places in them.
//!
//! The catalogs are `de.rs` and `fr.rs`: a table of English and its translation. `tools/check_rules.py` fails if a text wrapped in `tr(`
//! or `trf(` is missing from either, or a translation leaves out one of the places the English has. The translations were written
//! for this program and have not been read by someone who writes either language as their own (`TESTING-TODO.md`).
//!
//! The language is kept for each thread, because the window is drawn by one thread and the tests each run in a thread of their own
//! (so one test in German cannot change the words another test is reading).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::fmt::Display;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

mod de;
mod fr;

/// The language of the program's words. A word in the settings file that this program does not know means English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Lang {
    De,
    Fr,
    /// Last in the list, because serde takes any other word to be this one.
    #[default]
    #[serde(other)]
    En,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::De, Lang::Fr];

    /// The language's name in itself, so that a person who cannot read the rest can find it.
    pub fn own_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Fr => "Fran\u{e7}ais",
        }
    }

    /// The language tag (`en`, `de`, `fr`).
    #[cfg(test)]
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Fr => "fr",
        }
    }

    /// The language for a Windows language name such as `de-DE` or `fr-CA`: German or French when it starts with `de` or `fr`, else English.
    pub fn from_windows(name: &str) -> Lang {
        let lower = name.trim().to_ascii_lowercase();
        let first = lower.split(['-', '_']).next().unwrap_or("");
        match first {
            "de" => Lang::De,
            "fr" => Lang::Fr,
            _ => Lang::En,
        }
    }

    fn entries(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::De => de::ENTRIES,
            Lang::Fr => fr::ENTRIES,
        }
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::En) };
    static MISSING: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

/// Use `lang` for the words drawn from now on, on this thread.
pub fn set_language(lang: Lang) {
    CURRENT.with(|c| c.set(lang));
}

/// The language in use on this thread.
pub fn language() -> Lang {
    CURRENT.with(Cell::get)
}

/// The texts asked for in German or French that the catalog does not have, since the last call. For tests.
#[cfg(test)]
pub fn take_missing() -> Vec<String> {
    MISSING.with(|m| std::mem::take(&mut *m.borrow_mut())).into_iter().collect()
}

/// One text of the catalog that has places in it, ready to be matched against a finished sentence.
struct Pattern {
    /// The words between the places, one more than there are places.
    parts: Vec<String>,
    translation: &'static str,
    /// How many letters of the English are fixed words (the most specific pattern wins when two match).
    fixed: usize,
}

struct Catalog {
    exact: HashMap<&'static str, &'static str>,
    patterns: Vec<Pattern>,
}

/// The places in an English text: `{}`, `{name}`, `{1}`, `{:.1}` and the like. `{{` and `}}` are the braces themselves.
fn split_places(english: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut chars = english.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                parts.last_mut().unwrap().push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                parts.last_mut().unwrap().push('}');
            }
            '{' => {
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                }
                parts.push(String::new());
            }
            c => parts.last_mut().unwrap().push(c),
        }
    }
    parts
}

fn build(lang: Lang) -> Catalog {
    let mut exact = HashMap::new();
    let mut patterns = Vec::new();
    for (english, translation) in lang.entries() {
        let parts = split_places(english);
        if parts.len() == 1 && !english.contains("{{") && !english.contains("}}") {
            exact.insert(*english, *translation);
        } else {
            // A text with places is found as it is written too (for `trf`), and by its shape (for a finished sentence).
            exact.insert(*english, *translation);
            if parts.len() > 1 {
                let fixed = parts.iter().map(|p| p.chars().count()).sum();
                patterns.push(Pattern { parts, translation, fixed });
            }
        }
    }
    patterns.sort_by_key(|p| std::cmp::Reverse(p.fixed));
    Catalog { exact, patterns }
}

fn catalog(lang: Lang) -> Option<&'static Catalog> {
    static DE: OnceLock<Catalog> = OnceLock::new();
    static FR: OnceLock<Catalog> = OnceLock::new();
    match lang {
        Lang::En => None,
        Lang::De => Some(DE.get_or_init(|| build(Lang::De))),
        Lang::Fr => Some(FR.get_or_init(|| build(Lang::Fr))),
    }
}

/// Put `args` in the places `{1}`, `{2}`... of `text`. A place with no argument is left as it is.
fn fill(text: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        let mut closed = false;
        let mut look = chars.clone();
        for d in look.by_ref() {
            if d.is_ascii_digit() {
                digits.push(d);
            } else {
                closed = d == '}';
                break;
            }
        }
        match digits.parse::<usize>() {
            Ok(n) if closed && n >= 1 && n <= args.len() => {
                out.push_str(&args[n - 1]);
                chars = look;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Marks English words that are kept in a table and translated where they are shown (`tr(table_entry)`): it changes nothing, and it lets
/// `tools/check_rules.py` find the words and see that both catalogs have them.
pub const fn k(english: &'static str) -> &'static str {
    english
}

/// The words in the language in use. English (or words that are not in the catalog) come back as they are.
pub fn tr(english: &str) -> String {
    let lang = language();
    let Some(catalog) = catalog(lang) else { return english.to_string() };
    match catalog.exact.get(english) {
        Some(found) => (*found).to_string(),
        None => {
            if english.chars().any(char::is_alphabetic) {
                MISSING.with(|m| m.borrow_mut().insert(english.to_string()));
            }
            english.to_string()
        }
    }
}

/// A text with places in it (`{1}`, `{2}`), translated and filled with `args`.
pub fn trf(english: &str, args: &[&dyn Display]) -> String {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let lang = language();
    let template = match catalog(lang) {
        None => english.to_string(),
        Some(catalog) => match catalog.exact.get(english) {
            Some(found) => (*found).to_string(),
            None => {
                MISSING.with(|m| m.borrow_mut().insert(english.to_string()));
                english.to_string()
            }
        },
    };
    fill(&template, &args)
}

/// [`translate_text`] for a `String`, so that `tt(format!("Moved {} steps.", n))` reads as one thing.
pub fn tt(text: String) -> String {
    translate_text(&text)
}

/// A finished sentence in the language in use: found as a whole in the catalog, or by its shape, with what filled its places carried over
/// as it was. A sentence the catalog does not know is returned as it is (English), and noted for the tests.
pub fn translate_text(text: &str) -> String {
    let Some(catalog) = catalog(language()) else { return text.to_string() };
    if let Some(found) = catalog.exact.get(text) {
        return (*found).to_string();
    }
    for pattern in &catalog.patterns {
        if let Some(captures) = match_pattern(pattern, text) {
            return fill(pattern.translation, &captures);
        }
    }
    if text.chars().any(char::is_alphabetic) {
        MISSING.with(|m| m.borrow_mut().insert(text.to_string()));
    }
    text.to_string()
}

/// What fills the places of `pattern` in `text`, if `text` has its shape.
fn match_pattern(pattern: &Pattern, text: &str) -> Option<Vec<String>> {
    let parts = &pattern.parts;
    let (first, last) = (parts.first()?, parts.last()?);
    if text.len() < first.len() + last.len() || !text.starts_with(first.as_str()) || !text.ends_with(last.as_str()) {
        return None;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    let mut captures = Vec::with_capacity(parts.len() - 1);
    for (i, part) in parts[1..parts.len() - 1].iter().enumerate() {
        let _ = i;
        if part.is_empty() {
            // Two places side by side: the first takes one letter, so that the shape still fits.
            let take = rest.chars().next()?.len_utf8();
            captures.push(rest[..take].to_string());
            rest = &rest[take..];
            continue;
        }
        let at = rest.find(part.as_str())?;
        captures.push(rest[..at].to_string());
        rest = &rest[at + part.len()..];
    }
    captures.push(rest.to_string());
    Some(captures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_found_in_every_notation_the_code_uses() {
        assert_eq!(split_places("Save"), ["Save"]);
        assert_eq!(split_places("Deleted {} steps."), ["Deleted ", " steps."]);
        assert_eq!(split_places("Moved {title} {way}."), ["Moved ", " ", "."]);
        assert_eq!(split_places("{:.1} MB, {0}"), ["", " MB, ", ""]);
        assert_eq!(split_places("a {{b}} c"), ["a {b} c"]);
    }

    #[test]
    fn places_are_filled_in_the_order_the_translation_wants() {
        let args = ["3".to_string(), "Frank".to_string()];
        assert_eq!(fill("{2} hat {1} Dateien", &args), "Frank hat 3 Dateien");
        assert_eq!(fill("{3} {0} {x} {", &args), "{3} {0} {x} {", "what is not a place stays");
        assert_eq!(fill("{1}{1}", &args), "33");
    }

    #[test]
    fn a_sentence_is_matched_by_its_shape_and_carries_what_filled_its_places() {
        let p = Pattern {
            parts: split_places("Deleted {} steps. Undo brings them back."),
            translation: "{1} Schritte gel\u{f6}scht.",
            fixed: 30,
        };
        assert_eq!(match_pattern(&p, "Deleted 4 steps. Undo brings them back."), Some(vec!["4".to_string()]));
        assert_eq!(match_pattern(&p, "Deleted steps. Undo brings them back."), None);
        assert_eq!(match_pattern(&p, "Undone."), None);
        let two = Pattern { parts: split_places("Moved {} {}."), translation: "", fixed: 8 };
        assert_eq!(
            match_pattern(&two, "Moved Click \"Save\" up."),
            Some(vec!["Click".to_string(), "\"Save\" up".to_string()])
        );
    }

    #[test]
    fn english_comes_back_as_it_is_and_a_place_is_filled_in_english_too() {
        set_language(Lang::En);
        assert_eq!(tr("Anything at all"), "Anything at all");
        assert_eq!(trf("{1} of {2}", &[&3, &"five"]), "3 of five");
        assert_eq!(translate_text("Whatever {} was."), "Whatever {} was.");
        assert!(take_missing().is_empty(), "English never has a missing word");
    }

    #[test]
    fn a_word_the_catalog_does_not_have_is_english_and_noted() {
        set_language(Lang::De);
        let _ = take_missing();
        assert_eq!(tr("A sentence nobody has translated yet"), "A sentence nobody has translated yet");
        assert_eq!(take_missing(), ["A sentence nobody has translated yet"]);
        assert_eq!(tr("1234 -- 5678"), "1234 -- 5678");
        assert!(take_missing().is_empty(), "something with no letters in it is not a word");
        set_language(Lang::En);
    }

    #[test]
    fn the_language_is_kept_for_each_thread() {
        set_language(Lang::Fr);
        let other = std::thread::spawn(language).join().unwrap();
        assert_eq!(other, Lang::En);
        assert_eq!(language(), Lang::Fr);
        set_language(Lang::En);
    }

    #[test]
    fn windows_language_names_give_german_french_or_english() {
        for (name, lang) in [
            ("de-DE", Lang::De),
            ("de", Lang::De),
            ("DE-at", Lang::De),
            ("fr-CA", Lang::Fr),
            ("fr_FR", Lang::Fr),
            ("en-US", Lang::En),
            ("ja-JP", Lang::En),
            ("", Lang::En),
            ("deu", Lang::En),
        ] {
            assert_eq!(Lang::from_windows(name), lang, "{name}");
        }
    }

    #[test]
    fn the_settings_file_word_for_a_language_is_the_code_and_an_unknown_one_means_english() {
        assert_eq!(serde_json::to_string(&Lang::De).unwrap(), "\"de\"");
        assert_eq!(serde_json::from_str::<Lang>("\"fr\"").unwrap(), Lang::Fr);
        assert_eq!(serde_json::from_str::<Lang>("\"klingon\"").unwrap(), Lang::En);
        for lang in Lang::ALL {
            assert_eq!(Lang::from_windows(lang.code()), lang);
        }
    }
}
