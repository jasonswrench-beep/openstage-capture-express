// SPDX-License-Identifier: Apache-2.0
//! Every command of the window in one list (the OpenStage suite's look): its words, its icon, its keys, a few words on what it is
//! for (which the command search also looks in), and the menu it is in. The menus, the command search and the Keyboard keys list
//! are all made from this list, so a command has the same name everywhere. Whether a command can be used now, and whether it is
//! ticked, is the window's to say (`App::command_state`).

use openstage_express_core::marks::Tool;

use super::icon::UiIcon;
use crate::settings::Appearance;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    // File
    StartCapturing,
    OpenCopy,
    ExportScreen,
    BackToGuides,
    Trash,
    // Edit
    Undo,
    Redo,
    ChooseAll,
    ChooseOne,
    DeletedSteps,
    // Step
    AddStep,
    DuplicateStep,
    DeleteStep,
    MoveUp,
    MoveDown,
    PreviousStep,
    NextStep,
    EditTitle,
    SkipInExport,
    FocusedView,
    // Marks
    AddMark(Tool),
    DeleteMark,
    // View
    FocusedAllOn,
    FocusedAllOff,
    Colours(Appearance),
    // Tools
    Settings,
    // Help
    SearchCommands,
    KeyboardKeys,
    About,
}

/// One of the window's menus, in the order of the menu bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPlan {
    pub title: &'static str,
    /// The commands in it; `None` is a line between groups.
    pub items: &'static [Option<Command>],
}

const MARKS: [Option<Command>; 11] = [
    Some(Command::AddMark(Tool::Rectangle)),
    Some(Command::AddMark(Tool::Oval)),
    Some(Command::AddMark(Tool::Line)),
    Some(Command::AddMark(Tool::Arrow)),
    Some(Command::AddMark(Tool::Text)),
    Some(Command::AddMark(Tool::Number)),
    Some(Command::AddMark(Tool::Highlight)),
    Some(Command::AddMark(Tool::Blur)),
    Some(Command::AddMark(Tool::Spotlight)),
    None,
    Some(Command::DeleteMark),
];

/// The menu bar: File, Edit, Step, Marks, View, Tools, Help.
pub const MENUS: [MenuPlan; 7] = [
    MenuPlan {
        title: "File",
        items: &[
            Some(Command::StartCapturing),
            Some(Command::OpenCopy),
            None,
            Some(Command::ExportScreen),
            None,
            Some(Command::BackToGuides),
            Some(Command::Trash),
        ],
    },
    MenuPlan {
        title: "Edit",
        items: &[
            Some(Command::Undo),
            Some(Command::Redo),
            None,
            Some(Command::ChooseAll),
            Some(Command::ChooseOne),
            None,
            Some(Command::DeletedSteps),
        ],
    },
    MenuPlan {
        title: "Step",
        items: &[
            Some(Command::AddStep),
            Some(Command::DuplicateStep),
            Some(Command::DeleteStep),
            None,
            Some(Command::MoveUp),
            Some(Command::MoveDown),
            Some(Command::PreviousStep),
            Some(Command::NextStep),
            None,
            Some(Command::EditTitle),
            Some(Command::SkipInExport),
            Some(Command::FocusedView),
        ],
    },
    MenuPlan { title: "Marks", items: &MARKS },
    MenuPlan {
        title: "View",
        items: &[
            Some(Command::FocusedAllOn),
            Some(Command::FocusedAllOff),
            None,
            Some(Command::Colours(Appearance::System)),
            Some(Command::Colours(Appearance::Light)),
            Some(Command::Colours(Appearance::Dark)),
        ],
    },
    MenuPlan { title: "Tools", items: &[Some(Command::Settings)] },
    MenuPlan {
        title: "Help",
        items: &[Some(Command::SearchCommands), Some(Command::KeyboardKeys), None, Some(Command::About)],
    },
];

/// The "..." menu of the steps list: what is done to the chosen steps.
pub const STEPS_MORE: [Option<Command>; 6] = [
    Some(Command::DuplicateStep),
    Some(Command::DeleteStep),
    None,
    Some(Command::ChooseAll),
    Some(Command::ChooseOne),
    Some(Command::DeletedSteps),
];

