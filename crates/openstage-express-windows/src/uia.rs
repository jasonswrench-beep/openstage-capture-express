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

    /// The field that has the keyboard now, and what it holds.
    pub fn focused_field(&self) -> FocusedField {
        let unknown = FocusedField { name: String::new(), role: String::new(), text: FieldText::Unreadable };
        unsafe {
            let Ok(element) = self.automation.GetFocusedElement() else { return unknown };
            let name = element.CurrentName().map(|n| n.to_string()).unwrap_or_default();
            let role = element.CurrentControlType().map(|t| role_name(t.0).to_string()).unwrap_or_default();
            // A password field is never read, not even to check how long it is.
            if element.CurrentIsPassword().map(|p| p.as_bool()).unwrap_or(false) {
                return FocusedField { name, role, text: FieldText::Password };
            }
            let text = match read_text(&element) {
                Some(text) => FieldText::Text(text),
                None => FieldText::Unreadable,
            };
            FocusedField { name, role, text }
        }
    }
}

struct ValueRead {
    text: String,
    read_only: bool,
}

unsafe fn value_of(element: &IUIAutomationElement) -> Option<ValueRead> {
    let pattern: IUIAutomationValuePattern = element.GetCurrentPattern(UIA_ValuePatternId).ok()?.cast().ok()?;
    let text = pattern.CurrentValue().ok()?.to_string();
    let read_only = pattern.CurrentIsReadOnly().map(|r| r.as_bool()).unwrap_or(true);
    Some(ValueRead { text, read_only })
}

/// The text of a field: its value if it has one, else the text of a document. `None` if it gives none, or too much.
unsafe fn read_text(element: &IUIAutomationElement) -> Option<String> {
    let text = match value_of(element) {
        Some(value) => value.text,
        None => {
            let pattern: IUIAutomationTextPattern = element.GetCurrentPattern(UIA_TextPatternId).ok()?.cast().ok()?;
            let range = pattern.DocumentRange().ok()?;
            range.GetText(MAX_FIELD_CHARS as i32 + 1).ok()?.to_string()
        }
    };
    fit(text)
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
    fn a_field_value_that_is_a_whole_document_is_not_taken_as_what_was_typed() {
        assert_eq!(fit("ann@example.org".to_string()).as_deref(), Some("ann@example.org"));
        assert_eq!(fit(String::new()).as_deref(), Some(""), "an empty field is read as empty (no Type step is made)");
        assert!(fit("x".repeat(MAX_FIELD_CHARS)).is_some());
        assert!(fit("x".repeat(MAX_FIELD_CHARS + 1)).is_none());
        assert!(fit("é".repeat(MAX_FIELD_CHARS + 1)).is_none(), "letters are counted, not bytes");
    }
}
