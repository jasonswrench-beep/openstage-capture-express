// SPDX-License-Identifier: Apache-2.0
//! The step builder (spec sections 8.1 and 8.2): the part that turns what a person does (clicks, typing, a hotkey) into
//! drafts of steps. It has no Windows code in it: the Windows side feeds it timestamped inputs and answers its two
//! questions through [`Host`] (a picture of the screen, and what is in the field that has the keyboard).
//!
//! Rules R1 to R12 of the spec are implemented here and each is covered by a scenario test below.
//!
//! The one thing the builder never sees is which keys were pressed: [`Input::Typing`] only says that a key went down,
//! and the text of a `Type` step is read from the field when typing finishes, so a password cannot leak through it.
//!
//! Choices this code makes where the spec is silent (all in the tests, none of them changes a spec rule):
//! - Any click ends a typing step in progress and settles a waiting click, whether or not that click is itself
//!   captured (outside the area, or not allowed by the capture mode). A click on one of Express's own windows, or on a
//!   mouse button other than left, right and middle, is ignored completely.
//! - A plain left click waits up to the double-click time before it becomes a draft, in case a second click makes it a
//!   double-click. Its picture is taken at once.
//! - Escape ends a typing step like Enter and Tab do, and the final text is read as usual.
//! - Typing that begins with no click waiting (the field was reached by keyboard) still makes a `Type` step, named
//!   from the focused field.
//! - The picture of a `Type` step is asked for at the time of the event that ended the typing. The host answers "the
//!   frame just before that time", which for a click is the frame the click's own picture would use.

use crate::model::{Button, Click, Control, Extra};
use crate::titles::{clean_name, click_title, screenshot_title, type_in_title, type_title, Language, Verb};

/// Milliseconds on any clock the host likes, as long as it never goes backwards.
pub type Ms = u64;

/// How long a click waits for typing before it becomes an ordinary click step, at the least (spec R4).
pub const PENDING_MIN_MS: Ms = 2000;

/// Stands for one picture of the screen the host holds. The builder never looks inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShotId(pub u64);

/// A modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKey {
    Ctrl,
    Alt,
    Shift,
    /// The Windows key (Cmd on a Mac).
    Meta,
}

/// Which modifier keys were held when something happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl Mods {
    pub fn none() -> Mods {
        Mods::default()
    }

    pub fn with(key: ModKey) -> Mods {
        let mut mods = Mods::default();
        match key {
            ModKey::Ctrl => mods.ctrl = true,
            ModKey::Alt => mods.alt = true,
            ModKey::Shift => mods.shift = true,
            ModKey::Meta => mods.meta = true,
        }
        mods
    }

    pub fn held(self, key: ModKey) -> bool {
        match key {
            ModKey::Ctrl => self.ctrl,
            ModKey::Alt => self.alt,
            ModKey::Shift => self.shift,
            ModKey::Meta => self.meta,
        }
    }
}

/// When a click makes a step (spec C2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Every click (the default).
    OnClick,
    /// Only a click made while this key is held.
    OnClickWithKey(ModKey),
    /// Every click except one made while this key is held.
    OnClickExceptWithKey(ModKey),
    /// No click makes a step; only the capture-now hotkey does.
    HotkeyOnly,
}

impl CaptureMode {
    fn allows(self, mods: Mods) -> bool {
        match self {
            CaptureMode::OnClick => true,
            CaptureMode::OnClickWithKey(key) => mods.held(key),
            CaptureMode::OnClickExceptWithKey(key) => !mods.held(key),
            CaptureMode::HotkeyOnly => false,
        }
    }
}

/// The mouse buttons the builder tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    /// Back, forward and any other button: ignored.
    Other,
}

/// What UI Automation says about the control under a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    /// Capture's English role name (`button`, `edit`, ...).
    pub role: String,
    /// True for an editable combo box or any field the host knows takes typing.
    pub editable: bool,
}

impl Target {
    pub fn new(name: &str, role: &str) -> Target {
        Target { name: name.to_string(), role: role.to_string(), editable: false }
    }

    pub fn editable(name: &str, role: &str) -> Target {
        Target { name: name.to_string(), role: role.to_string(), editable: true }
    }

    /// Does a click here start a possible typing step (spec R3)?
    fn takes_typing(&self) -> bool {
        self.editable || self.role == "edit" || self.role == "document"
    }
}

/// The special keys the builder cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Tab,
    Escape,
}

/// Everything the builder is told. Every input carries its time.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Click {
        at: Ms,
        button: MouseButton,
        x: i32,
        y: i32,
        target: Option<Target>,
        mods: Mods,
        /// Inside the capture area (always true when the whole screen is captured).
        inside_area: bool,
        /// One of Express's own windows (the capture bar and so on). Never captured.
        own_window: bool,
    },
    /// A key went down. Which key is never told.
    Typing {
        at: Ms,
    },
    Key {
        at: Ms,
        key: Key,
    },
    /// The capture-now hotkey, with the cursor position if the host has it.
    CaptureNow {
        at: Ms,
        cursor: Option<(i32, i32)>,
    },
    /// Time passing, so that waits can end.
    Tick {
        at: Ms,
    },
    /// Capturing was paused: settle everything waiting.
    Pause {
        at: Ms,
    },
    /// Capturing is finished: settle everything waiting.
    Finish {
        at: Ms,
    },
}

impl Input {
    pub fn at(&self) -> Ms {
        match self {
            Input::Click { at, .. }
            | Input::Typing { at }
            | Input::Key { at, .. }
            | Input::CaptureNow { at, .. }
            | Input::Tick { at }
            | Input::Pause { at }
            | Input::Finish { at } => *at,
        }
    }
}

/// What a field holds, as read when typing finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldText {
    Text(String),
    /// A password field: never read, never recorded.
    Password,
    /// The field gives no text to read.
    Unreadable,
}

/// The field that has the keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedField {
    pub name: String,
    pub role: String,
    pub text: FieldText,
}

