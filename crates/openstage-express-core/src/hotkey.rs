// SPDX-License-Identifier: Apache-2.0
//! The global hotkeys as a person chooses them (spec section 8.7, and C1 and C7). This file knows nothing of Windows: it turns what
//! was typed ("ctrl + shift + F7") into a key, says in words why a key cannot be used, and refuses the choices that would take a key
//! from every other program. Registering the keys with Windows is in `openstage-express-windows`.
//!
//! A hotkey is typed as text rather than picked by pressing the keys. A box that waits for a key press has to take the keyboard from
//! the person, which is a trap for a keyboard user and unreadable for a screen reader; a text box works the same for everyone.

use serde::{Deserialize, Serialize, Serializer};

/// What a hotkey does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    StartOrFinish,
    PauseOrResume,
    CaptureNow,
}

impl HotkeyAction {
    pub const ALL: [HotkeyAction; 3] =
        [HotkeyAction::StartOrFinish, HotkeyAction::PauseOrResume, HotkeyAction::CaptureNow];

    /// For example "start or finish", to finish "Ctrl+Shift+F7 ...".
    pub fn words(self) -> &'static str {
        match self {
            HotkeyAction::StartOrFinish => "start or finish",
            HotkeyAction::PauseOrResume => "pause or resume",
            HotkeyAction::CaptureNow => "capture now",
        }
    }

    /// The name of its box on the Settings screen.
    pub fn label(self) -> &'static str {
        match self {
            HotkeyAction::StartOrFinish => "Start or finish",
            HotkeyAction::PauseOrResume => "Pause or resume",
            HotkeyAction::CaptureNow => "Capture now",
        }
    }
}

/// A key with the modifier keys held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// The virtual-key code of the key itself (Windows' numbers for letters, digits and function keys).
    pub vk: u32,
}

const VK_F1: u32 = 0x70;
const VK_F12: u32 = 0x7B;
const VK_F24: u32 = 0x87;

fn is_function_key(vk: u32) -> bool {
    (VK_F1..=VK_F24).contains(&vk)
}

fn is_character_key(vk: u32) -> bool {
    matches!(vk, 0x30..=0x39 | 0x41..=0x5A)
}

impl KeyCombo {
    /// Ctrl+Shift with a function key (`n` is 1 to 24).
    pub const fn ctrl_shift_f(n: u32) -> KeyCombo {
        KeyCombo { ctrl: true, alt: false, shift: true, win: false, vk: 0x6F + n }
    }

    fn modifier_count(&self) -> usize {
        [self.ctrl, self.alt, self.shift, self.win].into_iter().filter(|m| *m).count()
    }

