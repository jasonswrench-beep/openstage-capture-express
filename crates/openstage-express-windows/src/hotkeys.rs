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

// What a hotkey does, the keys that do it and the defaults are in the core (so that the Settings screen can check what a person
// types without Windows); this file registers them with Windows.
pub use openstage_express_core::hotkey::{HotkeyAction as Action, KeyCombo as Hotkey, DEFAULTS};

/// What Windows wants to be told about the modifier keys.
fn modifiers(key: &Hotkey) -> HOT_KEY_MODIFIERS {
    let mut m = MOD_NOREPEAT;
    if key.ctrl {
        m |= MOD_CONTROL;
    }
    if key.alt {
        m |= MOD_ALT;
    }
    if key.shift {
        m |= MOD_SHIFT;
    }
    if key.win {
        m |= MOD_WIN;
    }
    m
}

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
    /// The hotkeys that are set, which is `wanted` less the ones in `problems`.
    pub working: Vec<(Action, Hotkey)>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Hotkeys {
    /// Register `wanted`. This does not fail: a hotkey that cannot be set is listed in `problems`.
    pub fn start(wanted: &[(Action, Hotkey)]) -> Hotkeys {
        let (event_tx, events) = mpsc::channel::<Action>();
        let (ready_tx, ready_rx) = mpsc::channel::<(u32, Vec<String>, Vec<(Action, Hotkey)>)>();
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
                match unsafe { RegisterHotKey(None, id, modifiers(key), key.vk) } {
                    Ok(()) => registered.push((id, *action, *key)),
                    Err(e) => problems.push(problem_words(*action, key, e.code().0 & 0xFFFF)),
                }
            }
            let working = registered.iter().map(|(_, action, key)| (*action, *key)).collect();
            let _ = ready_tx.send((unsafe { GetCurrentThreadId() }, problems, working));
            let mut message = MSG::default();
            while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
                if message.message == WM_HOTKEY {
                    let id = message.wParam.0 as i32;
                    if let Some((_, action, _)) = registered.iter().find(|(i, _, _)| *i == id) {
                        let _ = event_tx.send(*action);
                    }
                }
            }
            for (id, _, _) in &registered {
                let _ = unsafe { UnregisterHotKey(None, *id) };
            }
        });
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok((thread_id, problems, working)) => {
                Hotkeys { events, problems, working, thread_id, thread: Some(thread) }
            }
            Err(_) => Hotkeys {
                events,
                problems: vec![
                    "Windows did not set up the hotkeys in time. You can use the buttons on the capture bar.".into(),
                ],
                working: Vec::new(),
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
    fn the_modifiers_become_windows_flags() {
        let m = modifiers(&Hotkey { ctrl: true, alt: false, shift: true, win: false, vk: 0x76 });
        assert_eq!(m, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT);
        let all = modifiers(&Hotkey { ctrl: true, alt: true, shift: true, win: true, vk: 0x76 });
        assert_eq!(all, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_WIN | MOD_NOREPEAT);
        assert_eq!(modifiers(&Hotkey { ctrl: false, alt: false, shift: false, win: false, vk: 0x76 }), MOD_NOREPEAT);
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
        assert_eq!(hotkeys.working, vec![wanted[1]], "only the one that was set is listed as working");
    }

    #[test]
    fn a_hotkey_another_program_holds_is_reported_not_fatal() {
        // The first set takes the hotkey; a second set asking for the same one is told it is taken.
        let key = Hotkey { ctrl: true, alt: true, shift: true, win: true, vk: 0x7E };
        let first = Hotkeys::start(&[(Action::PauseOrResume, key)]);
        assert!(first.problems.is_empty(), "{:?}", first.problems);
        assert_eq!(first.working, vec![(Action::PauseOrResume, key)]);
        let second = Hotkeys::start(&[(Action::PauseOrResume, key)]);
        assert_eq!(second.problems.len(), 1, "{:?}", second.problems);
        assert!(second.problems[0].contains("already used by another program"), "{}", second.problems[0]);
        assert!(second.working.is_empty());
    }

    #[test]
    fn a_key_can_be_let_go_and_set_again_with_other_keys() {
        // This is what changing a hotkey does: the old set is dropped, then a new one is made.
        let first_key = Hotkey { ctrl: true, alt: true, shift: true, win: true, vk: 0x7F };
        let second_key = Hotkey { ctrl: true, alt: true, shift: true, win: true, vk: 0x80 };
        let first = Hotkeys::start(&[(Action::CaptureNow, first_key)]);
        assert!(first.problems.is_empty(), "{:?}", first.problems);
        drop(first);
        let again = Hotkeys::start(&[(Action::CaptureNow, first_key), (Action::PauseOrResume, second_key)]);
        assert!(again.problems.is_empty(), "the first key was let go when the set was dropped: {:?}", again.problems);
        assert_eq!(again.working.len(), 2);
    }
}
