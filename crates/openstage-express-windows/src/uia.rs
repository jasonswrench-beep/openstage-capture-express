// SPDX-License-Identifier: Apache-2.0
//! Windows' accessibility information (UI Automation), used for two questions (spec sections 8.3 and 8.4):
//!
//! * What is under this click? Its name and kind give the step its title (`Click "Save"`).
//! * What does the field that has the keyboard hold? The text of a `Type` step is read from it when typing finishes, so
//!   corrections, pasting and input-method text are right with nothing reconstructed from keys. A password field is never read.
//!
//! Asking can take a long time when a program is busy, so this is only ever used from the noting thread, never from a hook.

use openstage_capture_windows::role_name;
use openstage_express_core::builder::{FieldText, FocusedField};
use windows::core::Interface;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Ole::SafeArrayDestroy;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationValuePattern,
    UIA_TextPatternId, UIA_ValuePatternId,
};

/// A field with more text than this is not read: it is a whole document, not what was typed, and a wrong `Type` step is worse
/// than `Type in "Name"`.
pub const MAX_FIELD_CHARS: usize = 500;

/// What is at a point of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementInfo {
    pub name: String,
    /// Capture's English role name (`button`, `edit`, ...).
    pub role: String,
    /// A field that takes typing: an edit field, a document, or an editable combo box.
    pub editable: bool,
    /// The program the element belongs to.
    pub process_id: u32,
}

/// UI Automation, ready for questions on the thread that made it.
pub struct Uia {
    automation: IUIAutomation,
}

impl Uia {
    /// Start UI Automation on this thread. `None` if Windows will not.
    pub fn new() -> Option<Uia> {
        unsafe {
            // Harmless if this thread already started COM.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            Some(Uia { automation })
        }
    }

    /// The control at this point of the desktop (physical pixels).
    pub fn element_at(&self, x: i32, y: i32) -> Option<ElementInfo> {
        unsafe {
            let element = self.automation.ElementFromPoint(POINT { x, y }).ok()?;
            let name = element.CurrentName().map(|n| n.to_string()).unwrap_or_default();
            let kind = element.CurrentControlType().ok()?.0;
            let role = role_name(kind).to_string();
            let process_id = element.CurrentProcessId().unwrap_or(0).max(0) as u32;
            let editable = match role.as_str() {
                "edit" | "document" => true,
                "combo box" => value_of(&element).is_some_and(|v| !v.read_only),
                _ => false,
            };
            Some(ElementInfo { name, role, editable, process_id })
        }
    }

    /// The field that has the keyboard now, and what it holds when `read_text` is true (see `Host::focused_field`).
    pub fn focused_field(&self, read_text: bool) -> FocusedField {
        let unknown = FocusedField { name: String::new(), role: String::new(), text: FieldText::Unreadable, id: None };
        unsafe {
            let Ok(element) = self.automation.GetFocusedElement() else { return unknown };
            let name = element.CurrentName().map(|n| n.to_string()).unwrap_or_default();
            let role = element.CurrentControlType().map(|t| role_name(t.0).to_string()).unwrap_or_default();
            let id = runtime_id(&element);
            // A password field is never read, not even to check how long it is. If Windows cannot say whether it is one, it is
            // taken to be one (the security review of 2026-11-13: the check used to fail open).
            if counts_as_password(element.CurrentIsPassword()) {
                return FocusedField { name, role, text: FieldText::Password, id };
            }
            if !read_text {
                return FocusedField { name, role, text: FieldText::Unreadable, id };
            }
            let text = match read_text_of(&element) {
                Some(text) => FieldText::Text(text),
                None => FieldText::Unreadable,
            };
            FocusedField { name, role, text, id }
        }
    }
}

/// The element's runtime id, which names it for as long as it lives: how the field at the end of typing is told from the one
/// typing began in. None if Windows gives none.
unsafe fn runtime_id(element: &IUIAutomationElement) -> Option<Vec<i32>> {
    let array = element.GetRuntimeId().ok()?;
    if array.is_null() {
        return None;
    }
    let a = &*array;
    // A runtime id is a short list of 32-bit numbers, in one dimension.
    let id = (a.cDims == 1 && a.cbElements == 4 && !a.pvData.is_null()).then(|| {
        let count = (a.rgsabound[0].cElements as usize).min(64);
        std::slice::from_raw_parts(a.pvData as *const i32, count).to_vec()
    });
    let _ = SafeArrayDestroy(array);
    id
}

struct ValueRead {
    /// None when the value is far longer than any field's text could be kept (it is not copied at all).
    text: Option<String>,
    read_only: bool,
}

unsafe fn value_of(element: &IUIAutomationElement) -> Option<ValueRead> {
    let pattern: IUIAutomationValuePattern = element.GetCurrentPattern(UIA_ValuePatternId).ok()?.cast().ok()?;
    let value = pattern.CurrentValue().ok()?;
    // A value longer than a field's text could ever be kept is not turned into text at all (the review's hardening notes): only
    // its length is looked at.
    let text = (value.len() <= MAX_FIELD_CHARS * 4).then(|| value.to_string());
    let read_only = pattern.CurrentIsReadOnly().map(|r| r.as_bool()).unwrap_or(true);
    Some(ValueRead { text, read_only })
}

/// The text of a field: its value if it has one, else the text of a document. `None` if it gives none, or too much.
unsafe fn read_text_of(element: &IUIAutomationElement) -> Option<String> {
    let text = match value_of(element) {
        Some(value) => value.text?,
        None => {
            let pattern: IUIAutomationTextPattern = element.GetCurrentPattern(UIA_TextPatternId).ok()?.cast().ok()?;
            let range = pattern.DocumentRange().ok()?;
            range.GetText(MAX_FIELD_CHARS as i32 + 1).ok()?.to_string()
        }
    };
    fit(text)
}

/// Whether Windows' answer to "is this a password field?" means the field must be treated as one: yes, and also when Windows
/// could not answer (the check fails closed).
pub fn counts_as_password(answer: windows::core::Result<windows::core::BOOL>) -> bool {
    answer.map(|p| p.as_bool()).unwrap_or(true)
}

/// Keep the text if it is a plausible field value (not empty, not a whole document).
pub fn fit(text: String) -> Option<String> {
    if text.chars().count() > MAX_FIELD_CHARS {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_check_that_fails_counts_as_a_password_field() {
        assert!(counts_as_password(Ok(windows::core::BOOL(1))));
        assert!(!counts_as_password(Ok(windows::core::BOOL(0))));
        assert!(counts_as_password(Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))));
    }

    #[test]
    fn a_field_value_that_is_a_whole_document_is_not_taken_as_what_was_typed() {
        assert_eq!(fit("ann@example.org".to_string()).as_deref(), Some("ann@example.org"));
        assert_eq!(fit(String::new()).as_deref(), Some(""), "an empty field is read as empty (no Type step is made)");
        assert!(fit("x".repeat(MAX_FIELD_CHARS)).is_some());
        assert!(fit("x".repeat(MAX_FIELD_CHARS + 1)).is_none());
        assert!(fit("é".repeat(MAX_FIELD_CHARS + 1)).is_none(), "letters are counted, not bytes");
    }
}