    /// For example `Ctrl+Shift+F7`. The modifiers come in the order Ctrl, Alt, Shift, Windows.
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.alt {
            parts.push("Alt".into());
        }
        if self.shift {
            parts.push("Shift".into());
        }
        if self.win {
            parts.push("Windows".into());
        }
        parts.push(match self.vk {
            vk if is_function_key(vk) => format!("F{}", vk - 0x6F),
            vk if is_character_key(vk) => ((vk as u8) as char).to_string(),
            other => format!("key {other:#x}"),
        });
        parts.join("+")
    }

    /// Read a key written as words: `Ctrl+Shift+F7`, `ctrl + alt + k`, `Windows+Shift+5`. Capitals and spaces do not matter. The
    /// answer is a key that has not yet been checked with [`KeyCombo::check`].
    pub fn parse(text: &str) -> Result<KeyCombo, String> {
        let example = "Write the keys joined with plus signs, for example Ctrl+Shift+F7.";
        if text.trim().is_empty() {
            return Err(format!("Nothing is written there. {example}"));
        }
        let mut combo = KeyCombo { ctrl: false, alt: false, shift: false, win: false, vk: 0 };
        for part in text.split('+') {
            let word = part.trim();
            if word.is_empty() {
                return Err(format!("There is an empty part between two plus signs. {example}"));
            }
            let lower = word.to_ascii_lowercase();
            let slot = match lower.as_str() {
                "ctrl" | "control" => Some((&mut combo.ctrl, "Ctrl")),
                "alt" => Some((&mut combo.alt, "Alt")),
                "shift" => Some((&mut combo.shift, "Shift")),
                "win" | "windows" => Some((&mut combo.win, "Windows")),
                _ => None,
            };
            if let Some((flag, name)) = slot {
                if *flag {
                    return Err(format!("{name} is written twice. {example}"));
                }
                *flag = true;
                continue;
            }
            let vk = key_from_word(&lower).ok_or_else(|| {
                format!("\"{word}\" is not a key Express can use. Use a letter, a digit or F1 to F24. {example}")
            })?;
            if combo.vk != 0 {
                return Err(format!(
                    "There are two keys, {} and {word}. Only one key can go with Ctrl, Alt, Shift and Windows.",
                    combo.key_name()
                ));
            }
            combo.vk = vk;
        }
        if combo.vk == 0 {
            return Err(format!("There is no key, only Ctrl, Alt, Shift or Windows. {example}"));
        }
        Ok(combo)
    }

    fn key_name(&self) -> String {
        KeyCombo { ctrl: false, alt: false, shift: false, win: false, vk: self.vk }.describe()
    }

    /// Whether Express may take this key from every other program, and if not, why, in words that say what to do.
    pub fn check(&self) -> Result<(), String> {
        let name = self.describe();
        if !is_function_key(self.vk) && !is_character_key(self.vk) {
            return Err(format!("{name} is not a key Express can use. Use a letter, a digit or F1 to F24."));
        }
        if self.vk == VK_F12 {
            return Err(format!("{name} is not allowed: Windows keeps F12 for debuggers. Choose another key."));
        }
        let count = self.modifier_count();
        if count == 0 {
            return Err(format!(
                "{name} has no Ctrl, Alt, Shift or Windows with it. A key on its own would stop working for typing in every other program."
            ));
        }
        if self.ctrl && self.alt && is_character_key(self.vk) {
            return Err(format!(
                "{name} would stop some keyboards typing characters such as @ and the euro sign, which they type with Ctrl and Alt together (AltGr). Choose another."
            ));
        }
        if count == 1 && is_character_key(self.vk) {
            return Err(format!(
                "{name} is used by too many programs, or (with Shift) types a capital letter. Add a second key such as Shift, or use a function key."
            ));
        }
        let reserved = [
            (false, true, false, 0x73, "close the window"),
            (true, false, false, 0x73, "close a tab"),
            (false, false, true, 0x79, "open the context menu"),
        ];
        for (ctrl, alt, shift, vk, does) in reserved {
            if self.vk == vk && self.ctrl == ctrl && self.alt == alt && self.shift == shift && !self.win {
                return Err(format!("{name} is how Windows and most programs {does}. Choose another."));
            }
        }
        Ok(())
    }
}

fn key_from_word(lower: &str) -> Option<u32> {
    let mut chars = lower.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        return match first {
            'a'..='z' => Some(first.to_ascii_uppercase() as u32),
            '0'..='9' => Some(first as u32),
            _ => None,
        };
    }
    let number: u32 = lower.strip_prefix('f')?.parse().ok()?;
    (1..=24).contains(&number).then(|| 0x6F + number)
}

impl Serialize for KeyCombo {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.describe())
    }
}

/// The defaults (spec 8.7).
pub const DEFAULTS: [(HotkeyAction, KeyCombo); 3] = [
    (HotkeyAction::StartOrFinish, KeyCombo::ctrl_shift_f(7)),
    (HotkeyAction::PauseOrResume, KeyCombo::ctrl_shift_f(8)),
    (HotkeyAction::CaptureNow, KeyCombo::ctrl_shift_f(9)),
];

/// The three hotkeys the person has chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySet {
    pub start_or_finish: KeyCombo,
    pub pause_or_resume: KeyCombo,
    pub capture_now: KeyCombo,
}

impl Default for HotkeySet {
    fn default() -> HotkeySet {
        HotkeySet { start_or_finish: DEFAULTS[0].1, pause_or_resume: DEFAULTS[1].1, capture_now: DEFAULTS[2].1 }
    }
}