/// The "Add a mark" menu of the Marks section.
pub const ADD_MARK: [Option<Command>; 9] = [
    Some(Command::AddMark(Tool::Rectangle)),
    Some(Command::AddMark(Tool::Oval)),
    Some(Command::AddMark(Tool::Line)),
    Some(Command::AddMark(Tool::Arrow)),
    Some(Command::AddMark(Tool::Text)),
    Some(Command::AddMark(Tool::Number)),
    Some(Command::AddMark(Tool::Highlight)),
    Some(Command::AddMark(Tool::Blur)),
    Some(Command::AddMark(Tool::Spotlight)),
];

/// How a command shows in a menu: an ordinary item, one with a tick that turns on and off, or one of a few choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Plain,
    Tick,
    OneOf,
}

impl Command {
    /// Every command, in menu order, each once (for the command search and the tests).
    pub fn all() -> Vec<Command> {
        let mut out = Vec::new();
        for menu in MENUS {
            for command in menu.items.iter().flatten() {
                if !out.contains(command) {
                    out.push(*command);
                }
            }
        }
        out
    }

    /// Its words in a menu and in the command search. A command that opens a screen of choices ends in "...".
    pub fn words(self) -> String {
        match self {
            Command::StartCapturing => "Start capturing".into(),
            Command::OpenCopy => "Open a copy...".into(),
            Command::ExportScreen => "Export...".into(),
            Command::BackToGuides => "Back to guides".into(),
            Command::Trash => "Trash...".into(),
            Command::Undo => "Undo".into(),
            Command::Redo => "Redo".into(),
            Command::ChooseAll => "Choose all steps".into(),
            Command::ChooseOne => "Choose just one step".into(),
            Command::DeletedSteps => "Deleted steps...".into(),
            Command::AddStep => "Add a step".into(),
            Command::DuplicateStep => "Duplicate step".into(),
            Command::DeleteStep => "Delete step".into(),
            Command::MoveUp => "Move step up".into(),
            Command::MoveDown => "Move step down".into(),
            Command::PreviousStep => "Previous step".into(),
            Command::NextStep => "Next step".into(),
            Command::EditTitle => "Edit the step's title".into(),
            Command::SkipInExport => "Skip in export".into(),
            Command::FocusedView => "Focused view".into(),
            Command::AddMark(tool) => format!("Add {}", tool.name()),
            Command::DeleteMark => "Delete mark".into(),
            Command::FocusedAllOn => "Focused view on for every step".into(),
            Command::FocusedAllOff => "Focused view off for every step".into(),
            Command::Colours(Appearance::System) => "Colours: follow Windows".into(),
            Command::Colours(Appearance::Light) => "Colours: light".into(),
            Command::Colours(Appearance::Dark) => "Colours: dark".into(),
            Command::Settings => "Settings...".into(),
            Command::SearchCommands => "Search for a command".into(),
            Command::KeyboardKeys => "Keyboard keys".into(),
            Command::About => "About and licences".into(),
        }
    }

