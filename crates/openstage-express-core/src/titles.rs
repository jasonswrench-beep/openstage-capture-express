// SPDX-License-Identifier: Apache-2.0
//! Smart step titles (spec section 8.3): the title a step gets before the author touches it, such as `Click "Save"`.
//!
//! The verb follows the kind of control that was clicked, and it is deliberately cautious: when the program is not sure
//! it says `Click`. There is no `Enter` verb, because `Enter "Email"` reads as "type the word Email" in several
//! languages. Names are tidied (see [`clean_name`]) before they go into a title.
//!
//! The English strings are the spec's. The German and French ones are the spec's proposals, and the ones for middle
//! clicks and for "here" with a verb other than Click are this program's own proposals; a native speaker should check
//! them before release (see `TESTING-TODO.md`).

use crate::model::Button;

/// How many characters a name may have in a title, counting the ellipsis that shows it was cut.
pub const MAX_NAME_CHARS: usize = 80;

/// The language of step titles. Independent of the language of the program's own windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    German,
    French,
}

impl Language {
    /// Read `en`, `de-AT`, `fr_CA` and the like. Anything else is English.
    pub fn from_code(code: &str) -> Language {
        let lower = code.to_ascii_lowercase();
        match lower.split(['-', '_']).next().unwrap_or("") {
            "de" => Language::German,
            "fr" => Language::French,
            _ => Language::English,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::German => "de",
            Language::French => "fr",
        }
    }
}

/// What the person did to the control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Click,
    RightClick,
    MiddleClick,
    DoubleClick,
    Select,
}

impl Verb {
    /// The verb for a plain click on this kind of control (Capture's English role names). Dropdowns, combo boxes and
    /// radio buttons are `Select`; everything else, including check boxes, is `Click`. A check box stays `Click`
    /// because whether it is now on or off cannot be read reliably (`Check` and `Uncheck` come later).
    pub fn for_role(role: &str) -> Verb {
        match role {
            "combo box" | "radio button" | "split button" => Verb::Select,
            _ => Verb::Click,
        }
    }

    pub fn for_button(button: Button) -> Verb {
        match button {
            Button::Left => Verb::Click,
            Button::Right => Verb::RightClick,
            Button::Middle => Verb::MiddleClick,
        }
    }
}

/// Characters that can hide or reorder text (zero-width marks and the bidirectional controls).
fn is_invisible_format(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// Tidy a name for a title: control characters and invisible format characters are removed (a tab or a line break
/// becomes a space), runs of white space become one space, straight double quotes become single quotes so the title's
/// own quotes stay unambiguous, and anything over 80 characters is cut with an ellipsis.
pub fn clean_name(raw: &str) -> String {
    let mut spaced = String::with_capacity(raw.len());
    for c in raw.chars() {
        if c == '"' {
            spaced.push('\'');
        } else if c.is_whitespace() || c == '\t' || c == '\n' || c == '\r' {
            spaced.push(' ');
        } else if c.is_control() || is_invisible_format(c) {
            continue;
        } else {
            spaced.push(c);
        }
    }
    let collapsed = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > MAX_NAME_CHARS {
        let cut: String = collapsed.chars().take(MAX_NAME_CHARS - 1).collect();
        format!("{}\u{2026}", cut.trim_end())
    } else {
        collapsed
    }
}

/// The title for a click with this verb on a control with this name. A name that is empty once tidied gives the
/// "here" form instead.
pub fn click_title(language: Language, verb: Verb, name: &str) -> String {
    use Language::*;
    use Verb::*;
    let name = clean_name(name);
    if name.is_empty() {
        let text = match (language, verb) {
            (English, Click | Select) => "Click here",
            (German, Click | Select) => "Hier klicken",
            (French, Click | Select) => "Cliquez ici",
            (English, RightClick) => "Right-click here",
            (German, RightClick) => "Hier mit der rechten Maustaste klicken",
            (French, RightClick) => "Faites un clic droit ici",
            (English, MiddleClick) => "Middle-click here",
            (German, MiddleClick) => "Hier mit der mittleren Maustaste klicken",
            (French, MiddleClick) => "Faites un clic central ici",
            (English, DoubleClick) => "Double-click here",
            (German, DoubleClick) => "Hier doppelklicken",
            (French, DoubleClick) => "Double-cliquez ici",
        };
        return text.to_string();
    }
    let (before, after) = match (language, verb) {
        (English, Click) => ("Click ", ""),
        (German, Click) => ("Klicken Sie auf ", ""),
        (French, Click) => ("Cliquez sur ", ""),
        (English, RightClick) => ("Right-click ", ""),
        (German, RightClick) => ("Klicken Sie mit der rechten Maustaste auf ", ""),
        (French, RightClick) => ("Faites un clic droit sur ", ""),
        (English, MiddleClick) => ("Middle-click ", ""),
        (German, MiddleClick) => ("Klicken Sie mit der mittleren Maustaste auf ", ""),
        (French, MiddleClick) => ("Faites un clic central sur ", ""),
        (English, DoubleClick) => ("Double-click ", ""),
        (German, DoubleClick) => ("Doppelklicken Sie auf ", ""),
        (French, DoubleClick) => ("Double-cliquez sur ", ""),
        (English, Select) => ("Select ", ""),
        (German, Select) => ("W\u{e4}hlen Sie ", ""),
        (French, Select) => ("S\u{e9}lectionnez ", ""),
    };
    format!("{before}\"{name}\"{after}")
}

/// `Type "text"` for a step where the text of a field was read when typing finished.
pub fn type_title(language: Language, text: &str) -> String {
    let text = clean_name(text);
    if text.is_empty() {
        return type_here_title(language).to_string();
    }
    match language {
        Language::English => format!("Type \"{text}\""),
        Language::German => format!("Geben Sie \"{text}\" ein"),
        Language::French => format!("Saisissez \"{text}\""),
    }
}

/// `Type in "Email"` for a step where text was typed but could not be read back (never guess the text).
pub fn type_in_title(language: Language, field_name: &str) -> String {
    let name = clean_name(field_name);
    if name.is_empty() {
        return type_here_title(language).to_string();
    }
    match language {
        Language::English => format!("Type in \"{name}\""),
        Language::German => format!("Geben Sie Text in \"{name}\" ein"),
        Language::French => format!("Saisissez du texte dans \"{name}\""),
    }
}

fn type_here_title(language: Language) -> &'static str {
    match language {
        Language::English => "Type here",
        Language::German => "Hier eingeben",
        Language::French => "Saisissez ici",
    }
}

