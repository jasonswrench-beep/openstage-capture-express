// SPDX-License-Identifier: Apache-2.0
//! The mouse and keyboard hooks (spec sections 6.4 and 8.4).
//!
//! Windows calls a hook function whenever a mouse button or a key goes down, and removes a hook that is slow to answer. So the
//! hook functions here do almost nothing: they work out what kind of thing happened, stamp it with the time and hand it to a
//! bounded queue without ever waiting. A second thread (the noting thread, in [`crate::capture`]) does everything slow. If it
//! falls behind, what the hooks hand over is thrown away and counted, and the capture bar says so.
//!
//! **No key is ever recorded.** The keyboard hook sees which key it was, and in the same function decides one of three things:
//! Enter, Tab or Escape (which end a typing step), "a key that types something", or "nothing that matters here". Only that
//! answer leaves the function. [`Raw`] has no variant that could carry a key, so a character cannot reach the queue, a file or
//! the screen by accident. The text of a `Type` step is read from the field when typing finishes (spec D8).

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use openstage_express_core::builder::{Key, Mods, MouseButton};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_QUIT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

/// How many happenings the hooks may have handed over and not yet been dealt with before more are thrown away.
const QUEUE: usize = 8192;

/// What a hook hands over. There is deliberately no variant that carries a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raw {
    /// A mouse button went down at this point of the desktop, with these modifier keys held.
    Click { x: i32, y: i32, button: MouseButton, mods: Mods },
    /// A key that types something went down.
    Typing,
    /// Enter, Tab or Escape went down.
    Special(Key),
}

/// A happening and the moment it was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamped {
    pub at: Instant,
    pub raw: Raw,
}

/// What a key that went down means to the capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyClass {
    /// Nothing the capture cares about: a modifier on its own, an arrow, a function key, a shortcut.
    Ignore,
    /// A key that puts something into a field (a letter, a digit, Backspace, a pasted text).
    Typing,
    /// Enter, Tab or Escape.
    Special(Key),
}

const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_DELETE: u32 = 0x2E;

/// Decide what a key means, from its virtual-key code and the modifiers held. The code is looked at here and nowhere else.
pub fn classify_key(vk: u32, mods: Mods) -> KeyClass {
    let command = mods.ctrl || mods.alt || mods.meta;
    // Enter, Tab and Escape end a typing step, unless they are part of a shortcut such as Alt+Tab or Ctrl+Tab.
    if !command {
        match vk {
            VK_RETURN => return KeyClass::Special(Key::Enter),
            VK_TAB => return KeyClass::Special(Key::Tab),
            VK_ESCAPE => return KeyClass::Special(Key::Escape),
            _ => {}
        }
    }
    let types_something = matches!(vk,
        VK_BACK | VK_SPACE | VK_DELETE
        | 0x30..=0x39            // digits
        | 0x41..=0x5A            // letters
        | 0x60..=0x6F            // number pad digits and operators
        | 0xBA..=0xC0            // ; = , - . / `
        | 0xDB..=0xDF            // [ \ ] ' and the extra key some layouts have
        | 0xE2                   // the extra key between left Shift and Z
        | 0xE5                   // a key handled by an input method (Japanese, Chinese, Korean)
        | 0xE7                   // text sent as a character rather than a key (dictation, some touch keyboards)
    );
    if !types_something {
        return KeyClass::Ignore;
    }
    // A key held with the Windows key, or with Alt alone, is a shortcut. Control plus Alt is how many keyboards type @ or the
    // euro sign (AltGr). Control alone is a shortcut, except for the four that change a field: paste, cut, undo and redo.
    match (mods.ctrl, mods.alt, mods.meta) {
        (_, _, true) => KeyClass::Ignore,
        (false, true, false) => KeyClass::Ignore,
        (true, true, false) => KeyClass::Typing,
        (true, false, false) => {
            if matches!(vk, 0x56 | 0x58 | 0x5A | 0x59) {
                KeyClass::Typing
            } else {
                KeyClass::Ignore
            }
        }
        (false, false, false) => KeyClass::Typing,
    }
}

/// What the hook functions need. They are bare functions that Windows calls, so this lives with the hook thread.
struct HookState {
    tx: SyncSender<Stamped>,
    dropped: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
}

thread_local! {
    static HOOK: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

/// Hand a happening on. Nothing while capturing is paused. Never waits.
fn hand_over(raw: Raw) {
    HOOK.with(|slot| {
        let Ok(slot) = slot.try_borrow() else { return };
        let Some(state) = slot.as_ref() else { return };
        if !state.active.load(Ordering::Relaxed) {
            return;
        }
        match state.tx.try_send(Stamped { at: Instant::now(), raw }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                state.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    });
}

fn held(vk: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(vk.0 as i32) } as u16) & 0x8000 != 0
}