    pub fn icon(self) -> UiIcon {
        match self {
            Command::StartCapturing => UiIcon::Camera,
            Command::OpenCopy => UiIcon::PackageOpen,
            Command::ExportScreen => UiIcon::FileOutput,
            Command::BackToGuides => UiIcon::ArrowLeft,
            Command::Trash => UiIcon::Archive,
            Command::Undo => UiIcon::Undo2,
            Command::Redo => UiIcon::Redo2,
            Command::ChooseAll => UiIcon::SquareDashed,
            Command::ChooseOne => UiIcon::Check,
            Command::DeletedSteps => UiIcon::ArchiveRestore,
            Command::AddStep => UiIcon::Plus,
            Command::DuplicateStep => UiIcon::CopyPlus,
            Command::DeleteStep | Command::DeleteMark => UiIcon::Trash,
            Command::MoveUp => UiIcon::ArrowUp,
            Command::MoveDown => UiIcon::ArrowDown,
            Command::PreviousStep => UiIcon::ChevronUp,
            Command::NextStep => UiIcon::ChevronDown,
            Command::EditTitle => UiIcon::Pencil,
            Command::SkipInExport => UiIcon::EyeOff,
            Command::FocusedView => UiIcon::Focus,
            Command::AddMark(tool) => tool_icon(tool),
            Command::FocusedAllOn => UiIcon::ZoomIn,
            Command::FocusedAllOff => UiIcon::ZoomOut,
            Command::Colours(Appearance::System) => UiIcon::Monitor,
            Command::Colours(Appearance::Light) => UiIcon::Sun,
            Command::Colours(Appearance::Dark) => UiIcon::Moon,
            Command::Settings => UiIcon::Settings,
            Command::SearchCommands => UiIcon::Search,
            Command::KeyboardKeys => UiIcon::Keyboard,
            Command::About => UiIcon::Info,
        }
    }

    /// The keys that do it, as a person types them.
    pub fn keys(self) -> Option<&'static str> {
        Some(match self {
            Command::Undo => "Ctrl+Z",
            Command::Redo => "Ctrl+Y",
            Command::ChooseAll => "Ctrl+A",
            Command::DeleteStep => "Ctrl+Shift+Delete",
            Command::MoveUp => "Alt+Up",
            Command::MoveDown => "Alt+Down",
            Command::PreviousStep => "Ctrl+Shift+Up",
            Command::NextStep => "Ctrl+Shift+Down",
            Command::EditTitle => "F2",
            Command::DeleteMark => "Delete",
            Command::SearchCommands => "Ctrl+K",
            Command::KeyboardKeys => "Ctrl+/",
            _ => return None,
        })
    }

    /// A few words on what it does or where it is, which the command search also looks in.
    pub fn what_for(self) -> &'static str {
        match self {
            Command::StartCapturing => "record a new guide from what you do on screen, capture bar",
            Command::OpenCopy => "open a guide saved as one file, zip, import",
            Command::ExportScreen => "make a web page, Word, PowerPoint, PDF, pictures, GIF, Markdown, JSON, share",
            Command::BackToGuides => "home, the list of guides, close the guide",
            Command::Trash => "guides that were deleted, bring one back, restore",
            Command::Undo => "take back the last change",
            Command::Redo => "do again what was undone",
            Command::ChooseAll => "select every step",
            Command::ChooseOne => "select only the step with the keyboard",
            Command::DeletedSteps => "put a deleted step back, restore, empty the trash",
            Command::AddStep => "a new step after the chosen one, new",
            Command::DuplicateStep => "copy the step",
            Command::DeleteStep => "remove the chosen steps, they can be put back",
            Command::MoveUp => "earlier in the guide",
            Command::MoveDown => "later in the guide",
            Command::PreviousStep => "choose the step before",
            Command::NextStep => "choose the step after",
            Command::EditTitle => "rename the step, go to its title",
            Command::SkipInExport => "leave the step out of exports",
            Command::FocusedView => "close-up of the click, crop, zoom",
            Command::AddMark(_) => "draw on the picture, annotate, mark",
            Command::DeleteMark => "remove the chosen mark",
            Command::FocusedAllOn => "close-up for the whole guide, crop, zoom",
            Command::FocusedAllOff => "whole pictures for the whole guide",
            Command::Colours(_) => "appearance, theme, dark mode, light mode",
            Command::Settings => "hotkeys, folders, sounds, appearance, capturing, preferences, options",
            Command::SearchCommands => "find a command by its name",
            Command::KeyboardKeys => "shortcuts, every key",
            Command::About => "version, licence, notices, what it is built from",
        }
    }

    /// The menu it is in, for the command search ("Step menu").
    pub fn menu(self) -> &'static str {
        MENUS.iter().find(|m| m.items.contains(&Some(self))).map(|m| m.title).unwrap_or("")
    }

    pub fn kind(self) -> ItemKind {
        match self {
            Command::SkipInExport | Command::FocusedView => ItemKind::Tick,
            Command::Colours(_) => ItemKind::OneOf,
            _ => ItemKind::Plain,
        }
    }
}