/// The title of a step made by the "capture now" hotkey.
pub fn screenshot_title(language: Language) -> &'static str {
    match language {
        Language::English => "Screenshot",
        Language::German => "Bildschirmfoto",
        Language::French => "Capture d'\u{e9}cran",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROLES: &[&str] = &[
        "button",
        "calendar",
        "check box",
        "combo box",
        "edit",
        "link",
        "image",
        "list item",
        "list",
        "menu",
        "menu bar",
        "menu item",
        "progress bar",
        "radio button",
        "scroll bar",
        "slider",
        "spinner",
        "status bar",
        "tab",
        "tab item",
        "text",
        "tool bar",
        "tool tip",
        "tree",
        "tree item",
        "custom",
        "group",
        "thumb",
        "data grid",
        "data item",
        "document",
        "split button",
        "window",
        "pane",
        "header",
        "header item",
        "table",
        "title bar",
        "separator",
        "semantic zoom",
        "app bar",
        "unknown",
    ];

    #[test]
    fn the_verb_follows_the_kind_of_control() {
        for role in ROLES {
            let expected = match *role {
                "combo box" | "radio button" | "split button" => Verb::Select,
                _ => Verb::Click,
            };
            assert_eq!(Verb::for_role(role), expected, "{role}");
        }
        assert_eq!(Verb::for_role("check box"), Verb::Click, "check boxes stay Click");
        assert_eq!(Verb::for_role("edit"), Verb::Click, "a field you only click is a Click");
        assert_eq!(Verb::for_role("something new"), Verb::Click, "unsure means Click");
    }

    #[test]
    fn titles_in_english_match_the_spec() {
        let l = Language::English;
        assert_eq!(click_title(l, Verb::Click, "Save"), "Click \"Save\"");
        assert_eq!(click_title(l, Verb::RightClick, "Save"), "Right-click \"Save\"");
        assert_eq!(click_title(l, Verb::DoubleClick, "Save"), "Double-click \"Save\"");
        assert_eq!(click_title(l, Verb::MiddleClick, "Save"), "Middle-click \"Save\"");
        assert_eq!(click_title(l, Verb::Select, "Country"), "Select \"Country\"");
        assert_eq!(type_title(l, "jason@example.com"), "Type \"jason@example.com\"");
        assert_eq!(type_in_title(l, "Email"), "Type in \"Email\"");
        assert_eq!(click_title(l, Verb::Click, ""), "Click here");
        assert_eq!(screenshot_title(l), "Screenshot");
    }

    #[test]
    fn titles_in_german_and_french_match_the_spec() {
        let de = Language::German;
        assert_eq!(click_title(de, Verb::Click, "X"), "Klicken Sie auf \"X\"");
        assert_eq!(click_title(de, Verb::RightClick, "X"), "Klicken Sie mit der rechten Maustaste auf \"X\"");
        assert_eq!(click_title(de, Verb::DoubleClick, "X"), "Doppelklicken Sie auf \"X\"");
        assert_eq!(click_title(de, Verb::Select, "X"), "W\u{e4}hlen Sie \"X\"");
        assert_eq!(type_title(de, "T"), "Geben Sie \"T\" ein");
        assert_eq!(type_in_title(de, "X"), "Geben Sie Text in \"X\" ein");
        assert_eq!(click_title(de, Verb::Click, ""), "Hier klicken");
        assert_eq!(screenshot_title(de), "Bildschirmfoto");
        let fr = Language::French;
        assert_eq!(click_title(fr, Verb::Click, "X"), "Cliquez sur \"X\"");
        assert_eq!(click_title(fr, Verb::RightClick, "X"), "Faites un clic droit sur \"X\"");
        assert_eq!(click_title(fr, Verb::DoubleClick, "X"), "Double-cliquez sur \"X\"");
        assert_eq!(click_title(fr, Verb::Select, "X"), "S\u{e9}lectionnez \"X\"");
        assert_eq!(type_title(fr, "T"), "Saisissez \"T\"");
        assert_eq!(type_in_title(fr, "X"), "Saisissez du texte dans \"X\"");
        assert_eq!(click_title(fr, Verb::Click, ""), "Cliquez ici");
        assert_eq!(screenshot_title(fr), "Capture d'\u{e9}cran");
    }

    #[test]
    fn every_verb_in_every_language_gives_a_named_and_a_nameless_title() {
        for language in [Language::English, Language::German, Language::French] {
            for verb in [Verb::Click, Verb::RightClick, Verb::MiddleClick, Verb::DoubleClick, Verb::Select] {
                let named = click_title(language, verb, "Save");
                assert!(named.contains("\"Save\""), "{language:?} {verb:?}: {named}");
                let nameless = click_title(language, verb, "  ");
                assert!(!nameless.contains('"') && !nameless.is_empty(), "{language:?} {verb:?}: {nameless}");
                assert_ne!(named, nameless);
            }
            assert!(!type_in_title(language, "").contains('"'));
            assert!(!type_title(language, "\t").contains('"'));
            assert!(!screenshot_title(language).is_empty());
        }
    }

    #[test]
    fn no_title_string_has_an_em_dash_or_a_trailing_space() {
        for language in [Language::English, Language::German, Language::French] {
            for verb in [Verb::Click, Verb::RightClick, Verb::MiddleClick, Verb::DoubleClick, Verb::Select] {
                for name in ["", "x"] {
                    let title = click_title(language, verb, name);
                    assert!(!title.contains('\u{2014}') && title == title.trim(), "{title:?}");
                }
            }
        }
    }

    #[test]
    fn names_are_tidied() {
        assert_eq!(clean_name("  Save   as\t\u{2026} \n"), "Save as \u{2026}");
        assert_eq!(clean_name("Say \"hi\""), "Say 'hi'");
        assert_eq!(clean_name("a\u{0}b\u{7}c\u{1b}d"), "abcd");
        assert_eq!(clean_name("a\u{200B}b\u{202E}c\u{FEFF}d"), "abcd", "invisible and reordering marks are removed");
        assert_eq!(clean_name("line one\r\nline two"), "line one line two");
        assert_eq!(clean_name(""), "");
        assert_eq!(clean_name("\u{0}\u{7}"), "");
        assert_eq!(clean_name("caf\u{e9} \u{65e5}\u{672c}\u{8a9e}"), "caf\u{e9} \u{65e5}\u{672c}\u{8a9e}");
    }

    #[test]
    fn long_names_are_cut_with_an_ellipsis_at_80_characters() {
        let exactly = "x".repeat(80);
        assert_eq!(clean_name(&exactly), exactly);
        let long = "y".repeat(81);
        let cut = clean_name(&long);
        assert_eq!(cut.chars().count(), 80);
        assert!(cut.ends_with('\u{2026}') && cut.starts_with("yyy"));
        let words = format!("{} tail", "word ".repeat(30));
        let cut = clean_name(&words);
        assert!(cut.chars().count() <= 80 && cut.ends_with('\u{2026}') && !cut.contains(" \u{2026}"), "{cut}");
        let wide = "\u{65e5}".repeat(200);
        assert_eq!(clean_name(&wide).chars().count(), 80, "counted in characters, not bytes");
    }

    #[test]
    fn names_with_quotes_and_odd_characters_stay_inside_their_quotes() {
        let title = click_title(Language::English, Verb::Click, "Say \"hi\"\u{0}\u{202E}");
        assert_eq!(title, "Click \"Say 'hi'\"");
        assert_eq!(title.matches('"').count(), 2);
    }

    #[test]
    fn language_codes() {
        assert_eq!(Language::from_code("en"), Language::English);
        assert_eq!(Language::from_code("de"), Language::German);
        assert_eq!(Language::from_code("DE-at"), Language::German);
        assert_eq!(Language::from_code("fr_CA"), Language::French);
        assert_eq!(Language::from_code("ja"), Language::English);
        assert_eq!(Language::from_code(""), Language::English);
        for language in [Language::English, Language::German, Language::French] {
            assert_eq!(Language::from_code(language.code()), language);
        }
    }

    #[test]
    fn the_button_decides_the_verb() {
        assert_eq!(Verb::for_button(Button::Left), Verb::Click);
        assert_eq!(Verb::for_button(Button::Right), Verb::RightClick);
        assert_eq!(Verb::for_button(Button::Middle), Verb::MiddleClick);
    }
}
