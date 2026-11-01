// SPDX-License-Identifier: Apache-2.0
//! The global hotkeys (spec section 8.7), registered with `RegisterHotKey` so they work while another program has the focus.
//!
//! Never F12, which Windows keeps for debuggers. If another program already holds a hotkey, that one is left unassigned and the
//! reason is kept in words for the capture bar to show; Express never fails to start because of a hotkey.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT};

/// What a hotkey does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    StartOrFinish,
    PauseOrResume,
    CaptureNow,
}

impl Action {
    pub fn words(self) -> &'static str {
        match self {
            Action::StartOrFinish => "start or finish",
            Action::PauseOrResume => "pause or resume",
            Action::CaptureNow => "capture now",
        }
    }
}

/// A key with modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// The virtual-key code of the key itself.
    pub vk: u32,
}

impl Hotkey {
    /// Ctrl+Shift with a function key (`n` is 1 to 24).
    pub const fn ctrl_shift_f(n: u32) -> Hotkey {
        Hotkey { ctrl: true, alt: false, shift: true, win: false, vk: 0x6F + n }
    }

    /// For example `Ctrl+Shift+F7`.
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
            0x70..=0x87 => format!("F{}", self.vk - 0x6F),
            0x30..=0x39 | 0x41..=0x5A => ((self.vk as u8) as char).to_string(),
            other => format!("key {other:#x}"),
        });
        parts.join("+")
    }

    fn modifiers(&self) -> HOT_KEY_MODIFIERS {
        let mut m = MOD_NOREPEAT;
        if self.ctrl {
            m |= MOD_CONTROL;
        }
        if self.alt {
            m |= MOD_ALT;
        }
        if self.shift {
            m |= MOD_SHIFT;
        }
        if self.win {
            m |= MOD_WIN;
        }
        m
    }
}

/// The defaults (spec 8.7).
pub const DEFAULTS: [(Action, Hotkey); 3] = [
    (Action::StartOrFinish, Hotkey::ctrl_shift_f(7)),
    (Action::PauseOrResume, Hotkey::ctrl_shift_f(8)),
    (Action::CaptureNow, Hotkey::ctrl_shift_f(9)),
];

/// Windows' answer when another program already has the hotkey.
const ERROR_HOTKEY_ALREADY_REGISTERED: i32 = 1409;
const VK_F12: u32 = 0x7B;

/// Why a hotkey could not be set, in words for the person.
pub fn problem_words(action: Action, key: &Hotkey, windows_error: i32) -> String {
    if windows_error == ERROR_HOTKEY_ALREADY_REGISTERED {
        format!(
            "{} ({}) is already used by another program, so it is not set. You can still use the buttons on the capture bar.",
            key.describe(),
            action.words()
        )
    } else {
        format!(
            "Windows would not set {} ({}), error code {windows_error}. You can still use the buttons on the capture bar.",
            key.describe(),
            action.words()
        )
    }
}