/// The icon of each kind of mark.
pub fn tool_icon(tool: Tool) -> UiIcon {
    match tool {
        Tool::Rectangle => UiIcon::Square,
        Tool::Oval => UiIcon::Circle,
        Tool::Line => UiIcon::Minus,
        Tool::Arrow => UiIcon::MoveUpRight,
        Tool::Text => UiIcon::Type,
        Tool::Number => UiIcon::Hash,
        Tool::Highlight => UiIcon::Highlighter,
        Tool::Blur => UiIcon::Droplets,
        Tool::Spotlight => UiIcon::Flashlight,
    }
}

/// How well a command matches what was typed in the command search: 0 is no match. Every word typed must be found; a word at the
/// start of the command's name counts most, then anywhere in its name, then in its menu or what it is for.
pub fn score(command: Command, typed: &str) -> u32 {
    let words: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return 1;
    }
    let name = command.words().to_lowercase().replace("...", "");
    let extra = format!(
        "{} {} {}",
        command.menu().to_lowercase(),
        command.what_for().to_lowercase(),
        command.keys().unwrap_or("").to_lowercase()
    );
    let mut total = 0;
    for word in &words {
        let points = if name.starts_with(word.as_str()) {
            30
        } else if name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(word.as_str())) {
            20
        } else if name.contains(word.as_str()) {
            12
        } else if extra.split(|c: char| !c.is_alphanumeric() && c != '+').any(|w| w.starts_with(word.as_str())) {
            6
        } else if extra.contains(word.as_str()) {
            3
        } else {
            return 0;
        };
        total += points;
    }
    total
}

/// The commands that match what was typed, best first; commands that tie keep their menu order.
pub fn search(typed: &str) -> Vec<Command> {
    let mut found: Vec<(u32, usize, Command)> = Command::all()
        .into_iter()
        .enumerate()
        .map(|(i, c)| (score(c, typed), i, c))
        .filter(|(s, _, _)| *s > 0)
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_in_one_menu_has_words_and_an_icon_and_no_two_share_words() {
        let all = Command::all();
        assert!(all.len() > 30);
        let mut seen = std::collections::HashSet::new();
        for c in &all {
            assert!(!c.menu().is_empty(), "{c:?} is in no menu");
            assert!(seen.insert(c.words()), "two commands are called {:?}", c.words());
            assert!(!c.what_for().is_empty());
        }
        for tool in Tool::ALL {
            assert!(all.contains(&Command::AddMark(tool)), "{tool:?} can be added from the Marks menu");
        }
        assert_eq!(MENUS.map(|m| m.title), ["File", "Edit", "Step", "Marks", "View", "Tools", "Help"]);
    }

    #[test]
    fn the_search_finds_by_name_first_then_by_what_it_is_for_and_every_word_must_match() {
        assert_eq!(search("undo").first(), Some(&Command::Undo));
        assert_eq!(search("exp").first(), Some(&Command::ExportScreen));
        assert_eq!(search("dark").first(), Some(&Command::Colours(Appearance::Dark)));
        assert!(search("hotkeys").contains(&Command::Settings), "found by what it is for");
        assert!(search("pdf").contains(&Command::ExportScreen));
        assert_eq!(search("add arrow"), [Command::AddMark(Tool::Arrow)]);
        assert!(search("nothing like this").is_empty());
        assert_eq!(search("").len(), Command::all().len(), "nothing typed lists every command");
        assert_eq!(search("  ").first(), Command::all().first());
    }

    #[test]
    fn keys_are_written_the_way_a_person_types_them() {
        for c in Command::all() {
            if let Some(keys) = c.keys() {
                assert!(!keys.contains(' ') && !keys.ends_with('+'), "{keys:?}");
            }
        }
        assert_eq!(Command::SearchCommands.keys(), Some("Ctrl+K"));
        assert_eq!(Command::KeyboardKeys.keys(), Some("Ctrl+/"));
    }
}