/// What the builder needs from the outside world.
pub trait Host {
    /// A picture of the screen for the step made at this time: the frame just before it, or a fresh one if none is
    /// held. A time in the future means "wait until then" (the screenshot delay).
    fn screenshot(&mut self, at: Ms) -> ShotId;
    /// The field that has the keyboard now, and its text.
    fn focused_field(&mut self) -> FocusedField;
}

/// What a draft is, for the editor and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftKind {
    Click,
    RightClick,
    MiddleClick,
    DoubleClick,
    Select,
    /// Typed text that was read from the field.
    Type,
    /// Text was typed but could not be read back: `Type in "Email"`.
    TypeUnreadable,
    /// Made by the capture-now hotkey.
    Screenshot,
}

/// A step waiting to be given its picture and added to the guide.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub kind: DraftKind,
    pub title: String,
    pub shot: ShotId,
    pub click: Option<Click>,
    pub control: Option<Control>,
    /// The text of a `Type` step. Private (spec D8).
    pub typed: Option<String>,
    /// When the action that started this step happened.
    pub at: Ms,
}

/// The settings the builder follows (spec section 8.9). The defaults are the spec's.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub mode: CaptureMode,
    pub capture_typing: bool,
    pub typing_delay_ms: Ms,
    pub screenshot_delay_ms: Ms,
    pub capture_outside_clicks: bool,
    pub hotkey_captures_cursor: bool,
    pub language: Language,
    /// Two left clicks this close in time and place are one double-click.
    pub double_click_ms: Ms,
    pub double_click_px: i32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            mode: CaptureMode::OnClick,
            capture_typing: true,
            typing_delay_ms: 1000,
            screenshot_delay_ms: 0,
            capture_outside_clicks: false,
            hotkey_captures_cursor: false,
            language: Language::English,
            double_click_ms: 500,
            double_click_px: 4,
        }
    }
}

/// A left click that has a picture and is waiting to become a step.
#[derive(Debug, Clone)]
struct Candidate {
    at: Ms,
    shot: ShotId,
    x: i32,
    y: i32,
    target: Option<Target>,
}

#[derive(Debug)]
enum State {
    Idle,
    /// A plain left click, waiting out the double-click time.
    Held(Candidate),
    /// A click on a field, waiting for typing (spec R3, R4).
    Pending(Candidate),
    /// Typing is going on (spec R5).
    Typing {
        pending: Option<Candidate>,
        last: Ms,
    },
}

/// The step builder.
pub struct StepBuilder {
    settings: Settings,
    state: State,
    discarded: Vec<ShotId>,
}