/// The modifier keys held right now.
pub fn current_mods() -> Mods {
    Mods { ctrl: held(VK_CONTROL), alt: held(VK_MENU), shift: held(VK_SHIFT), meta: held(VK_LWIN) || held(VK_RWIN) }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let button = match wparam.0 as u32 {
            WM_LBUTTONDOWN => Some(MouseButton::Left),
            WM_RBUTTONDOWN => Some(MouseButton::Right),
            WM_MBUTTONDOWN => Some(MouseButton::Middle),
            WM_XBUTTONDOWN => Some(MouseButton::Other),
            _ => None,
        };
        if let Some(button) = button {
            hand_over(Raw::Click { x: info.pt.x, y: info.pt.y, button, mods: current_mods() });
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        // The key code is read into `class` and goes no further.
        let class = {
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            classify_key(info.vkCode, current_mods())
        };
        match class {
            KeyClass::Typing => hand_over(Raw::Typing),
            KeyClass::Special(key) => hand_over(Raw::Special(key)),
            KeyClass::Ignore => {}
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// The thread that owns the hooks. A hook works only while its thread handles messages.
fn run_hooks(state: HookState, ready: mpsc::Sender<Result<u32, String>>) {
    HOOK.with(|slot| *slot.borrow_mut() = Some(state));
    unsafe {
        let module = match GetModuleHandleW(None) {
            Ok(module) => module,
            Err(e) => {
                let _ = ready.send(Err(format!("Windows would not say which program this is: {e}")));
                return;
            }
        };
        let mouse = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), Some(module.into()), 0) {
            Ok(hook) => hook,
            Err(e) => {
                let _ = ready.send(Err(format!("Windows would not let the program watch the mouse buttons: {e}")));
                return;
            }
        };
        let keyboard = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), Some(module.into()), 0) {
            Ok(hook) => hook,
            Err(e) => {
                let _ = UnhookWindowsHookEx(mouse);
                let _ = ready.send(Err(format!("Windows would not let the program watch the keyboard: {e}")));
                return;
            }
        };
        let _ = ready.send(Ok(GetCurrentThreadId()));
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(mouse);
        let _ = UnhookWindowsHookEx(keyboard);
    }
    // Letting go of the sender tells the other thread that nothing more will come.
    HOOK.with(|slot| slot.borrow_mut().take());
}

