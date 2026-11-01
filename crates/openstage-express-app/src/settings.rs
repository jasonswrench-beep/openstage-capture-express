// SPDX-License-Identifier: Apache-2.0
//! The person's settings (spec sections 7.3 and 8.9): a small JSON file in `%APPDATA%\OpenStage Capture Express\`, written
//! atomically. A settings file that is missing, damaged or newer than this program is never a reason to fail: the program starts with
//! the defaults and says so. Fields this program does not know are kept and written back, so a slightly newer Express does not lose
//! its settings when an older one saves.

use std::path::{Path, PathBuf};

use openstage_express_core::builder::{CaptureMode, ModKey, Settings as BuilderSettings};
use openstage_express_core::capture::MarkOptions;
use openstage_express_core::fsutil::write_atomic;
use openstage_express_core::Language;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::target::{AreaChoice, TargetKind};

pub const SCHEMA_VERSION: &str = "0.1.0";

/// The colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    /// Follow Windows.
    #[default]
    System,
    Light,
    Dark,
}

/// When a click makes a step (spec C2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ModeChoice {
    /// Every click.
    #[default]
    OnClick,
    /// Only a click made while the capture key is held.
    WithKey,
    /// Every click except one made while the capture key is held.
    ExceptKey,
    /// Only the capture-now hotkey.
    HotkeyOnly,
}

/// The key held in the two key modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum KeyChoice {
    #[default]
    Ctrl,
    Shift,
    Alt,
}

impl KeyChoice {
    pub fn name(self) -> &'static str {
        match self {
            KeyChoice::Ctrl => "Ctrl",
            KeyChoice::Shift => "Shift",
            KeyChoice::Alt => "Alt",
        }
    }

    fn key(self) -> ModKey {
        match self {
            KeyChoice::Ctrl => ModKey::Ctrl,
            KeyChoice::Shift => ModKey::Shift,
            KeyChoice::Alt => ModKey::Alt,
        }
    }
}

/// The language of step titles and the guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TitleLanguage {
    #[default]
    En,
    De,
    Fr,
}

impl TitleLanguage {
    pub fn name(self) -> &'static str {
        match self {
            TitleLanguage::En => "English",
            TitleLanguage::De => "German",
            TitleLanguage::Fr => "French",
        }
    }

    /// The language code written in a guide.
    pub fn code(self) -> &'static str {
        match self {
            TitleLanguage::En => "en",
            TitleLanguage::De => "de",
            TitleLanguage::Fr => "fr",
        }
    }

    fn language(self) -> Language {
        match self {
            TitleLanguage::En => Language::English,
            TitleLanguage::De => Language::German,
            TitleLanguage::Fr => Language::French,
        }
    }
}

/// Everything the person can set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub schema_version: String,
    pub appearance: Appearance,
    pub capture_mode: ModeChoice,
    pub capture_key: KeyChoice,
    /// Draw a pointer at each click.
    pub show_cursor: bool,
    /// Draw a circle round each click.
    pub mark_clicked_area: bool,
    pub capture_typing: bool,
    /// What to capture: the whole screen, one window or an area (the window or area itself is chosen each time, but the last area
    /// is kept).
    pub capture_target: TargetKind,
    pub last_area: Option<AreaChoice>,
    pub title_language: TitleLanguage,
    /// How long to wait after a click before taking its picture (so an animation can finish).
    pub screenshot_delay_ms: u64,
    pub typing_delay_ms: u64,
    pub capture_outside_clicks: bool,
    pub hotkey_captures_cursor: bool,
    /// New steps start with the automatic close-up of the click.
    pub focused_view_for_new_steps: bool,
    pub open_folder_after_export: bool,
    /// Where guides are kept; `None` is `Documents\OpenStage Express\Guides`.
    pub guides_folder: Option<String>,
    /// Where exports go; `None` is `Documents\OpenStage Express\Exports`.
    pub exports_folder: Option<String>,
    /// The formats last chosen on the Export screen, by folder name (`html`, `pdf`, ...).
    pub export_formats: Vec<String>,
    /// Fields this program does not know, kept and written back.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for AppSettings {
    fn default() -> AppSettings {
        AppSettings {
            schema_version: SCHEMA_VERSION.to_string(),
            appearance: Appearance::System,
            capture_mode: ModeChoice::OnClick,
            capture_key: KeyChoice::Ctrl,
            show_cursor: true,
            mark_clicked_area: true,
            capture_typing: true,
            capture_target: TargetKind::Screen,
            last_area: None,
            title_language: TitleLanguage::En,
            screenshot_delay_ms: 0,
            typing_delay_ms: 1000,
            capture_outside_clicks: false,
            hotkey_captures_cursor: false,
            focused_view_for_new_steps: true,
            open_folder_after_export: true,
            guides_folder: None,
            exports_folder: None,
            export_formats: vec!["html".to_string(), "pdf".to_string()],
            extra: Map::new(),
        }
    }
}