impl HotkeySet {
    pub fn get(&self, action: HotkeyAction) -> KeyCombo {
        match action {
            HotkeyAction::StartOrFinish => self.start_or_finish,
            HotkeyAction::PauseOrResume => self.pause_or_resume,
            HotkeyAction::CaptureNow => self.capture_now,
        }
    }

    fn put(&mut self, action: HotkeyAction, combo: KeyCombo) {
        match action {
            HotkeyAction::StartOrFinish => self.start_or_finish = combo,
            HotkeyAction::PauseOrResume => self.pause_or_resume = combo,
            HotkeyAction::CaptureNow => self.capture_now = combo,
        }
    }

    /// The three, each with what it does.
    pub fn entries(&self) -> [(HotkeyAction, KeyCombo); 3] {
        HotkeyAction::ALL.map(|action| (action, self.get(action)))
    }

    /// Give `action` the key written in `text`. If the text cannot be read, the key is not allowed, or another hotkey already has it,
    /// the answer says why in words and nothing changes.
    pub fn try_set(&mut self, action: HotkeyAction, text: &str) -> Result<KeyCombo, String> {
        let combo = KeyCombo::parse(text)?;
        combo.check()?;
        for other in HotkeyAction::ALL {
            if other != action && self.get(other) == combo {
                return Err(format!(
                    "{} is already the hotkey to {}. Each hotkey needs its own keys.",
                    combo.describe(),
                    other.words()
                ));
            }
        }
        self.put(action, combo);
        Ok(combo)
    }
}

/// What Express's hotkeys are, in a sentence, for the capture bar and the Settings screen. `working` is the ones that are set.
pub fn sentence(working: &[(HotkeyAction, KeyCombo)]) -> String {
    if working.is_empty() {
        return String::new();
    }
    let each: Vec<String> =
        working.iter().map(|(action, key)| format!("{} {}", key.describe(), action.words())).collect();
    format!("Hotkeys: {}.", each.join(", "))
}