impl StepBuilder {
    pub fn new(settings: Settings) -> StepBuilder {
        StepBuilder { settings, state: State::Idle, discarded: Vec::new() }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Change the settings (the next input uses them).
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Whether anything is waiting (a click, or typing). Finish and Pause settle it.
    pub fn is_waiting(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// The pictures the builder asked for and then did not use, so the host can let them go. Cleared by the call.
    pub fn take_discarded(&mut self) -> Vec<ShotId> {
        std::mem::take(&mut self.discarded)
    }

    /// Tell the builder something happened. Returns the drafts that are now settled, in the order the actions happened
    /// (spec R12).
    pub fn push(&mut self, host: &mut dyn Host, input: Input) -> Vec<Draft> {
        let mut out = Vec::new();
        self.expire(host, input.at(), &mut out);
        match input {
            Input::Tick { .. } => {}
            Input::Pause { at } | Input::Finish { at } => self.settle(host, at, &mut out),
            Input::Typing { at } => self.on_typing(at, &mut out),
            Input::Key { at, .. } => {
                // Enter, Tab and Escape all finish a typing step (spec R5). Nothing else waiting is affected.
                if matches!(self.state, State::Typing { .. }) {
                    self.end_typing(host, at, &mut out);
                }
            }
            Input::CaptureNow { at, cursor } => {
                self.settle(host, at, &mut out);
                let shot = host.screenshot(at);
                let click = cursor.filter(|_| self.settings.hotkey_captures_cursor).map(|(x, y)| Click {
                    x,
                    y,
                    button: Button::Left,
                    extra: Extra::new(),
                });
                out.push(Draft {
                    kind: DraftKind::Screenshot,
                    title: screenshot_title(self.settings.language).to_string(),
                    shot,
                    click,
                    control: None,
                    typed: None,
                    at,
                });
            }
            Input::Click { at, button, x, y, target, mods, inside_area, own_window } => {
                self.on_click(host, at, button, x, y, target, mods, inside_area, own_window, &mut out)
            }
        }
        out
    }

    /// Finish the capture and return everything still waiting.
    pub fn finish(&mut self, host: &mut dyn Host, at: Ms) -> Vec<Draft> {
        self.push(host, Input::Finish { at })
    }

    // ---- waits that run out

    fn expire(&mut self, host: &mut dyn Host, at: Ms, out: &mut Vec<Draft>) {
        let pending_timeout = PENDING_MIN_MS.max(self.settings.typing_delay_ms);
        let due = match &self.state {
            State::Held(c) => at.saturating_sub(c.at) > self.settings.double_click_ms,
            State::Pending(c) => at.saturating_sub(c.at) >= pending_timeout,
            State::Typing { last, .. } => at.saturating_sub(*last) >= self.settings.typing_delay_ms,
            State::Idle => false,
        };
        if due {
            self.settle(host, at, out);
        }
    }

    /// Settle whatever is waiting: a click becomes a plain click step, typing ends.
    fn settle(&mut self, host: &mut dyn Host, at: Ms, out: &mut Vec<Draft>) {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Idle => {}
            State::Held(c) | State::Pending(c) => out.push(self.plain_click(c)),
            State::Typing { pending, .. } => {
                self.state = State::Typing { pending, last: at };
                self.end_typing(host, at, out);
            }
        }
    }

    // ---- clicks

    #[allow(clippy::too_many_arguments)]
    fn on_click(
        &mut self,
        host: &mut dyn Host,
        at: Ms,
        button: MouseButton,
        x: i32,
        y: i32,
        target: Option<Target>,
        mods: Mods,
        inside_area: bool,
        own_window: bool,
        out: &mut Vec<Draft>,
    ) {
        // R9: Express's own windows are never captured. Other buttons (back, forward) are ignored (R7).
        if own_window || button == MouseButton::Other {
            return;
        }
        // R8: a second left click close to a waiting one makes it a double-click.
        if button == MouseButton::Left {
            let near = |c: &Candidate| {
                at.saturating_sub(c.at) <= self.settings.double_click_ms
                    && (x - c.x).abs() <= self.settings.double_click_px
                    && (y - c.y).abs() <= self.settings.double_click_px
            };
            let is_double = matches!(&self.state, State::Held(c) | State::Pending(c) if near(c));
            if is_double {
                if let State::Held(c) | State::Pending(c) = std::mem::replace(&mut self.state, State::Idle) {
                    out.push(self.click_draft(DraftKind::DoubleClick, Verb::DoubleClick, c));
                }
                return;
            }
        }
        // Any other click settles what was waiting first (R4, R5, R12).
        self.settle(host, at, out);
        // R1 and R2: the capture mode, then the capture area.
        let counts = self.settings.mode.allows(mods) && (inside_area || self.settings.capture_outside_clicks);
        if !counts {
            return;
        }
        // R11: the picture is the frame from the moment of the click, or one taken after the screenshot delay.
        let shot = host.screenshot(at + self.settings.screenshot_delay_ms);
        let candidate = Candidate { at, shot, x, y, target };
        match button {
            MouseButton::Left => {
                let typing =
                    self.settings.capture_typing && candidate.target.as_ref().is_some_and(Target::takes_typing);
                self.state = if typing { State::Pending(candidate) } else { State::Held(candidate) };
            }
            MouseButton::Right => out.push(self.click_draft(DraftKind::RightClick, Verb::RightClick, candidate)),
            MouseButton::Middle => out.push(self.click_draft(DraftKind::MiddleClick, Verb::MiddleClick, candidate)),
            MouseButton::Other => unreachable!("ignored above"),
        }
    }

    /// The draft for a click candidate with the given verb. The title is made from the control's name.
    fn click_draft(&self, kind: DraftKind, verb: Verb, c: Candidate) -> Draft {
        let name = c.target.as_ref().map(|t| clean_name(&t.name)).unwrap_or_default();
        let control =
            c.target.as_ref().map(|t| Control { name: clean_name(&t.name), role: t.role.clone(), extra: Extra::new() });
        let button = match kind {
            DraftKind::RightClick => Button::Right,
            DraftKind::MiddleClick => Button::Middle,
            _ => Button::Left,
        };
        Draft {
            kind,
            title: click_title(self.settings.language, verb, &name),
            shot: c.shot,
            click: Some(Click { x: c.x, y: c.y, button, extra: Extra::new() }),
            control,
            typed: None,
            at: c.at,
        }
    }

    /// An ordinary left click step: `Select` for dropdowns and radio buttons, `Click` otherwise.
    fn plain_click(&self, c: Candidate) -> Draft {
        let verb = c.target.as_ref().map(|t| Verb::for_role(&t.role)).unwrap_or(Verb::Click);
        let kind = if verb == Verb::Select { DraftKind::Select } else { DraftKind::Click };
        self.click_draft(kind, verb, c)
    }

    // ---- typing

    fn on_typing(&mut self, at: Ms, out: &mut Vec<Draft>) {
        if !self.settings.capture_typing {
            return;
        }
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Pending(c) => self.state = State::Typing { pending: Some(c), last: at },
            State::Typing { pending, .. } => self.state = State::Typing { pending, last: at },
            State::Held(c) => {
                // The click was not on a field Windows described as editable, but typing followed. The click stands
                // as its own step and a typing step begins.
                out.push(self.plain_click(c));
                self.state = State::Typing { pending: None, last: at };
            }
            State::Idle => self.state = State::Typing { pending: None, last: at },
        }
    }