/// The choices on offer for the two delays (a list to pick from is easier to use with a keyboard and a screen reader than a number
/// box, and these cover what is useful).
pub const SCREENSHOT_DELAYS_MS: [u64; 5] = [0, 250, 500, 1000, 2000];
pub const TYPING_DELAYS_MS: [u64; 5] = [500, 1000, 2000, 3000, 5000];

impl AppSettings {
    /// What the step builder follows.
    pub fn builder_settings(&self) -> BuilderSettings {
        let mode = match self.capture_mode {
            ModeChoice::OnClick => CaptureMode::OnClick,
            ModeChoice::WithKey => CaptureMode::OnClickWithKey(self.capture_key.key()),
            ModeChoice::ExceptKey => CaptureMode::OnClickExceptWithKey(self.capture_key.key()),
            ModeChoice::HotkeyOnly => CaptureMode::HotkeyOnly,
        };
        BuilderSettings {
            mode,
            capture_typing: self.capture_typing,
            typing_delay_ms: self.typing_delay_ms.clamp(500, 5000),
            screenshot_delay_ms: self.screenshot_delay_ms.min(5000),
            capture_outside_clicks: self.capture_outside_clicks,
            hotkey_captures_cursor: self.hotkey_captures_cursor,
            language: self.title_language.language(),
            ..BuilderSettings::default()
        }
    }

    /// How the marker, the cursor and the focused view are made, for a display with this scaling.
    pub fn mark_options(&self, dpi_percent: u32) -> MarkOptions {
        MarkOptions {
            click_mark: self.mark_clicked_area,
            cursor: self.show_cursor,
            dpi_percent,
            focused_view: self.focused_view_for_new_steps,
            ..MarkOptions::default()
        }
    }

    /// The guides folder this setting asks for, if any.
    pub fn guides_path(&self) -> Option<PathBuf> {
        self.guides_folder.as_ref().filter(|p| !p.trim().is_empty()).map(PathBuf::from)
    }

    pub fn exports_path(&self) -> Option<PathBuf> {
        self.exports_folder.as_ref().filter(|p| !p.trim().is_empty()).map(PathBuf::from)
    }
}

/// Where the settings file is: `%APPDATA%\OpenStage Capture Express\settings.json`.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .filter(|v| !v.is_empty())
        .map(|appdata| PathBuf::from(appdata).join("OpenStage Capture Express").join("settings.json"))
}

/// Read the settings. A missing file gives the defaults and no message. A file that cannot be read, is not JSON, or was written by a
/// newer program gives the defaults and a message to show; the file itself is not touched until the next save.
pub fn load(path: &Path) -> (AppSettings, Option<String>) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (AppSettings::default(), None),
        Err(e) => {
            return (
                AppSettings::default(),
                Some(format!("The settings could not be read ({e}), so the usual ones are used.")),
            )
        }
    };
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            return (
                AppSettings::default(),
                Some("The settings file is damaged, so the usual settings are used. It is replaced the next time a setting changes.".to_string()),
            )
        }
    };
    let newer = value.get("schemaVersion").and_then(Value::as_str).is_some_and(|v| version_is_newer(v, SCHEMA_VERSION));
    match serde_json::from_value::<AppSettings>(value) {
        Ok(settings) => {
            let note = newer.then(|| {
                "The settings were saved by a newer Express. Settings this one does not know are kept.".to_string()
            });
            (settings, note)
        }
        Err(_) => (
            AppSettings::default(),
            Some(
                "The settings file has something this program cannot read, so the usual settings are used.".to_string(),
            ),
        ),
    }
}

fn version_is_newer(found: &str, ours: &str) -> bool {
    let parse = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(found) > parse(ours)
}