/// The hotkeys, listening. Dropping this lets them go.
pub struct Hotkeys {
    events: Receiver<Action>,
    /// Hotkeys that could not be set, each in words.
    pub problems: Vec<String>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Hotkeys {
    /// Register `wanted`. This does not fail: a hotkey that cannot be set is listed in `problems`.
    pub fn start(wanted: &[(Action, Hotkey)]) -> Hotkeys {
        let (event_tx, events) = mpsc::channel::<Action>();
        let (ready_tx, ready_rx) = mpsc::channel::<(u32, Vec<String>)>();
        let wanted: Vec<(Action, Hotkey)> = wanted.to_vec();
        let thread = std::thread::spawn(move || {
            let mut problems = Vec::new();
            let mut registered = Vec::new();
            for (n, (action, key)) in wanted.iter().enumerate() {
                if key.vk == VK_F12 {
                    problems.push(format!(
                        "{} ({}) is not allowed: Windows keeps F12 for debuggers. Choose another key.",
                        key.describe(),
                        action.words()
                    ));
                    continue;
                }
                let id = n as i32 + 1;
                match unsafe { RegisterHotKey(None, id, key.modifiers(), key.vk) } {
                    Ok(()) => registered.push((id, *action)),
                    Err(e) => problems.push(problem_words(*action, key, e.code().0 & 0xFFFF)),
                }
            }
            let _ = ready_tx.send((unsafe { GetCurrentThreadId() }, problems));
            let mut message = MSG::default();
            while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
                if message.message == WM_HOTKEY {
                    let id = message.wParam.0 as i32;
                    if let Some((_, action)) = registered.iter().find(|(i, _)| *i == id) {
                        let _ = event_tx.send(*action);
                    }
                }
            }
            for (id, _) in &registered {
                let _ = unsafe { UnregisterHotKey(None, *id) };
            }
        });
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok((thread_id, problems)) => Hotkeys { events, problems, thread_id, thread: Some(thread) },
            Err(_) => Hotkeys {
                events,
                problems: vec![
                    "Windows did not set up the hotkeys in time. You can use the buttons on the capture bar.".into(),
                ],
                thread_id: 0,
                thread: None,
            },
        }
    }

    /// The next hotkey pressed, waiting up to `wait` for one.
    pub fn next(&self, wait: Duration) -> Result<Action, RecvTimeoutError> {
        self.events.recv_timeout(wait)
    }

    pub fn try_next(&self) -> Option<Action> {
        self.events.try_recv().ok()
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        if self.thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_specs_and_never_f12() {
        let words: Vec<String> = DEFAULTS.iter().map(|(_, k)| k.describe()).collect();
        assert_eq!(words, ["Ctrl+Shift+F7", "Ctrl+Shift+F8", "Ctrl+Shift+F9"]);
        assert!(DEFAULTS.iter().all(|(_, k)| k.vk != VK_F12));
    }

    #[test]
    fn a_hotkey_is_described_in_words() {
        let k = Hotkey { ctrl: true, alt: true, shift: false, win: false, vk: 0x51 };
        assert_eq!(k.describe(), "Ctrl+Alt+Q");
        assert_eq!(Hotkey { ctrl: false, alt: false, shift: false, win: true, vk: 0x31 }.describe(), "Windows+1");
        assert_eq!(Hotkey::ctrl_shift_f(12).describe(), "Ctrl+Shift+F12");
    }

    #[test]
    fn the_message_for_a_taken_hotkey_says_which_and_what_to_do() {
        let words = problem_words(Action::CaptureNow, &Hotkey::ctrl_shift_f(9), ERROR_HOTKEY_ALREADY_REGISTERED);
        assert!(words.contains("Ctrl+Shift+F9"), "{words}");
        assert!(words.contains("capture now"), "{words}");
        assert!(words.contains("another program"), "{words}");
        assert!(words.contains("buttons on the capture bar"), "{words}");
        let other = problem_words(Action::StartOrFinish, &Hotkey::ctrl_shift_f(7), 5);
        assert!(other.contains("error code 5"), "{other}");
    }

    #[test]
    fn f12_is_refused_in_words_and_does_not_stop_the_others() {
        let wanted = [
            (Action::StartOrFinish, Hotkey { ctrl: true, alt: false, shift: true, win: false, vk: VK_F12 }),
            (Action::CaptureNow, Hotkey { ctrl: true, alt: true, shift: true, win: true, vk: 0x7D }),
        ];
        let hotkeys = Hotkeys::start(&wanted);
        assert_eq!(hotkeys.problems.len(), 1, "{:?}", hotkeys.problems);
        assert!(hotkeys.problems[0].contains("F12"));
        assert!(hotkeys.try_next().is_none());
    }

    #[test]
    fn a_hotkey_another_program_holds_is_reported_not_fatal() {
        // The first set takes the hotkey; a second set asking for the same one is told it is taken.
        let key = Hotkey { ctrl: true, alt: true, shift: true, win: true, vk: 0x7E };
        let first = Hotkeys::start(&[(Action::PauseOrResume, key)]);
        assert!(first.problems.is_empty(), "{:?}", first.problems);
        let second = Hotkeys::start(&[(Action::PauseOrResume, key)]);
        assert_eq!(second.problems.len(), 1, "{:?}", second.problems);
        assert!(second.problems[0].contains("already used by another program"), "{}", second.problems[0]);
    }
}