    /// End a typing step: read the field and make one `Type` step, or the plain click it came from (R5, R6).
    fn end_typing(&mut self, host: &mut dyn Host, at: Ms, out: &mut Vec<Draft>) {
        let State::Typing { pending, .. } = std::mem::replace(&mut self.state, State::Idle) else {
            return;
        };
        let field = host.focused_field();
        let language = self.settings.language;
        match field.text {
            // R6: never a Type step, and never any text, for a password field.
            FieldText::Password => {
                if let Some(c) = pending {
                    out.push(self.plain_click(c));
                }
            }
            // R6: text was typed but cannot be read, so say where and not what.
            FieldText::Unreadable => {
                let name = pending
                    .as_ref()
                    .and_then(|c| c.target.as_ref())
                    .map(|t| clean_name(&t.name))
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| clean_name(&field.name));
                if name.is_empty() {
                    if let Some(c) = pending {
                        out.push(self.plain_click(c));
                    }
                } else {
                    let shot = host.screenshot(at);
                    self.discard_pending_shot(&pending);
                    out.push(Draft {
                        kind: DraftKind::TypeUnreadable,
                        title: type_in_title(language, &name),
                        shot,
                        click: pending.as_ref().map(|c| Click {
                            x: c.x,
                            y: c.y,
                            button: Button::Left,
                            extra: Extra::new(),
                        }),
                        control: Some(Control {
                            name,
                            role: self.field_role(&pending, &field.role),
                            extra: Extra::new(),
                        }),
                        typed: None,
                        at: pending.as_ref().map(|c| c.at).unwrap_or(at),
                    });
                }
            }
            FieldText::Text(text) => {
                let typed: String = text.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect();
                if clean_name(&typed).is_empty() {
                    // R5: nothing left in the field, so no Type step. The click it began with stays.
                    if let Some(c) = pending {
                        out.push(self.plain_click(c));
                    }
                } else {
                    let shot = host.screenshot(at);
                    self.discard_pending_shot(&pending);
                    let name = pending
                        .as_ref()
                        .and_then(|c| c.target.as_ref())
                        .map(|t| clean_name(&t.name))
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| clean_name(&field.name));
                    let role = self.field_role(&pending, &field.role);
                    out.push(Draft {
                        kind: DraftKind::Type,
                        title: type_title(language, &typed),
                        shot,
                        click: pending.as_ref().map(|c| Click {
                            x: c.x,
                            y: c.y,
                            button: Button::Left,
                            extra: Extra::new(),
                        }),
                        control: if name.is_empty() && role.is_empty() {
                            None
                        } else {
                            Some(Control { name, role, extra: Extra::new() })
                        },
                        typed: Some(typed),
                        at: pending.as_ref().map(|c| c.at).unwrap_or(at),
                    });
                }
            }
        }
    }

    fn field_role(&self, pending: &Option<Candidate>, from_field: &str) -> String {
        pending
            .as_ref()
            .and_then(|c| c.target.as_ref())
            .map(|t| t.role.clone())
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| from_field.to_string())
    }

    /// The click picture of a pending click is not used when a typing step replaces it.
    fn discard_pending_shot(&mut self, pending: &Option<Candidate>) {
        if let Some(c) = pending {
            self.discarded.push(c.shot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host with a scripted field and a log of every picture asked for.
    struct FakeHost {
        next: u64,
        asked: Vec<(ShotId, Ms)>,
        field: FocusedField,
    }

    impl FakeHost {
        fn new() -> FakeHost {
            FakeHost { next: 1, asked: Vec::new(), field: field("Email", "edit", FieldText::Text(String::new())) }
        }

        fn set_text(&mut self, text: &str) {
            self.field.text = FieldText::Text(text.to_string());
        }

        fn time_of(&self, shot: ShotId) -> Ms {
            self.asked.iter().find(|(id, _)| *id == shot).map(|(_, at)| *at).expect("a known picture")
        }
    }

    impl Host for FakeHost {
        fn screenshot(&mut self, at: Ms) -> ShotId {
            let id = ShotId(self.next);
            self.next += 1;
            self.asked.push((id, at));
            id
        }

        fn focused_field(&mut self) -> FocusedField {
            self.field.clone()
        }
    }

    fn field(name: &str, role: &str, text: FieldText) -> FocusedField {
        FocusedField { name: name.to_string(), role: role.to_string(), text }
    }

    fn click(at: Ms, x: i32, y: i32, name: &str, role: &str) -> Input {
        Input::Click {
            at,
            button: MouseButton::Left,
            x,
            y,
            target: Some(Target::new(name, role)),
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        }
    }

    fn field_click(at: Ms, x: i32, y: i32, name: &str) -> Input {
        Input::Click {
            at,
            button: MouseButton::Left,
            x,
            y,
            target: Some(Target::editable(name, "edit")),
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        }
    }

    /// Run a list of inputs and collect every draft in order.
    fn run(builder: &mut StepBuilder, host: &mut FakeHost, inputs: Vec<Input>) -> Vec<Draft> {
        inputs.into_iter().flat_map(|i| builder.push(host, i)).collect()
    }

    fn titles(drafts: &[Draft]) -> Vec<&str> {
        drafts.iter().map(|d| d.title.as_str()).collect()
    }

    fn builder() -> StepBuilder {
        StepBuilder::new(Settings::default())
    }

    #[test]
    fn a_plain_click_becomes_a_click_step_once_the_double_click_time_has_passed() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        assert!(b.push(&mut h, click(1000, 312, 188, "Save", "button")).is_empty(), "held for a possible double-click");
        assert!(b.is_waiting());
        assert!(b.push(&mut h, Input::Tick { at: 1500 }).is_empty(), "exactly the double-click time still waits");
        let drafts = b.push(&mut h, Input::Tick { at: 1501 });
        assert_eq!(drafts.len(), 1);
        let d = &drafts[0];
        assert_eq!((d.kind, d.title.as_str()), (DraftKind::Click, "Click \"Save\""));
        assert_eq!(d.click.as_ref().map(|c| (c.x, c.y, c.button)), Some((312, 188, Button::Left)));
        assert_eq!(d.control.as_ref().map(|c| (c.name.as_str(), c.role.as_str())), Some(("Save", "button")));
        assert_eq!(h.time_of(d.shot), 1000, "R11: the picture is from the moment of the click");
        assert!(!b.is_waiting());
        assert_eq!(d.typed, None);
    }

    #[test]
    fn finish_settles_a_waiting_click_at_once() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        b.push(&mut h, click(1000, 1, 1, "Save", "button"));
        let drafts = b.finish(&mut h, 1001);
        assert_eq!(titles(&drafts), ["Click \"Save\""]);
        assert!(b.finish(&mut h, 1002).is_empty(), "finishing twice makes nothing more");
    }

    #[test]
    fn dropdowns_and_radio_buttons_say_select_and_other_controls_say_click() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 10, 10, "Country", "combo box"),
                click(2000, 50, 50, "Yes", "radio button"),
                click(3000, 90, 90, "Remember me", "check box"),
                click(4000, 130, 130, "Terms", "link"),
                click(5000, 170, 170, "France", "list item"),
                click(6000, 210, 210, "Mode", "split button"),
                Input::Finish { at: 7000 },
            ],
        );
        assert_eq!(
            titles(&drafts),
            [
                "Select \"Country\"",
                "Select \"Yes\"",
                "Click \"Remember me\"",
                "Click \"Terms\"",
                "Click \"France\"",
                "Select \"Mode\""
            ]
        );
        assert_eq!(drafts[0].kind, DraftKind::Select);
        assert_eq!(drafts[2].kind, DraftKind::Click);
    }

    #[test]
    fn a_click_with_no_name_or_no_control_says_click_here() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let no_control = Input::Click {
            at: 3000,
            button: MouseButton::Left,
            x: 5,
            y: 5,
            target: None,
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        };
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 1, 1, "", "button"),
                click(2000, 90, 90, "  \t ", "link"),
                no_control,
                Input::Finish { at: 4000 },
            ],
        );
        assert_eq!(titles(&drafts), ["Click here", "Click here", "Click here"]);
        assert!(drafts[2].control.is_none(), "no control at all means no control field");
        assert_eq!(drafts[0].control.as_ref().map(|c| c.role.as_str()), Some("button"));
    }

    #[test]
    fn clicking_a_field_then_typing_makes_one_type_step_with_a_new_picture() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let mut drafts = run(&mut b, &mut h, vec![field_click(1000, 300, 200, "Email")]);
        assert!(drafts.is_empty(), "R3: a click on a field is pending, no step yet");
        for t in [1200, 1300, 1400] {
            drafts.extend(b.push(&mut h, Input::Typing { at: t }));
        }
        assert!(drafts.is_empty());
        h.set_text("jason@example.com");
        assert!(b.push(&mut h, Input::Tick { at: 2399 }).is_empty(), "not yet 1000 ms since the last key");
        drafts = b.push(&mut h, Input::Tick { at: 2400 });
        assert_eq!(drafts.len(), 1, "one merged step, not a click step and a type step");
        let d = &drafts[0];
        assert_eq!((d.kind, d.title.as_str()), (DraftKind::Type, "Type \"jason@example.com\""));
        assert_eq!(d.typed.as_deref(), Some("jason@example.com"));
        assert_eq!(d.click.as_ref().map(|c| (c.x, c.y)), Some((300, 200)), "the click's point is kept");
        assert_eq!(d.control.as_ref().map(|c| c.name.as_str()), Some("Email"));
        assert_eq!(h.time_of(d.shot), 2400, "a new picture, showing the filled field");
        assert_eq!(b.take_discarded().len(), 1, "the click's own picture is not used");
        assert!(b.take_discarded().is_empty());
    }

    #[test]
    fn backspacing_to_an_empty_field_makes_no_type_step_and_the_click_survives() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Search"),
                Input::Typing { at: 1100 },
                Input::Typing { at: 1200 },
                Input::Tick { at: 2200 },
            ],
        );
        assert_eq!(titles(&drafts), ["Click \"Search\""]);
        assert_eq!(drafts[0].kind, DraftKind::Click);
        assert_eq!(h.time_of(drafts[0].shot), 1000, "its own picture, from the click");
        assert!(b.take_discarded().is_empty());
    }

    #[test]
    fn a_field_with_only_spaces_counts_as_empty() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("   \t ");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Search"),
                Input::Typing { at: 1100 },
                Input::Key { at: 1200, key: Key::Enter },
            ],
        );
        assert_eq!(titles(&drafts), ["Click \"Search\""]);
    }

    #[test]
    fn enter_ends_typing_with_the_picture_from_that_moment() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("hello");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Search"),
                Input::Typing { at: 1100 },
                Input::Key { at: 1150, key: Key::Enter },
            ],
        );
        assert_eq!(titles(&drafts), ["Type \"hello\""]);
        assert_eq!(h.time_of(drafts[0].shot), 1150);
        assert!(!b.is_waiting());
    }

    #[test]
    fn tab_and_escape_also_end_typing() {
        for key in [Key::Tab, Key::Escape] {
            let (mut b, mut h) = (builder(), FakeHost::new());
            h.set_text("abc");
            let drafts = run(
                &mut b,
                &mut h,
                vec![field_click(1000, 5, 5, "Name"), Input::Typing { at: 1100 }, Input::Key { at: 1200, key }],
            );
            assert_eq!(titles(&drafts), ["Type \"abc\""], "{key:?}");
        }
    }

    #[test]
    fn enter_with_nothing_waiting_does_nothing() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        assert!(b.push(&mut h, Input::Key { at: 1000, key: Key::Enter }).is_empty());
        b.push(&mut h, field_click(1100, 5, 5, "Name"));
        assert!(b.push(&mut h, Input::Key { at: 1200, key: Key::Enter }).is_empty(), "a pending click is not typing");
        assert!(b.is_waiting());
    }

    #[test]
    fn the_typing_delay_is_a_setting() {
        let settings = Settings { typing_delay_ms: 2500, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        h.set_text("slow");
        b.push(&mut h, field_click(1000, 5, 5, "Name"));
        b.push(&mut h, Input::Typing { at: 1100 });
        assert!(b.push(&mut h, Input::Tick { at: 3599 }).is_empty());
        assert_eq!(titles(&b.push(&mut h, Input::Tick { at: 3600 })), ["Type \"slow\""]);
    }

    #[test]
    fn a_click_elsewhere_ends_typing_first_and_the_order_is_kept() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("name");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Name"),
                Input::Typing { at: 1100 },
                click(1300, 400, 400, "Next", "button"),
                Input::Finish { at: 1400 },
            ],
        );
        assert_eq!(titles(&drafts), ["Type \"name\"", "Click \"Next\""]);
        assert_eq!(h.time_of(drafts[0].shot), 1300, "the typing picture is the frame just before the click");
        assert_eq!(h.time_of(drafts[1].shot), 1300);
    }

    #[test]
    fn a_click_outside_the_area_makes_no_step_but_still_ends_typing() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("abc");
        b.push(&mut h, field_click(1000, 5, 5, "Name"));
        b.push(&mut h, Input::Typing { at: 1100 });
        let outside = Input::Click {
            at: 1200,
            button: MouseButton::Left,
            x: 9000,
            y: 9000,
            target: Some(Target::new("Elsewhere", "button")),
            mods: Mods::none(),
            inside_area: false,
            own_window: false,
        };
        let drafts = b.push(&mut h, outside);
        assert_eq!(titles(&drafts), ["Type \"abc\""], "R2: it still ends the typing step");
        assert!(b.push(&mut h, Input::Finish { at: 2000 }).is_empty(), "and makes none of its own");
    }

    #[test]
    fn outside_clicks_are_captured_when_the_setting_is_on() {
        let settings = Settings { capture_outside_clicks: true, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        let outside = Input::Click {
            at: 1000,
            button: MouseButton::Left,
            x: 9000,
            y: 9000,
            target: Some(Target::new("Elsewhere", "button")),
            mods: Mods::none(),
            inside_area: false,
            own_window: false,
        };
        b.push(&mut h, outside);
        assert_eq!(titles(&b.finish(&mut h, 1100)), ["Click \"Elsewhere\""]);
    }

    fn click_with(at: Ms, mods: Mods) -> Input {
        Input::Click {
            at,
            button: MouseButton::Left,
            x: 1,
            y: 1,
            target: Some(Target::new("Go", "button")),
            mods,
            inside_area: true,
            own_window: false,
        }
    }

    #[test]
    fn the_four_capture_modes_with_and_without_the_held_key() {
        let cases = [
            (CaptureMode::OnClick, Mods::none(), true),
            (CaptureMode::OnClick, Mods::with(ModKey::Ctrl), true),
            (CaptureMode::OnClickWithKey(ModKey::Ctrl), Mods::none(), false),
            (CaptureMode::OnClickWithKey(ModKey::Ctrl), Mods::with(ModKey::Ctrl), true),
            (CaptureMode::OnClickWithKey(ModKey::Ctrl), Mods::with(ModKey::Shift), false),
            (CaptureMode::OnClickExceptWithKey(ModKey::Shift), Mods::none(), true),
            (CaptureMode::OnClickExceptWithKey(ModKey::Shift), Mods::with(ModKey::Shift), false),
            (CaptureMode::OnClickExceptWithKey(ModKey::Alt), Mods::with(ModKey::Shift), true),
            (CaptureMode::HotkeyOnly, Mods::none(), false),
            (CaptureMode::HotkeyOnly, Mods::with(ModKey::Ctrl), false),
        ];
        for (mode, mods, expected) in cases {
            let settings = Settings { mode, ..Settings::default() };
            let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
            b.push(&mut h, click_with(1000, mods));
            let drafts = b.finish(&mut h, 1100);
            assert_eq!(drafts.len(), usize::from(expected), "{mode:?} with {mods:?}");
        }
    }

    #[test]
    fn a_click_the_mode_ignores_still_ends_typing() {
        let settings = Settings { mode: CaptureMode::OnClickWithKey(ModKey::Ctrl), ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        h.set_text("typed");
        b.push(
            &mut h,
            Input::Click {
                at: 1000,
                button: MouseButton::Left,
                x: 5,
                y: 5,
                target: Some(Target::editable("Name", "edit")),
                mods: Mods::with(ModKey::Ctrl),
                inside_area: true,
                own_window: false,
            },
        );
        b.push(&mut h, Input::Typing { at: 1100 });
        let drafts = b.push(&mut h, click_with(1200, Mods::none()));
        assert_eq!(titles(&drafts), ["Type \"typed\""]);
    }

    #[test]
    fn hotkey_only_mode_still_takes_the_capture_now_hotkey() {
        let settings = Settings { mode: CaptureMode::HotkeyOnly, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        assert!(b.push(&mut h, click_with(1000, Mods::none())).is_empty());
        let drafts = b.push(&mut h, Input::CaptureNow { at: 2000, cursor: Some((7, 8)) });
        assert_eq!(titles(&drafts), ["Screenshot"]);
        assert_eq!(drafts[0].kind, DraftKind::Screenshot);
    }

    #[test]
    fn two_quick_clicks_in_the_same_place_make_one_double_click_step() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let mut drafts = run(
            &mut b,
            &mut h,
            vec![click(1000, 100, 100, "Report.docx", "list item"), click(1300, 102, 99, "Report.docx", "list item")],
        );
        assert_eq!(titles(&drafts), ["Double-click \"Report.docx\""]);
        assert_eq!(drafts[0].kind, DraftKind::DoubleClick);
        assert_eq!(h.time_of(drafts[0].shot), 1000, "the picture from before the first click");
        assert_eq!(h.asked.len(), 1, "the second click of a double-click takes no picture");
        drafts = b.push(&mut h, Input::Finish { at: 5000 });
        assert!(drafts.is_empty(), "and nothing is left over");
    }

    #[test]
    fn a_third_click_after_a_double_click_starts_afresh() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 100, 100, "A", "button"),
                click(1200, 100, 100, "A", "button"),
                click(1400, 100, 100, "A", "button"),
                Input::Finish { at: 2000 },
            ],
        );
        assert_eq!(titles(&drafts), ["Double-click \"A\"", "Click \"A\""]);
    }

    #[test]
    fn clicks_in_different_places_or_too_far_apart_in_time_are_separate_steps() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 100, 100, "A", "button"),
                click(1200, 160, 100, "B", "button"),
                click(2000, 160, 100, "B", "button"),
                click(2600, 160, 100, "B", "button"),
                Input::Finish { at: 3000 },
            ],
        );
        assert_eq!(titles(&drafts), ["Click \"A\"", "Click \"B\"", "Click \"B\"", "Click \"B\""]);
    }

    #[test]
    fn a_double_click_on_a_field_is_a_double_click_and_leaves_nothing_pending() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let drafts = run(&mut b, &mut h, vec![field_click(1000, 50, 50, "Name"), field_click(1200, 50, 50, "Name")]);
        assert_eq!(titles(&drafts), ["Double-click \"Name\""]);
        assert!(!b.is_waiting());
    }

    #[test]
    fn right_and_middle_clicks_make_their_own_steps_at_once() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let mut right = click(1000, 1, 1, "Desktop", "pane");
        let mut middle = click(2000, 2, 2, "Tab", "tab item");
        let mut right_on_field = field_click(3000, 3, 3, "Name");
        for (input, button) in [
            (&mut right, MouseButton::Right),
            (&mut middle, MouseButton::Middle),
            (&mut right_on_field, MouseButton::Right),
        ] {
            if let Input::Click { button: b, .. } = input {
                *b = button;
            }
        }
        let drafts = run(&mut b, &mut h, vec![right, middle, right_on_field]);
        assert_eq!(titles(&drafts), ["Right-click \"Desktop\"", "Middle-click \"Tab\"", "Right-click \"Name\""]);
        assert_eq!(drafts[0].click.as_ref().map(|c| c.button), Some(Button::Right));
        assert_eq!(drafts[1].click.as_ref().map(|c| c.button), Some(Button::Middle));
        assert!(!b.is_waiting(), "a right click on a field is not a pending typing click");
    }

    #[test]
    fn other_mouse_buttons_are_ignored_completely() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("abc");
        b.push(&mut h, field_click(1000, 5, 5, "Name"));
        b.push(&mut h, Input::Typing { at: 1100 });
        let mut back = click(1200, 1, 1, "Page", "pane");
        if let Input::Click { button, .. } = &mut back {
            *button = MouseButton::Other;
        }
        assert!(b.push(&mut h, back).is_empty());
        assert!(b.is_waiting(), "typing carries on");
        assert_eq!(titles(&b.finish(&mut h, 1300)), ["Type \"abc\""]);
        assert_eq!(h.asked.len(), 2, "no picture was taken for the ignored click");
    }

    #[test]
    fn a_click_on_express_itself_is_never_captured_and_does_not_end_typing() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("abc");
        b.push(&mut h, field_click(1000, 5, 5, "Name"));
        b.push(&mut h, Input::Typing { at: 1100 });
        let mut own = click(1200, 1, 1, "Pause", "button");
        if let Input::Click { own_window, .. } = &mut own {
            *own_window = true;
        }
        assert!(b.push(&mut h, own).is_empty());
        assert!(b.is_waiting());
        assert_eq!(h.asked.len(), 1);
        assert_eq!(titles(&b.push(&mut h, Input::Pause { at: 1250 })), ["Type \"abc\""], "pausing settles it");
    }

    #[test]
    fn capture_now_makes_a_screenshot_step_with_or_without_the_cursor() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let drafts = b.push(&mut h, Input::CaptureNow { at: 1000, cursor: Some((10, 20)) });
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].title, "Screenshot");
        assert!(drafts[0].click.is_none(), "the cursor is only marked when the setting is on");
        assert!(drafts[0].control.is_none());

        let settings = Settings { hotkey_captures_cursor: true, ..Settings::default() };
        b.set_settings(settings);
        let with = b.push(&mut h, Input::CaptureNow { at: 2000, cursor: Some((10, 20)) });
        assert_eq!(with[0].click.as_ref().map(|c| (c.x, c.y)), Some((10, 20)));
        let without = b.push(&mut h, Input::CaptureNow { at: 3000, cursor: None });
        assert!(without[0].click.is_none(), "no position known, none marked");
    }

    #[test]
    fn capture_now_settles_what_is_waiting_before_it() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("abc");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Name"),
                Input::Typing { at: 1100 },
                Input::CaptureNow { at: 1200, cursor: None },
            ],
        );
        assert_eq!(titles(&drafts), ["Type \"abc\"", "Screenshot"]);
    }

    #[test]
    fn a_password_field_gives_only_a_click_and_never_any_text() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.field = field("Password", "edit", FieldText::Password);
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Password"),
                Input::Typing { at: 1100 },
                Input::Typing { at: 1200 },
                Input::Key { at: 1300, key: Key::Enter },
            ],
        );
        assert_eq!(titles(&drafts), ["Click \"Password\""]);
        assert_eq!(drafts[0].typed, None);
        let everything = format!("{drafts:?}");
        assert!(!everything.contains("hunter2"), "{everything}");
    }

    #[test]
    fn a_password_field_never_makes_a_step_even_when_typing_began_without_a_click() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.field = field("Password", "edit", FieldText::Password);
        let drafts = run(&mut b, &mut h, vec![Input::Typing { at: 1000 }, Input::Finish { at: 1100 }]);
        assert!(drafts.is_empty());
    }

    #[test]
    fn a_field_that_cannot_be_read_says_where_text_was_typed_and_not_what() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.field = field("Email", "edit", FieldText::Unreadable);
        let drafts = run(
            &mut b,
            &mut h,
            vec![field_click(1000, 5, 5, "Email"), Input::Typing { at: 1100 }, Input::Tick { at: 2100 }],
        );
        assert_eq!(titles(&drafts), ["Type in \"Email\""]);
        assert_eq!(drafts[0].kind, DraftKind::TypeUnreadable);
        assert_eq!(drafts[0].typed, None, "never guess text");
        assert_eq!(h.time_of(drafts[0].shot), 2100);
    }

    #[test]
    fn an_unreadable_field_with_no_name_at_all_leaves_the_click() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.field = field("", "", FieldText::Unreadable);
        let drafts = run(
            &mut b,
            &mut h,
            vec![field_click(1000, 5, 5, ""), Input::Typing { at: 1100 }, Input::Finish { at: 1200 }],
        );
        assert_eq!(titles(&drafts), ["Click here"]);
    }

    #[test]
    fn a_click_on_a_field_that_nobody_types_in_becomes_a_click_after_two_seconds() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        b.push(&mut h, field_click(1000, 5, 5, "Search"));
        assert!(b.push(&mut h, Input::Tick { at: 2999 }).is_empty());
        let drafts = b.push(&mut h, Input::Tick { at: 3000 });
        assert_eq!(titles(&drafts), ["Click \"Search\""]);
        assert_eq!(h.time_of(drafts[0].shot), 1000);
    }

    #[test]
    fn the_pending_wait_is_the_longer_of_two_seconds_and_the_typing_delay() {
        let settings = Settings { typing_delay_ms: 4000, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        b.push(&mut h, field_click(1000, 5, 5, "Search"));
        assert!(b.push(&mut h, Input::Tick { at: 4999 }).is_empty());
        assert_eq!(titles(&b.push(&mut h, Input::Tick { at: 5000 })), ["Click \"Search\""]);
    }

    #[test]
    fn typing_after_a_click_on_an_unremarkable_control_still_makes_a_type_step() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.field = field("Message", "pane", FieldText::Text("hello there".to_string()));
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 5, 5, "Chat", "pane"),
                Input::Typing { at: 1100 },
                Input::Key { at: 1200, key: Key::Enter },
            ],
        );
        assert_eq!(titles(&drafts), ["Click \"Chat\"", "Type \"hello there\""]);
        assert_eq!(
            drafts[1].control.as_ref().map(|c| c.name.as_str()),
            Some("Message"),
            "named from the focused field"
        );
        assert!(drafts[1].click.is_none());
    }

    #[test]
    fn typing_with_no_click_at_all_is_named_from_the_focused_field() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.field = field("Subject", "edit", FieldText::Text("Hello".to_string()));
        let drafts = run(&mut b, &mut h, vec![Input::Typing { at: 1000 }, Input::Key { at: 1100, key: Key::Tab }]);
        assert_eq!(titles(&drafts), ["Type \"Hello\""]);
        assert_eq!(drafts[0].control.as_ref().map(|c| (c.name.as_str(), c.role.as_str())), Some(("Subject", "edit")));
        assert_eq!(drafts[0].at, 1100);
    }

    #[test]
    fn typing_capture_off_means_plain_clicks_and_no_type_steps() {
        let settings = Settings { capture_typing: false, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        h.set_text("abc");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                field_click(1000, 5, 5, "Name"),
                Input::Typing { at: 1100 },
                Input::Key { at: 1200, key: Key::Enter },
                Input::Finish { at: 1300 },
            ],
        );
        assert_eq!(titles(&drafts), ["Click \"Name\""]);
    }

    #[test]
    fn titles_are_made_in_the_chosen_language() {
        let settings = Settings { language: Language::German, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        h.set_text("Hallo");
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 1, 1, "Speichern", "button"),
                field_click(3000, 5, 5, "Name"),
                Input::Typing { at: 3100 },
                Input::Key { at: 3200, key: Key::Enter },
                Input::CaptureNow { at: 4000, cursor: None },
            ],
        );
        assert_eq!(titles(&drafts), ["Klicken Sie auf \"Speichern\"", "Geben Sie \"Hallo\" ein", "Bildschirmfoto"]);
    }

    #[test]
    fn the_screenshot_delay_asks_for_a_picture_that_long_after_the_click() {
        let settings = Settings { screenshot_delay_ms: 500, ..Settings::default() };
        let (mut b, mut h) = (StepBuilder::new(settings), FakeHost::new());
        b.push(&mut h, click(1000, 1, 1, "Menu", "button"));
        let drafts = b.finish(&mut h, 1100);
        assert_eq!(h.time_of(drafts[0].shot), 1500);
        assert_eq!(drafts[0].at, 1000, "the step still dates from the click");
    }

    #[test]
    fn drafts_come_out_in_the_order_things_happened_with_everything_interleaved() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("typed");
        let mut right = click(5000, 9, 9, "Menu", "button");
        if let Input::Click { button, .. } = &mut right {
            *button = MouseButton::Right;
        }
        let drafts = run(
            &mut b,
            &mut h,
            vec![
                click(1000, 1, 1, "A", "button"),
                field_click(1200, 50, 50, "Box"),
                Input::Typing { at: 1400 },
                click(2500, 90, 90, "C", "button"),
                Input::CaptureNow { at: 2600, cursor: None },
                right,
                Input::Finish { at: 6000 },
            ],
        );
        assert_eq!(
            titles(&drafts),
            ["Click \"A\"", "Type \"typed\"", "Click \"C\"", "Screenshot", "Right-click \"Menu\""]
        );
        let times: Vec<Ms> = drafts.iter().map(|d| d.at).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]), "{times:?}");
    }

    #[test]
    fn finish_flushes_typing_and_a_held_click_together() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("abc");
        b.push(&mut h, field_click(1000, 5, 5, "Name"));
        b.push(&mut h, Input::Typing { at: 1100 });
        assert_eq!(titles(&b.finish(&mut h, 1200)), ["Type \"abc\""]);
        b.push(&mut h, click(2000, 5, 5, "Save", "button"));
        assert_eq!(titles(&b.finish(&mut h, 2050)), ["Click \"Save\""]);
        assert!(!b.is_waiting());
    }

    #[test]
    fn typed_text_is_kept_whole_and_the_title_is_tidied_and_cut() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        let long = format!("{}\nsecond line", "word ".repeat(30));
        h.set_text(&long);
        let drafts = run(
            &mut b,
            &mut h,
            vec![field_click(1000, 5, 5, "Notes"), Input::Typing { at: 1100 }, Input::Finish { at: 1200 }],
        );
        let d = &drafts[0];
        assert_eq!(d.typed.as_deref(), Some(long.as_str()), "the private text is whole, with its line break");
        assert!(d.title.starts_with("Type \"word word") && d.title.ends_with("\u{2026}\""), "{}", d.title);
        assert!(!d.title.contains('\n'));
        assert!(d.title.chars().count() <= 80 + "Type \"\"".len());
    }

    #[test]
    fn control_characters_in_typed_text_and_names_never_reach_a_title() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("ab\u{7}c\u{1b}[31md");
        let drafts = run(
            &mut b,
            &mut h,
            vec![field_click(1000, 5, 5, "Na\u{0}me\u{202E}"), Input::Typing { at: 1100 }, Input::Finish { at: 1200 }],
        );
        assert_eq!(drafts[0].title, "Type \"abc[31md\"");
        assert_eq!(drafts[0].typed.as_deref(), Some("abc[31md"));
        assert_eq!(drafts[0].control.as_ref().map(|c| c.name.as_str()), Some("Name"));
    }

    #[test]
    fn a_tick_with_nothing_waiting_does_nothing() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        assert!(b.push(&mut h, Input::Tick { at: 99_999 }).is_empty());
        assert!(h.asked.is_empty());
    }

    #[test]
    fn a_late_tick_settles_everything_that_was_due_in_order() {
        let (mut b, mut h) = (builder(), FakeHost::new());
        h.set_text("abc");
        b.push(&mut h, field_click(1000, 5, 5, "Name"));
        b.push(&mut h, Input::Typing { at: 1100 });
        let drafts = b.push(&mut h, Input::Tick { at: 60_000 });
        assert_eq!(titles(&drafts), ["Type \"abc\""]);
        assert!(!b.is_waiting());
    }

    #[test]
    fn input_times_are_read_correctly() {
        assert_eq!(Input::Tick { at: 5 }.at(), 5);
        assert_eq!(Input::Typing { at: 6 }.at(), 6);
        assert_eq!(Input::Key { at: 7, key: Key::Tab }.at(), 7);
        assert_eq!(Input::CaptureNow { at: 8, cursor: None }.at(), 8);
        assert_eq!(Input::Pause { at: 9 }.at(), 9);
        assert_eq!(Input::Finish { at: 10 }.at(), 10);
        assert_eq!(click(11, 0, 0, "x", "button").at(), 11);
    }
}