/// A settings file is read key by key: one that is missing, unreadable, not allowed or the same as another becomes the usual key for
/// that job, and the others are kept. A hand-edited file never stops the program from starting, or loses the other settings.
impl<'de> Deserialize<'de> for HotkeySet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<HotkeySet, D::Error> {
        #[derive(Deserialize, Default)]
        #[serde(rename_all = "camelCase", default)]
        struct Written {
            start_or_finish: Option<serde_json::Value>,
            pause_or_resume: Option<serde_json::Value>,
            capture_now: Option<serde_json::Value>,
        }
        let written = Written::deserialize(deserializer)?;
        let mut chosen = HotkeySet::default();
        for (action, value) in [
            (HotkeyAction::StartOrFinish, written.start_or_finish),
            (HotkeyAction::PauseOrResume, written.pause_or_resume),
            (HotkeyAction::CaptureNow, written.capture_now),
        ] {
            let combo = value.as_ref().and_then(|v| v.as_str()).and_then(|text| KeyCombo::parse(text).ok());
            if let Some(combo) = combo.filter(|c| c.check().is_ok()) {
                chosen.put(action, combo);
            }
        }
        let [a, b, c] = chosen.entries().map(|(_, key)| key);
        let all_different = a != b && a != c && b != c;
        Ok(if all_different { chosen } else { HotkeySet::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(text: &str) -> KeyCombo {
        KeyCombo::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    #[test]
    fn the_defaults_are_the_specs_and_never_f12() {
        let words: Vec<String> = DEFAULTS.iter().map(|(_, k)| k.describe()).collect();
        assert_eq!(words, ["Ctrl+Shift+F7", "Ctrl+Shift+F8", "Ctrl+Shift+F9"]);
        assert!(DEFAULTS.iter().all(|(_, k)| k.vk != VK_F12 && k.check().is_ok()));
        assert_eq!(HotkeySet::default().entries(), DEFAULTS);
    }

    #[test]
    fn a_key_is_described_in_words() {
        assert_eq!(combo("ctrl+alt+shift+k").describe(), "Ctrl+Alt+Shift+K");
        assert_eq!(KeyCombo { ctrl: false, alt: false, shift: false, win: true, vk: 0x31 }.describe(), "Windows+1");
        assert_eq!(KeyCombo::ctrl_shift_f(12).describe(), "Ctrl+Shift+F12");
        assert_eq!(KeyCombo::ctrl_shift_f(24).describe(), "Ctrl+Shift+F24");
    }

    #[test]
    fn what_is_typed_is_read_however_it_is_spaced_or_capitalised() {
        let want = combo("Ctrl+Shift+F7");
        assert_eq!(want, KeyCombo::ctrl_shift_f(7));
        for text in ["ctrl+shift+f7", "  CTRL + SHIFT + F7  ", "Shift+Control+f7", "shift+ctrl+F7"] {
            assert_eq!(combo(text), want, "{text}");
        }
        assert_eq!(combo("Windows+Shift+5").describe(), "Shift+Windows+5");
        assert_eq!(combo("win+shift+q"), combo("Windows+Shift+Q"));
        assert_eq!(combo("alt+shift+z").vk, 0x5A);
        assert_eq!(combo("ctrl+f24").vk, 0x87);
    }

    #[test]
    fn what_cannot_be_read_says_what_is_wrong() {
        let say = |text: &str| KeyCombo::parse(text).unwrap_err();
        assert!(say("").contains("Nothing is written"));
        assert!(say("   ").contains("Nothing is written"));
        assert!(say("Ctrl+Shift").contains("no key"), "{}", say("Ctrl+Shift"));
        assert!(say("Ctrl++F7").contains("empty part"));
        assert!(say("Ctrl+Ctrl+F7").contains("Ctrl is written twice"));
        assert!(say("Ctrl+Q+W").contains("two keys, Q and W"), "{}", say("Ctrl+Q+W"));
        assert!(say("Ctrl+Enter").contains("\"Enter\" is not a key Express can use"));
        assert!(say("Ctrl+F25").contains("F25"));
        assert!(say("Ctrl+F0").contains("F0"));
        assert!(say("Ctrl+Shift+?").contains("not a key"));
        for text in ["", "Ctrl+Shift", "Ctrl+Enter"] {
            assert!(say(text).contains("for example Ctrl+Shift+F7") || say(text).contains("Nothing"), "{text}");
        }
    }

    #[test]
    fn keys_that_would_take_a_key_from_every_other_program_are_refused() {
        let refused = |text: &str| combo(text).check().unwrap_err();
        assert!(refused("Ctrl+Shift+F12").contains("F12"));
        assert!(refused("F7").contains("no Ctrl"));
        assert!(refused("Q").contains("no Ctrl"));
        assert!(refused("Ctrl+C").contains("too many programs"));
        assert!(refused("Shift+Q").contains("capital letter"));
        assert!(refused("Windows+L").contains("too many programs"));
        assert!(refused("Ctrl+Alt+K").contains("AltGr"));
        assert!(refused("Ctrl+Alt+Shift+K").contains("AltGr"));
        assert!(refused("Alt+F4").contains("close the window"));
        assert!(refused("Ctrl+F4").contains("close a tab"));
        assert!(refused("Shift+F10").contains("context menu"));
        // A function key with one modifier is fine, and so is a letter with two.
        for text in [
            "Ctrl+F7",
            "Alt+F2",
            "Shift+F7",
            "Ctrl+Shift+Q",
            "Alt+Shift+9",
            "Windows+Shift+K",
            "Ctrl+Alt+F7",
            "Ctrl+Shift+F24",
        ] {
            assert_eq!(combo(text).check(), Ok(()), "{text}");
        }
        // Alt+F4 with Shift added is a different key and is allowed.
        assert_eq!(combo("Alt+Shift+F4").check(), Ok(()));
        assert!(KeyCombo { ctrl: true, alt: false, shift: true, win: false, vk: 0x0D }.check().is_err());
    }

    #[test]
    fn setting_a_hotkey_checks_the_text_and_leaves_the_others_alone_when_it_is_refused() {
        let mut set = HotkeySet::default();
        assert!(set.try_set(HotkeyAction::CaptureNow, "ctrl+alt+shift+k").unwrap_err().contains("AltGr"));
        assert_eq!(set, HotkeySet::default(), "a refusal changes nothing");
        let key = set.try_set(HotkeyAction::CaptureNow, "Ctrl + Shift + S").unwrap();
        assert_eq!(key.describe(), "Ctrl+Shift+S");
        assert_eq!(set.capture_now, key);
        assert_eq!(set.start_or_finish, DEFAULTS[0].1);
        // Another job already has Ctrl+Shift+F7.
        let clash = set.try_set(HotkeyAction::PauseOrResume, "ctrl+shift+f7").unwrap_err();
        assert!(
            clash.contains("Ctrl+Shift+F7") && clash.contains("start or finish") && clash.contains("own keys"),
            "{clash}"
        );
        assert_eq!(set.pause_or_resume, DEFAULTS[1].1);
        // The job's own key is not a clash with itself.
        assert!(set.try_set(HotkeyAction::StartOrFinish, "Ctrl+Shift+F7").is_ok());
    }

    #[test]
    fn the_sentence_names_the_keys_that_are_set() {
        assert_eq!(
            sentence(&DEFAULTS),
            "Hotkeys: Ctrl+Shift+F7 start or finish, Ctrl+Shift+F8 pause or resume, Ctrl+Shift+F9 capture now."
        );
        assert_eq!(sentence(&DEFAULTS[..2]), "Hotkeys: Ctrl+Shift+F7 start or finish, Ctrl+Shift+F8 pause or resume.");
        assert_eq!(sentence(&[]), "");
    }

    #[test]
    fn the_set_is_saved_as_words_and_read_back() {
        let mut set = HotkeySet::default();
        set.try_set(HotkeyAction::CaptureNow, "Alt+Shift+K").unwrap();
        let text = serde_json::to_string(&set).unwrap();
        assert_eq!(
            text,
            r#"{"startOrFinish":"Ctrl+Shift+F7","pauseOrResume":"Ctrl+Shift+F8","captureNow":"Alt+Shift+K"}"#
        );
        let back: HotkeySet = serde_json::from_str(&text).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn a_file_with_a_bad_key_keeps_the_good_ones_or_falls_back_to_the_usual() {
        let read = |json: &str| serde_json::from_str::<HotkeySet>(json).unwrap();
        // Missing keys are the usual ones.
        assert_eq!(read("{}"), HotkeySet::default());
        let partly = read(r#"{"captureNow": "Alt+Shift+K"}"#);
        assert_eq!(partly.capture_now.describe(), "Alt+Shift+K");
        assert_eq!(partly.start_or_finish, DEFAULTS[0].1);
        // A key that cannot be read, or is not allowed, is the usual key for that job.
        let bad = read(r#"{"startOrFinish": "banana", "pauseOrResume": "Ctrl+C", "captureNow": "Alt+Shift+K"}"#);
        assert_eq!(bad.start_or_finish, DEFAULTS[0].1);
        assert_eq!(bad.pause_or_resume, DEFAULTS[1].1);
        assert_eq!(bad.capture_now.describe(), "Alt+Shift+K");
        // Two jobs with the same key cannot both have it: the usual keys come back.
        let twice = read(r#"{"startOrFinish": "Alt+Shift+K", "pauseOrResume": "alt+shift+k"}"#);
        assert_eq!(twice, HotkeySet::default());
        // Swapping two jobs' keys is fine.
        let swapped = read(r#"{"startOrFinish": "Ctrl+Shift+F8", "pauseOrResume": "Ctrl+Shift+F7"}"#);
        assert_eq!(swapped.start_or_finish.describe(), "Ctrl+Shift+F8");
        assert_eq!(swapped.pause_or_resume.describe(), "Ctrl+Shift+F7");
        // Something that is not even text for a key is not a reason to lose the whole settings file.
        assert_eq!(read(r#"{"captureNow": 5, "pauseOrResume": null}"#), HotkeySet::default());
    }
}