/// Write the settings atomically.
pub fn save(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("The settings folder could not be made: {e}"))?;
    }
    let mut bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write_atomic(path, &bytes).map_err(|e| format!("The settings could not be saved: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_specs() {
        let s = AppSettings::default();
        assert_eq!(s.appearance, Appearance::System);
        assert_eq!(s.capture_mode, ModeChoice::OnClick);
        assert!(s.show_cursor && s.mark_clicked_area && s.capture_typing);
        assert_eq!((s.screenshot_delay_ms, s.typing_delay_ms), (0, 1000));
        assert!(!s.capture_outside_clicks && !s.hotkey_captures_cursor);
        assert!(s.focused_view_for_new_steps && s.open_folder_after_export);
        assert_eq!(s.builder_settings(), BuilderSettings::default());
    }

    #[test]
    fn settings_survive_a_save_and_a_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("settings.json");
        let mut s =
            AppSettings { appearance: Appearance::Dark, capture_mode: ModeChoice::WithKey, ..AppSettings::default() };
        s.capture_key = KeyChoice::Alt;
        s.guides_folder = Some("D:\\Guides".to_string());
        s.screenshot_delay_ms = 500;
        save(&path, &s).unwrap();
        let (back, note) = load(&path);
        assert_eq!(back, s);
        assert!(note.is_none());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"schemaVersion\": \"0.1.0\"") && text.contains("\"captureMode\": \"withKey\""),
            "{text}"
        );
    }

    #[test]
    fn a_missing_file_is_just_the_defaults_with_nothing_said() {
        let dir = tempfile::tempdir().unwrap();
        let (s, note) = load(&dir.path().join("nothing.json"));
        assert_eq!(s, AppSettings::default());
        assert!(note.is_none());
    }

    #[test]
    fn a_damaged_file_gives_the_defaults_and_a_message_and_is_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let (s, note) = load(&path);
        assert_eq!(s, AppSettings::default());
        assert!(note.unwrap().contains("damaged"));
        assert!(path.is_file(), "the file is left alone");
        std::fs::write(&path, br#"{"appearance": "neon"}"#).unwrap();
        let (s, note) = load(&path);
        assert_eq!(s, AppSettings::default());
        assert!(note.unwrap().contains("cannot read"));
    }

    #[test]
    fn fields_this_program_does_not_know_are_kept_and_a_newer_file_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, br#"{"schemaVersion": "0.2.0", "appearance": "light", "futureThing": {"a": 1}}"#)
            .unwrap();
        let (s, note) = load(&path);
        assert_eq!(s.appearance, Appearance::Light);
        assert!(note.unwrap().contains("newer Express"));
        save(&path, &s).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("futureThing"), "{text}");
        assert!(AppSettings::default().extra.is_empty());
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, br#"{"showCursor": false}"#).unwrap();
        let (s, _) = load(&path);
        assert!(!s.show_cursor);
        assert!(s.mark_clicked_area && s.capture_typing, "everything else is as usual");
    }

    #[test]
    fn the_capture_mode_and_key_become_the_builders_mode() {
        let mut s = AppSettings::default();
        assert_eq!(s.builder_settings().mode, CaptureMode::OnClick);
        s.capture_mode = ModeChoice::WithKey;
        s.capture_key = KeyChoice::Shift;
        assert_eq!(s.builder_settings().mode, CaptureMode::OnClickWithKey(ModKey::Shift));
        s.capture_mode = ModeChoice::ExceptKey;
        s.capture_key = KeyChoice::Alt;
        assert_eq!(s.builder_settings().mode, CaptureMode::OnClickExceptWithKey(ModKey::Alt));
        s.capture_mode = ModeChoice::HotkeyOnly;
        assert_eq!(s.builder_settings().mode, CaptureMode::HotkeyOnly);
    }

    #[test]
    fn delays_and_language_reach_the_builder_and_wild_values_are_kept_in_range() {
        let s = AppSettings {
            typing_delay_ms: 99_999,
            screenshot_delay_ms: 99_999,
            title_language: TitleLanguage::De,
            capture_typing: false,
            capture_outside_clicks: true,
            ..AppSettings::default()
        };
        let b = s.builder_settings();
        assert_eq!((b.typing_delay_ms, b.screenshot_delay_ms), (5000, 5000));
        assert_eq!(b.language, Language::German);
        assert!(!b.capture_typing && b.capture_outside_clicks);
        assert_eq!(
            AppSettings { typing_delay_ms: 1, ..AppSettings::default() }.builder_settings().typing_delay_ms,
            500
        );
    }

    #[test]
    fn marks_follow_the_settings_and_the_display_scaling() {
        let s = AppSettings {
            show_cursor: false,
            mark_clicked_area: true,
            focused_view_for_new_steps: false,
            ..AppSettings::default()
        };
        let m = s.mark_options(150);
        assert!(!m.cursor && m.click_mark && !m.focused_view);
        assert_eq!(m.dpi_percent, 150);
    }

    #[test]
    fn an_empty_folder_setting_means_the_usual_place() {
        let s = AppSettings {
            guides_folder: Some("  ".to_string()),
            exports_folder: Some("D:\\Out".to_string()),
            ..AppSettings::default()
        };
        assert_eq!(s.guides_path(), None);
        assert_eq!(s.exports_path(), Some(PathBuf::from("D:\\Out")));
    }

    #[test]
    fn version_comparison_is_by_number_not_by_text() {
        assert!(version_is_newer("0.10.0", "0.9.0"));
        assert!(version_is_newer("1.0.0", "0.1.0"));
        assert!(!version_is_newer("0.1.0", "0.1.0"));
        assert!(!version_is_newer("0.0.9", "0.1.0"));
    }
}