/// The running hooks. Dropping this takes them out.
pub struct Hooks {
    inbox: Receiver<Stamped>,
    dropped: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Hooks {
    /// Put the hooks in. They start out active.
    pub fn start() -> Result<Hooks, String> {
        let (tx, inbox) = mpsc::sync_channel::<Stamped>(QUEUE);
        let dropped = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicBool::new(true));
        let state = HookState { tx, dropped: Arc::clone(&dropped), active: Arc::clone(&active) };
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread = std::thread::spawn(move || run_hooks(state, ready_tx));
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(thread_id)) => Ok(Hooks { inbox, dropped, active, thread_id, thread: Some(thread) }),
            Ok(Err(why)) => {
                let _ = thread.join();
                Err(why)
            }
            Err(_) => Err("Windows did not set up the watching of the mouse and keyboard in time.".to_string()),
        }
    }

    /// While not active the hooks hand nothing over (capturing is paused).
    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    /// The next happening, waiting up to `wait` for one.
    pub fn next(&self, wait: Duration) -> Result<Stamped, RecvTimeoutError> {
        self.inbox.recv_timeout(wait)
    }

    /// How many happenings were thrown away because the queue was full.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        // Taking the hooks out is quick: ask the hook thread to leave its message loop and wait for it.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Mods {
        Mods::none()
    }

    fn ctrl() -> Mods {
        Mods { ctrl: true, ..Mods::none() }
    }

    fn alt() -> Mods {
        Mods { alt: true, ..Mods::none() }
    }

    fn altgr() -> Mods {
        Mods { ctrl: true, alt: true, ..Mods::none() }
    }

    fn win() -> Mods {
        Mods { meta: true, ..Mods::none() }
    }

    fn shift() -> Mods {
        Mods { shift: true, ..Mods::none() }
    }

    #[test]
    fn enter_tab_and_escape_are_the_special_keys() {
        assert_eq!(classify_key(0x0D, none()), KeyClass::Special(Key::Enter));
        assert_eq!(classify_key(0x09, none()), KeyClass::Special(Key::Tab));
        assert_eq!(classify_key(0x1B, none()), KeyClass::Special(Key::Escape));
        assert_eq!(
            classify_key(0x09, shift()),
            KeyClass::Special(Key::Tab),
            "Shift+Tab goes back, and still leaves the field"
        );
    }

    #[test]
    fn those_keys_inside_a_shortcut_are_not_special() {
        assert_eq!(classify_key(0x09, alt()), KeyClass::Ignore, "Alt+Tab switches programs");
        assert_eq!(classify_key(0x09, ctrl()), KeyClass::Ignore, "Ctrl+Tab switches tabs");
        assert_eq!(classify_key(0x1B, ctrl()), KeyClass::Ignore);
        assert_eq!(classify_key(0x0D, win()), KeyClass::Ignore);
    }

    #[test]
    fn letters_digits_and_editing_keys_count_as_typing() {
        for vk in [0x41, 0x5A, 0x30, 0x39, 0x20, 0x08, 0x2E, 0x60, 0x6F, 0xBA, 0xC0, 0xDB, 0xDF, 0xE2, 0xE5, 0xE7] {
            assert_eq!(classify_key(vk, none()), KeyClass::Typing, "{vk:#x}");
            assert_eq!(classify_key(vk, shift()), KeyClass::Typing, "{vk:#x} with Shift");
        }
    }

    #[test]
    fn keys_that_type_nothing_are_ignored() {
        // Shift, Control, Alt on their own and the left and right forms, Caps Lock, the arrows, Home, End, Page Up and Down,
        // Insert, Print Screen, F1 to F24, the Windows keys, the menu key, volume and media keys, Num Lock, Scroll Lock.
        for vk in [
            0x10, 0x11, 0x12, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x14, 0x25, 0x26, 0x27, 0x28, 0x21, 0x22, 0x23, 0x24,
            0x2D, 0x2C, 0x70, 0x7B, 0x87, 0x5B, 0x5C, 0x5D, 0xAD, 0xAE, 0xAF, 0xB0, 0x90, 0x91,
        ] {
            assert_eq!(classify_key(vk, none()), KeyClass::Ignore, "{vk:#x}");
        }
    }

    #[test]
    fn shortcuts_are_not_typing_but_paste_cut_undo_and_redo_are() {
        assert_eq!(classify_key(0x53, ctrl()), KeyClass::Ignore, "Ctrl+S");
        assert_eq!(classify_key(0x43, ctrl()), KeyClass::Ignore, "Ctrl+C changes nothing");
        assert_eq!(classify_key(0x41, ctrl()), KeyClass::Ignore, "Ctrl+A");
        assert_eq!(classify_key(0x56, ctrl()), KeyClass::Typing, "Ctrl+V");
        assert_eq!(classify_key(0x58, ctrl()), KeyClass::Typing, "Ctrl+X");
        assert_eq!(classify_key(0x5A, ctrl()), KeyClass::Typing, "Ctrl+Z");
        assert_eq!(classify_key(0x59, ctrl()), KeyClass::Typing, "Ctrl+Y");
        assert_eq!(classify_key(0x46, alt()), KeyClass::Ignore, "Alt+F opens a menu");
        assert_eq!(classify_key(0x44, win()), KeyClass::Ignore, "Windows+D");
        assert_eq!(classify_key(0x77, ctrl()), KeyClass::Ignore, "a function key is never typing");
    }

    #[test]
    fn the_capture_hotkeys_are_not_typing() {
        // Ctrl+Shift+F7, F8 and F9 are the default hotkeys.
        let both = Mods { ctrl: true, shift: true, ..Mods::none() };
        for vk in [0x76, 0x77, 0x78] {
            assert_eq!(classify_key(vk, both), KeyClass::Ignore);
        }
    }

    #[test]
    fn control_with_alt_is_a_keyboard_typing_a_symbol() {
        assert_eq!(classify_key(0x51, altgr()), KeyClass::Typing, "AltGr+Q is @ on a German keyboard");
        assert_eq!(classify_key(0x45, altgr()), KeyClass::Typing, "AltGr+E is the euro sign");
    }

    /// The type that leaves the keyboard hook has no room for a key. This is a compile-time fact: if a variant that carries
    /// a code were ever added to `Raw`, this match would have to change, and the change would be seen in review.
    #[test]
    fn what_the_hook_hands_over_has_no_key_in_it() {
        fn what(raw: Raw) -> &'static str {
            match raw {
                Raw::Click { .. } => "click",
                Raw::Typing => "typing",
                Raw::Special(Key::Enter | Key::Tab | Key::Escape) => "special",
            }
        }
        assert_eq!(what(Raw::Typing), "typing");
        assert!(std::mem::size_of::<Raw>() <= 24, "a click and a few flags, nothing more");
    }
}
