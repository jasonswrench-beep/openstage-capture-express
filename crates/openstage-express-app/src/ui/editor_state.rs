// SPDX-License-Identifier: Apache-2.0
//! The editor's state, with no drawing in it: which step is selected, what has been typed but not yet kept, and every change a
//! person can make (each one undoable). The screens call these; the tests call them too.
//!
//! What is typed into the title, the description or the picture description is held in a buffer and kept as *one* change when
//! the person leaves the box, picks another step, or does anything else, so undo takes back a whole edit and never one letter.

use std::sync::Arc;

use eframe::egui;
use openstage_capture_render::BlockText;
use openstage_express_core::focus::{auto_focus, DEFAULT_ZOOM_PERCENT};
use openstage_express_core::fsutil::utc_now_iso;
use openstage_express_core::marks::{self, Handle, Tool};
use openstage_express_core::model::{Annotation, AnnotationKind};
use openstage_express_core::render::render_marked;
use openstage_express_core::{Document, GuideFolder, Position, Session, Step, StepDraft};
use openstage_express_export::demo::Demo;
use openstage_express_export::picture::{DiskPictures, PictureSource};

use super::rows::{rows, Row};

/// What has been typed for the selected step.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Buffer {
    /// The guide's own title, as typed.
    pub guide_title: String,
    /// The step these words are for.
    pub for_step: Option<String>,
    pub title: String,
    pub description: String,
    pub picture_description: String,
}

pub struct EditorState {
    session: Session,
    pictures: Arc<dyn PictureSource + Send + Sync>,
    /// The guide folder this guide lives in, if it is a real guide (the example guide has none). Changes are saved to it.
    folder: Option<GuideFolder>,
    /// What the document looked like when it was last saved, to know when to save again.
    saved_stamp: Option<Stamp>,
    selected: Option<String>,
    pub buffer: Buffer,
    texture: Option<(String, egui::TextureHandle)>,
    /// Set by F2: the screen moves the keyboard to the title box.
    pub focus_title_next: bool,
    /// Set when Up or Down (or a click) chose a step: the screen moves the keyboard to that row.
    pub focus_row_next: bool,
    /// The mark being worked on, by its id within the selected step.
    selected_mark: Option<String>,
    /// Set when Up, Down or a click chose a mark in the marks list: the screen moves the keyboard to that row.
    pub focus_mark_next: bool,
    /// The words being typed for the selected text mark, and the mark they are for.
    pub mark_words: String,
    mark_words_for: Option<String>,
    /// A mark being dragged with the mouse: shown moved or resized, and only kept (as one change) when the mouse is let go.
    drag: Option<Drag>,
}

/// What a drag on the picture is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Move,
    Resize(Handle),
}

#[derive(Debug, Clone)]
struct Drag {
    kind: DragKind,
    /// The mark as it was when the drag began.
    original: Annotation,
    /// The mark as it is now.
    current: Annotation,
}

/// The words in the status line after something happens (a screen reader says them).
pub type Said = Option<String>;

/// Enough of the document's state to tell that it has changed since it was saved: how many changes can be undone, whether any can
/// be redone, and when the guide was last touched.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    undo_depth: usize,
    can_redo: bool,
    updated_at: String,
}

impl EditorState {
    /// An editor over the example guide.
    pub fn new(demo: Demo) -> EditorState {
        let mut state = EditorState {
            session: Session::new(demo.document),
            pictures: Arc::new(demo.pictures),
            folder: None,
            saved_stamp: None,
            selected: None,
            buffer: Buffer::default(),
            texture: None,
            focus_title_next: false,
            focus_row_next: false,
            selected_mark: None,
            focus_mark_next: false,
            mark_words: String::new(),
            mark_words_for: None,
            drag: None,
        };
        let first = state.session.doc().guide.steps.first().map(|s| s.id.clone());
        if let Some(id) = first {
            state.select(&id);
        }
        state
    }

    pub fn doc(&self) -> &Document {
        self.session.doc()
    }

    /// An editor over a real guide in `folder`. Every change is saved to the folder (see [`EditorState::save_if_changed`]).
    pub fn open(folder: GuideFolder, doc: Document) -> EditorState {
        let pictures = DiskPictures { root: folder.path().to_path_buf() };
        let mut state = EditorState {
            session: Session::new(doc),
            pictures: Arc::new(pictures),
            folder: Some(folder),
            saved_stamp: None,
            selected: None,
            buffer: Buffer::default(),
            texture: None,
            focus_title_next: false,
            focus_row_next: false,
            selected_mark: None,
            focus_mark_next: false,
            mark_words: String::new(),
            mark_words_for: None,
            drag: None,
        };
        state.saved_stamp = Some(state.stamp());
        let first = state.session.doc().guide.steps.first().map(|s| s.id.clone());
        if let Some(id) = first {
            state.select(&id);
        }
        state
    }

    fn stamp(&self) -> Stamp {
        Stamp {
            undo_depth: self.session.undo_depth(),
            can_redo: self.session.can_redo(),
            updated_at: self.session.doc().guide.updated_at.clone(),
        }
    }

    /// The guide's pictures, to share with the thread that makes an export.
    pub fn pictures(&self) -> Arc<dyn PictureSource + Send + Sync> {
        Arc::clone(&self.pictures)
    }

    /// Whether this editor saves to a guide folder.
    pub fn is_saved_to_disk(&self) -> bool {
        self.folder.is_some()
    }

    /// Save the guide if it changed since it was last saved. Returns what to tell the person if the save failed (the editor
    /// carries on, and tries again at the next change).
    pub fn save_if_changed(&mut self) -> Option<String> {
        let folder = self.folder.as_ref()?;
        let now = self.stamp();
        if self.saved_stamp.as_ref() == Some(&now) {
            return None;
        }
        match folder.save(self.session.doc()) {
            Ok(_) => {
                self.saved_stamp = Some(now);
                None
            }
            Err(why) => Some(format!("The guide could not be saved: {why}")),
        }
    }

    pub fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    pub fn rows(&self) -> Vec<Row> {
        rows(&self.session.doc().guide)
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn selected_step(&self) -> Option<&Step> {
        self.session.doc().guide.step(self.selected.as_deref()?)
    }

    /// The row for the selected step.
    pub fn selected_row(&self) -> Option<Row> {
        let id = self.selected.as_deref()?;
        self.rows().into_iter().find(|r| r.id == id)
    }

    /// Select a step, keeping what was typed for the one before.
    pub fn select(&mut self, id: &str) {
        self.commit_pending();
        if self.session.doc().guide.step(id).is_some() {
            if self.selected.as_deref() != Some(id) {
                self.selected_mark = None;
                self.drag = None;
            }
            self.selected = Some(id.to_string());
        }
        self.reload_buffer();
    }

    fn reload_buffer(&mut self) {
        let guide_title = self.session.doc().guide.title.clone();
        self.buffer = match self.selected_step() {
            Some(step) => Buffer {
                guide_title: guide_title.clone(),
                for_step: Some(step.id.clone()),
                title: step.title.clone(),
                description: step.description.clone(),
                picture_description: step.image_description.clone(),
            },
            None => Buffer { guide_title, ..Buffer::default() },
        };
    }

    /// Keep what has been typed, as one undoable change for each box that differs from the guide.
    pub fn commit_pending(&mut self) {
        // The guide's own title. A title with nothing in it is not kept: the old one comes back into the box.
        let wanted = self.buffer.guide_title.trim().to_string();
        if wanted.is_empty() {
            self.buffer.guide_title = self.session.doc().guide.title.clone();
        } else if wanted != self.session.doc().guide.title {
            let now = utc_now_iso();
            let _ = self.session.apply(|d| {
                d.set_guide_title(&wanted, &now);
                Ok(())
            });
        }
        let Some(id) = self.buffer.for_step.clone() else { return };
        let Some(step) = self.session.doc().guide.step(&id) else { return };
        let (title, description, picture_description) = (
            step.title != self.buffer.title,
            step.description != self.buffer.description,
            step.image_description != self.buffer.picture_description,
        );
        let now = utc_now_iso();
        if title {
            let text = self.buffer.title.clone();
            let _ = self.session.apply(|d| d.set_title(&id, &text, &now));
        }
        if description {
            let text = self.buffer.description.clone();
            let _ = self.session.apply(|d| d.set_description(&id, &text, &now));
        }
        if picture_description {
            let text = self.buffer.picture_description.clone();
            let _ = self.session.apply(|d| d.set_image_description(&id, &text, &now));
        }
    }

    fn keep_selection_valid(&mut self) {
        self.drag = None;
        let still_there = self
            .selected_mark
            .as_deref()
            .is_some_and(|m| self.selected_step().is_some_and(|s| s.annotations.iter().any(|a| a.id == m)));
        if !still_there {
            self.selected_mark = None;
        }
        let valid = self.selected.as_deref().is_some_and(|id| self.session.doc().guide.step(id).is_some());
        if !valid {
            self.selected = self.session.doc().guide.steps.first().map(|s| s.id.clone());
        }
        self.reload_buffer();
    }

    pub fn undo(&mut self) -> Said {
        self.commit_pending();
        if self.session.undo() {
            self.keep_selection_valid();
            Some("Undone.".to_string())
        } else {
            Some("There is nothing to undo.".to_string())
        }
    }

    pub fn redo(&mut self) -> Said {
        self.commit_pending();
        if self.session.redo() {
            self.keep_selection_valid();
            Some("Redone.".to_string())
        } else {
            Some("There is nothing to redo.".to_string())
        }
    }

    /// The step before or after the selected one in the list (`delta` of -1 or 1).
    pub fn neighbour(&self, delta: i32) -> Option<String> {
        let all = self.rows();
        let at = all.iter().position(|r| Some(r.id.as_str()) == self.selected.as_deref())? as i32;
        let target = at + delta;
        (0..all.len() as i32).contains(&target).then(|| all[target as usize].id.clone())
    }

    fn describe_selected(&self) -> String {
        self.selected_row().map(|r| r.label()).unwrap_or_default()
    }

    /// Move the selected step up or down one place among its neighbours.
    pub fn move_selected(&mut self, delta: i32) -> Said {
        self.commit_pending();
        let id = self.selected.clone()?;
        let before = self.session.doc().guide.steps.iter().position(|s| s.id == id);
        let now = utc_now_iso();
        match self.session.apply(|d| d.move_step_by(&id, delta, &now)) {
            Ok(()) if before != self.session.doc().guide.steps.iter().position(|s| s.id == id) => {
                self.reload_buffer();
                let now_at = self.describe_selected();
                Some(format!("Moved {}. Now {now_at}.", if delta < 0 { "up" } else { "down" }))
            }
            _ => Some(format!("Cannot move {} any further.", if delta < 0 { "up" } else { "down" })),
        }
    }

    /// Delete the selected step (it goes to the trash and can be brought back with undo).
    pub fn delete_selected(&mut self) -> Said {
        self.commit_pending();
        let id = self.selected.clone()?;
        let title = self.describe_selected();
        let next = self.neighbour(1).or_else(|| self.neighbour(-1));
        let now = utc_now_iso();
        match self.session.apply(|d| d.delete_step(&id, &now)) {
            Ok(()) => {
                self.selected = next;
                self.keep_selection_valid();
                Some(format!("Deleted {title}. Undo brings it back."))
            }
            Err(_) => Some("That step could not be deleted.".to_string()),
        }
    }

    /// Add an empty step after the selected one (and its substeps), or at the end.
    pub fn add_step(&mut self) -> Said {
        self.commit_pending();
        let now = utc_now_iso();
        let after = self.selected.clone();
        let result = self.session.apply(|d| {
            let position = match after.as_deref() {
                Some(id) => Position::After(id),
                None => Position::End,
            };
            d.add_step(StepDraft::empty("New step"), position, &now)
        });
        match result {
            Ok(id) => {
                self.selected = Some(id);
                self.reload_buffer();
                self.focus_title_next = true;
                Some("Added an empty step. Type its title.".to_string())
            }
            Err(_) => Some("A step could not be added.".to_string()),
        }
    }

    /// Duplicate the selected step (a copy of it, with its picture and marks, goes straight after it).
    pub fn duplicate_selected(&mut self) -> Said {
        self.commit_pending();
        let id = self.selected.clone()?;
        let now = utc_now_iso();
        match self.session.apply(|d| d.duplicate_step(&id, &now)) {
            Ok(copy) => {
                self.selected = Some(copy);
                self.selected_mark = None;
                self.reload_buffer();
                let row = self.describe_selected();
                Some(format!("Duplicated. The copy is {row}."))
            }
            Err(_) => Some("That step could not be duplicated.".to_string()),
        }
    }

    pub fn set_skipped(&mut self, skipped: bool) -> Said {
        self.commit_pending();
        let id = self.selected.clone()?;
        let now = utc_now_iso();
        let _ = self.session.apply(|d| d.set_skipped(&id, skipped, &now));
        let row = self.describe_selected();
        Some(if skipped {
            format!("{row}. It will be left out of exports.")
        } else {
            format!("{row}. It will be in exports.")
        })
    }

    /// Whether the focused view can be turned on for the selected step: it needs a picture and a click to centre on.
    pub fn can_focus(&self) -> bool {
        self.selected_step().is_some_and(|s| s.image.is_some() && s.click.is_some())
    }

    pub fn is_focused(&self) -> bool {
        self.selected_step().is_some_and(|s| s.focus.is_some())
    }

    pub fn set_focused(&mut self, on: bool) -> Said {
        self.commit_pending();
        let id = self.selected.clone()?;
        let step = self.selected_step()?;
        let rect = if on {
            let (image, click) = (step.image.as_ref()?, step.click.as_ref()?);
            Some(auto_focus(image.width, image.height, Some((click.x, click.y)), DEFAULT_ZOOM_PERCENT)?)
        } else {
            None
        };
        let now = utc_now_iso();
        let _ = self.session.apply(|d| d.set_focus(&id, rect, &now));
        Some(if on {
            "Focused view is on for this step.".to_string()
        } else {
            "Focused view is off for this step.".to_string()
        })
    }

    /// The words that describe the selected step's picture: the author's own, or the step's title.
    pub fn picture_words(&self) -> String {
        match self.selected_step() {
            Some(s) if !s.image_description.trim().is_empty() => s.image_description.trim().to_string(),
            Some(s) if !s.title.trim().is_empty() => s.title.trim().to_string(),
            _ => "A picture of the screen".to_string(),
        }
    }

    // ---- marks

    /// The size of the selected step's picture in pixels.
    pub fn picture_size(&self) -> Option<(u32, u32)> {
        self.selected_step()?.image.as_ref().map(|i| (i.width, i.height))
    }

    /// The selected step's marks, as the editor shows them (a mark being dragged is shown where it is now).
    pub fn marks(&self) -> Vec<Annotation> {
        let Some(step) = self.selected_step() else { return Vec::new() };
        step.annotations
            .iter()
            .map(|m| match &self.drag {
                Some(d) if d.current.id == m.id => d.current.clone(),
                _ => m.clone(),
            })
            .collect()
    }

    /// One line for each mark of the selected step, with its id: what the marks list shows and says.
    pub fn mark_rows(&self) -> Vec<(String, String)> {
        let h = self.picture_size().map(|(_, h)| h).unwrap_or(1080);
        self.marks()
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.clone(), format!("Mark {}: {}", i + 1, marks::describe(m, h))))
            .collect()
    }

    pub fn selected_mark_id(&self) -> Option<&str> {
        self.selected_mark.as_deref()
    }

    /// The selected mark, as it is now (while dragging, where it is being dragged to).
    pub fn selected_mark(&self) -> Option<Annotation> {
        let id = self.selected_mark.as_deref()?;
        self.marks().into_iter().find(|m| m.id == id)
    }

    /// Make the box for typing a text mark's words show the selected mark's words, when the selected mark has changed.
    pub fn sync_mark_words(&mut self) {
        let now = self.selected_mark.clone();
        if self.mark_words_for != now {
            self.mark_words = self.selected_mark().and_then(|m| m.text).unwrap_or_default();
            self.mark_words_for = now;
        }
    }

    /// The mark above or below the selected one in the marks list (`delta` of -1 or 1).
    pub fn mark_neighbour(&self, delta: i32) -> Option<String> {
        let rows = self.mark_rows();
        let at = rows.iter().position(|(id, _)| Some(id.as_str()) == self.selected_mark.as_deref())? as i32;
        let target = at + delta;
        (0..rows.len() as i32).contains(&target).then(|| rows[target as usize].0.clone())
    }

    pub fn select_mark(&mut self, id: Option<&str>) {
        self.drag = None;
        self.selected_mark = id
            .filter(|id| self.selected_step().is_some_and(|s| s.annotations.iter().any(|a| a.id == *id)))
            .map(str::to_string);
    }

    /// The words for the selected mark, for the status line.
    fn mark_words(&self) -> String {
        let h = self.picture_size().map(|(_, h)| h).unwrap_or(1080);
        self.selected_mark().map(|m| marks::describe(&m, h)).unwrap_or_default()
    }

    /// Change the selected mark with change as one undoable step. Returns false if there is no mark or the change did nothing.
    fn change_mark(&mut self, change: impl FnOnce(&mut Annotation, u32, u32)) -> bool {
        self.commit_pending();
        let (Some(step_id), Some((w, h))) = (self.selected.clone(), self.picture_size()) else { return false };
        let Some(mut mark) = self.selected_mark_in_document() else { return false };
        let before = mark.clone();
        change(&mut mark, w, h);
        if mark == before {
            return false;
        }
        let now = utc_now_iso();
        self.session.apply(|d| d.replace_annotation(&step_id, mark, &now)).is_ok()
    }

    /// The selected mark as the guide has it (not as a drag is showing it).
    fn selected_mark_in_document(&self) -> Option<Annotation> {
        let id = self.selected_mark.as_deref()?;
        self.selected_step()?.annotations.iter().find(|a| a.id == id).cloned()
    }

    /// Add a mark to the selected step's picture, round the click if there is one, and select it.
    pub fn add_mark(&mut self, tool: Tool) -> Said {
        self.commit_pending();
        let step_id = self.selected.clone()?;
        let Some((w, h)) = self.picture_size() else { return Some("This step has no picture to mark.".to_string()) };
        let step = self.selected_step()?;
        let around = step.click.as_ref().map(|c| (c.x, c.y));
        let numbers = step.annotations.iter().filter(|a| a.kind == marks::Tool::Number.kind()).count() as u32;
        let mark = marks::default_mark(tool, w, h, around, numbers);
        let now = utc_now_iso();
        match self.session.apply(|d| d.add_annotation(&step_id, mark, &now)) {
            Ok(id) => {
                self.selected_mark = Some(id);
                Some(format!("Added a {}: {}. Move it with the buttons or drag it.", tool.name(), self.mark_words()))
            }
            Err(_) => Some("The mark could not be added.".to_string()),
        }
    }

    /// Move the selected mark by dx, dy picture pixels.
    pub fn nudge_mark(&mut self, dx: i32, dy: i32) -> Said {
        self.selected_mark.as_ref()?;
        let moved = self.change_mark(|m, w, h| marks::nudge(m, dx, dy, w, h));
        Some(if moved {
            format!("Moved. {}.", self.mark_words())
        } else {
            "It cannot move any further that way.".to_string()
        })
    }

    /// Make the selected mark wider or narrower (dx) or taller or shorter (dy), by its bottom right handle (a line's end).
    pub fn size_mark(&mut self, dx: i32, dy: i32) -> Said {
        let kind = self.selected_mark()?.kind;
        if !marks::can_resize(kind) {
            return Some("That mark has no size to change.".to_string());
        }
        let handle = if matches!(kind, AnnotationKind::Line | AnnotationKind::Arrow) {
            Handle::End
        } else {
            Handle::BottomRight
        };
        let changed = self.change_mark(|m, w, h| marks::resize(m, handle, dx, dy, w, h));
        Some(if changed {
            format!("Resized. {}.", self.mark_words())
        } else {
            "It cannot change any further that way.".to_string()
        })
    }

    pub fn set_mark_colour(&mut self, name: &str, hex: &str) -> Said {
        self.selected_mark.as_ref()?;
        self.change_mark(|m, _, _| marks::set_colour(m, hex));
        Some(format!("The mark is now {name}."))
    }

    pub fn set_mark_width(&mut self, width: i32) -> Said {
        self.selected_mark.as_ref()?;
        self.change_mark(|m, _, _| m.stroke_width = Some(width));
        Some(format!("The line is now {width} pixels wide."))
    }

    pub fn set_mark_text(&mut self, text: &str) -> Said {
        self.selected_mark.as_ref()?;
        let words = text.to_string();
        self.change_mark(|m, _, _| m.text = Some(words));
        Some(format!("The words are now {text:?}."))
    }

    pub fn set_mark_number(&mut self, number: u32) -> Said {
        self.selected_mark.as_ref()?;
        self.change_mark(|m, _, _| m.number = Some(number.max(1)));
        Some(format!("The step number is now {}.", number.max(1)))
    }

    /// Delete the selected mark. Undo brings it back.
    pub fn delete_mark(&mut self) -> Said {
        self.commit_pending();
        let (step_id, mark_id) = (self.selected.clone()?, self.selected_mark.clone()?);
        let words = self.mark_words();
        let now = utc_now_iso();
        match self.session.apply(|d| d.remove_annotation(&step_id, &mark_id, &now)) {
            Ok(()) => {
                self.selected_mark = None;
                self.drag = None;
                Some(format!("Deleted: {words}. Undo brings it back."))
            }
            Err(_) => Some("The mark could not be deleted.".to_string()),
        }
    }

    // ---- dragging a mark with the mouse (picture pixels)

    /// A press at a point of the picture: pick the mark under it (or a handle of the selected mark) and begin a drag. Returns whether
    /// anything was picked. Nothing changes in the guide until [EditorState::end_drag].
    pub fn begin_drag(&mut self, x: i32, y: i32) -> bool {
        let Some((_, h)) = self.picture_size() else { return false };
        self.drag = None;
        if let Some(mark) = self.selected_mark_in_document() {
            if let Some(handle) = marks::handle_at(&mark, x, y, h) {
                self.drag = Some(Drag { kind: DragKind::Resize(handle), original: mark.clone(), current: mark });
                return true;
            }
        }
        let marks_now = self.selected_step().map(|s| s.annotations.clone()).unwrap_or_default();
        match marks::top_mark_at(&marks_now, x, y, h) {
            Some(index) => {
                let mark = marks_now[index].clone();
                self.selected_mark = Some(mark.id.clone());
                self.drag = Some(Drag { kind: DragKind::Move, original: mark.clone(), current: mark });
                true
            }
            None => {
                self.selected_mark = None;
                false
            }
        }
    }

    /// The pointer has moved dx, dy picture pixels since the press.
    pub fn drag_by(&mut self, dx: i32, dy: i32) {
        let Some((w, h)) = self.picture_size() else { return };
        let Some(drag) = self.drag.as_mut() else { return };
        let mut mark = drag.original.clone();
        match drag.kind {
            DragKind::Move => marks::nudge(&mut mark, dx, dy, w, h),
            DragKind::Resize(handle) => marks::resize(&mut mark, handle, dx, dy, w, h),
        }
        drag.current = mark;
    }

    /// The mouse was let go: keep what was dragged as one change. Returns what to say if the mark moved or changed size.
    pub fn end_drag(&mut self) -> Said {
        let drag = self.drag.take()?;
        if drag.current == drag.original {
            return None;
        }
        self.commit_pending();
        let step_id = self.selected.clone()?;
        let now = utc_now_iso();
        let kind = drag.kind;
        self.session.apply(|d| d.replace_annotation(&step_id, drag.current, &now)).ok()?;
        let words = self.mark_words();
        Some(match kind {
            DragKind::Move => format!("Moved. {words}."),
            DragKind::Resize(_) => format!("Resized. {words}."),
        })
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The selected step's picture with its marks, as a texture the window can draw. It is the whole picture, not the focused
    /// crop (the crop is drawn over it as a frame), so that a mark can be worked on anywhere. Made again when the step or its
    /// marks change, or while a mark is dragged.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let mut step = self.selected_step()?.clone();
        if let Some(drag) = &self.drag {
            for m in step.annotations.iter_mut().filter(|m| m.id == drag.current.id) {
                *m = drag.current.clone();
            }
        }
        let image = step.image.as_ref()?;
        let key = format!("{}|{:?}", step.id, step.annotations);
        if let Some((have, texture)) = &self.texture {
            if *have == key {
                return Some(texture.clone());
            }
        }
        let picture = self.pictures.picture(&image.file)?;
        let rendered = render_marked(&picture, &step, &BlockText);
        let canvas = rendered.canvas;
        let rgba: Vec<u8> = canvas.bgra.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let size = [canvas.width as usize, canvas.height as usize];
        let texture = ctx.load_texture(
            format!("step-{}", step.id),
            egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
            egui::TextureOptions::LINEAR,
        );
        self.texture = Some((key, texture.clone()));
        Some(texture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_export::demo::demo;

    fn editor() -> EditorState {
        EditorState::new(demo())
    }

    fn titles(e: &EditorState) -> Vec<String> {
        e.rows().iter().map(Row::label).collect()
    }

    #[test]
    fn it_opens_on_the_first_step_with_its_words_ready_to_edit() {
        let e = editor();
        assert_eq!(e.selected_id(), Some("step-1"));
        assert_eq!(e.buffer.for_step.as_deref(), Some("step-1"));
        assert_eq!(e.buffer.title, "Click \"Assignments\"");
        assert!(!e.can_undo() && !e.can_redo());
    }

    #[test]
    fn typing_is_kept_as_one_change_and_undo_takes_back_the_whole_edit() {
        let mut e = editor();
        e.buffer.title = "Open the Assignments page".to_string();
        e.buffer.description = "A new description.".to_string();
        e.commit_pending();
        assert_eq!(e.selected_step().unwrap().title, "Open the Assignments page");
        assert_eq!(e.selected_step().unwrap().description, "A new description.");
        e.commit_pending();
        assert!(e.can_undo());
        assert_eq!(e.undo().as_deref(), Some("Undone."));
        assert_eq!(e.selected_step().unwrap().description, demo().document.guide.steps[0].description);
        assert_eq!(e.undo().as_deref(), Some("Undone."));
        assert_eq!(e.selected_step().unwrap().title, "Click \"Assignments\"");
        assert_eq!(e.buffer.title, "Click \"Assignments\"", "the box shows the step again");
        assert_eq!(e.undo().as_deref(), Some("There is nothing to undo."));
        assert!(e.redo().is_some() && e.selected_step().unwrap().title == "Open the Assignments page");
    }

    #[test]
    fn committing_when_nothing_changed_adds_nothing_to_undo() {
        let mut e = editor();
        e.commit_pending();
        e.commit_pending();
        assert!(!e.can_undo());
    }

    #[test]
    fn choosing_another_step_keeps_what_was_typed_for_the_first() {
        let mut e = editor();
        e.buffer.title = "Typed but not left the box".to_string();
        e.select("step-2");
        assert_eq!(e.doc().guide.step("step-1").unwrap().title, "Typed but not left the box");
        assert_eq!(e.buffer.for_step.as_deref(), Some("step-2"));
        assert_eq!(e.buffer.title, "Click \"New assignment\"");
    }

    #[test]
    fn selecting_a_step_that_is_not_there_keeps_the_selection() {
        let mut e = editor();
        e.select("step-3");
        e.select("no-such-step");
        assert_eq!(e.selected_id(), Some("step-3"));
    }

    #[test]
    fn a_step_moves_up_and_down_and_the_words_say_where_it_is_now() {
        let mut e = editor();
        e.select("step-2");
        let said = e.move_selected(1).unwrap();
        assert!(said.starts_with("Moved down. Now "), "{said}");
        assert_eq!(e.selected_id(), Some("step-2"), "the same step stays selected");
        let said = e.move_selected(-1).unwrap();
        assert!(said.starts_with("Moved up. Now 2."), "{said}");
        assert_eq!(titles(&e), titles(&editor()));
        e.select("step-1");
        assert_eq!(e.move_selected(-1).as_deref(), Some("Cannot move up any further."));
        assert!(e.undo().is_some());
    }

    #[test]
    fn deleting_selects_the_next_step_and_undo_brings_the_step_back() {
        let mut e = editor();
        let course = e.rows().into_iter().find(|r| r.label() == "3. Select \"Course\"").unwrap().id;
        e.select(&course);
        let before = titles(&e);
        let said = e.delete_selected().unwrap();
        assert!(said.starts_with("Deleted 3. Select \"Course\"."), "{said}");
        assert!(said.ends_with("Undo brings it back."));
        assert_eq!(e.rows().len(), before.len() - 1);
        assert!(e.selected_id().is_some() && e.selected_id() != Some(course.as_str()));
        e.undo();
        assert_eq!(titles(&e), before);
    }

    #[test]
    fn deleting_the_last_step_selects_the_one_before_and_deleting_everything_leaves_no_selection() {
        let mut e = editor();
        let last = e.rows().last().unwrap().id.clone();
        e.select(&last);
        e.delete_selected();
        assert!(e.selected_id().is_some());
        for _ in 0..10 {
            e.delete_selected();
        }
        assert!(e.rows().is_empty());
        assert_eq!(e.selected_id(), None);
        assert_eq!(e.delete_selected(), None);
        let guide_title = e.doc().guide.title.clone();
        assert_eq!(
            e.buffer,
            Buffer { guide_title, ..Buffer::default() },
            "no step's words, but the guide's title is still there"
        );
    }

    #[test]
    fn the_guide_can_be_renamed_as_one_undo_and_an_empty_title_is_not_kept() {
        let mut e = editor();
        let depth = e.session.undo_depth();
        e.buffer.guide_title = "  Submitting an assignment  ".to_string();
        e.commit_pending();
        assert_eq!(e.doc().guide.title, "Submitting an assignment");
        assert_eq!(e.session.undo_depth(), depth + 1);
        e.buffer.guide_title = "   ".to_string();
        e.commit_pending();
        assert_eq!(e.doc().guide.title, "Submitting an assignment", "a title with nothing in it is refused");
        assert_eq!(e.buffer.guide_title, "Submitting an assignment", "and the old one comes back into the box");
        e.undo();
        assert_eq!(e.doc().guide.title, "How to submit an assignment");
        assert_eq!(e.buffer.guide_title, "How to submit an assignment");
    }

    #[test]
    fn a_duplicated_step_is_a_copy_straight_after_it_and_is_chosen() {
        let mut e = editor();
        let first = e.selected_id().unwrap().to_string();
        let count = e.doc().guide.steps.len();
        let said = e.duplicate_selected().unwrap();
        assert!(said.starts_with("Duplicated. The copy is"), "{said}");
        assert_eq!(e.doc().guide.steps.len(), count + 1);
        assert_ne!(e.selected_id(), Some(first.as_str()));
        assert_eq!(e.selected_step().unwrap().title, e.doc().guide.step(&first).unwrap().title);
        assert_eq!(e.neighbour(-1).as_deref(), Some(first.as_str()), "straight after the original");
        e.undo();
        assert_eq!(e.doc().guide.steps.len(), count);
    }

    #[test]
    fn a_new_step_goes_after_the_selected_one_and_asks_for_its_title() {
        let mut e = editor();
        e.select("step-2");
        let said = e.add_step().unwrap();
        assert!(said.starts_with("Added an empty step"), "{said}");
        let labels = titles(&e);
        let at = labels.iter().position(|l| l.contains("New step")).unwrap();
        assert!(labels[at - 1].contains("2.1."), "after the substep inside step 2: {labels:?}");
        assert!(e.focus_title_next);
        assert_eq!(e.selected_step().unwrap().title, "New step");
        e.undo();
        assert_eq!(titles(&e), titles(&editor()));
    }

    #[test]
    fn skipping_a_step_says_so_and_closes_up_the_numbers() {
        let mut e = editor();
        e.select("step-2");
        let said = e.set_skipped(true).unwrap();
        assert!(said.contains("left out of exports"), "{said}");
        let labels = titles(&e);
        assert!(labels.iter().any(|l| l.contains("(skipped)")), "{labels:?}");
        assert!(labels.iter().any(|l| l.starts_with("2. Select \"Course\"")), "step 3 became step 2: {labels:?}");
        assert!(e.set_skipped(false).unwrap().contains("in exports"));
    }

    #[test]
    fn the_focused_view_needs_a_picture_and_a_click_and_changes_the_crop() {
        let mut e = editor();
        e.select("step-1");
        assert!(e.can_focus());
        let was = e.is_focused();
        assert!(e.set_focused(!was).is_some());
        assert_eq!(e.is_focused(), !was);
        e.set_focused(true);
        let crop = e.selected_step().unwrap().focus.clone().unwrap();
        assert!(crop.width > 0 && crop.height > 0);
        e.set_focused(false);
        assert!(!e.is_focused());
        let tip = e.rows().iter().find(|r| r.kind == openstage_express_core::StepKind::Block).unwrap().id.clone();
        e.select(&tip);
        assert!(!e.can_focus(), "a note has no picture");
        assert_eq!(e.set_focused(true), None);
    }

    #[test]
    fn the_pictures_words_are_the_authors_or_the_title() {
        let mut e = editor();
        e.select("step-1");
        assert_eq!(e.picture_words(), "The course site with the Assignments item in the left menu circled");
        e.buffer.picture_description = String::new();
        e.commit_pending();
        assert_eq!(e.picture_words(), "Click \"Assignments\"", "no description, so the title");
    }

    #[test]
    fn neighbours_are_found_in_the_list_order_and_stop_at_the_ends() {
        let mut e = editor();
        e.select("step-1");
        assert_eq!(e.neighbour(-1), None);
        assert_eq!(e.neighbour(1).as_deref(), Some("step-2"));
        let last = e.rows().last().unwrap().id.clone();
        e.select(&last);
        assert_eq!(e.neighbour(1), None);
    }

    // ---- marks and dragging

    fn with_a_rectangle() -> (EditorState, Annotation) {
        let mut e = editor();
        e.add_mark(Tool::Rectangle);
        let mark = e.selected_mark().expect("the new mark is chosen");
        (e, mark)
    }

    fn middle_of(m: &Annotation) -> (i32, i32) {
        (m.x + m.width.unwrap() / 2, m.y + m.height.unwrap() / 2)
    }

    #[test]
    fn the_editor_shows_the_whole_picture_with_the_crop_drawn_over_it_not_the_cropped_picture() {
        let ctx = egui::Context::default();
        let mut e = editor();
        assert!(e.is_focused(), "the example's first step has a focused crop");
        let (w, h) = e.picture_size().unwrap();
        let texture = e.texture(&ctx).unwrap();
        assert_eq!([w as usize, h as usize], texture.size());
    }

    #[test]
    fn a_drag_shows_the_mark_moved_but_changes_the_guide_only_when_the_mouse_is_let_go_and_then_once() {
        let (mut e, start) = with_a_rectangle();
        let depth = e.session.undo_depth();
        let (cx, cy) = middle_of(&start);
        assert!(e.begin_drag(cx, cy), "the press is on the mark");
        e.drag_by(30, 20);
        e.drag_by(60, 40);
        let shown = e.selected_mark().unwrap();
        assert_eq!((shown.x, shown.y), (start.x + 60, start.y + 40), "shown where it is now");
        let in_guide = e.selected_step().unwrap().annotations.last().unwrap().clone();
        assert_eq!((in_guide.x, in_guide.y), (start.x, start.y), "the guide itself has not changed yet");
        assert_eq!(e.session.undo_depth(), depth);
        let said = e.end_drag().expect("it moved, so something is said");
        assert!(said.starts_with("Moved. Rectangle"), "{said}");
        assert_eq!(e.session.undo_depth(), depth + 1, "one change however far it was dragged");
        let kept = e.selected_step().unwrap().annotations.last().unwrap().clone();
        assert_eq!((kept.x, kept.y), (start.x + 60, start.y + 40));
        e.undo();
        let back = e.selected_step().unwrap().annotations.last().unwrap().clone();
        assert_eq!(
            (back.id.as_str(), back.x, back.y),
            (kept.id.as_str(), start.x, start.y),
            "one undo takes the whole drag back, and the mark stays"
        );
    }

    #[test]
    fn dragging_a_handle_resizes_and_a_drag_that_goes_nowhere_changes_nothing() {
        let (mut e, start) = with_a_rectangle();
        let depth = e.session.undo_depth();
        let corner = (start.x + start.width.unwrap(), start.y + start.height.unwrap());
        assert!(e.begin_drag(corner.0, corner.1));
        e.drag_by(25, 15);
        let said = e.end_drag().unwrap();
        assert!(said.starts_with("Resized."), "{said}");
        let grown = e.selected_mark().unwrap();
        assert_eq!((grown.width, grown.height), (Some(start.width.unwrap() + 25), Some(start.height.unwrap() + 15)));
        assert_eq!(e.session.undo_depth(), depth + 1);
        // Pressing and letting go on the same spot is a click, not a change.
        let (cx, cy) = middle_of(&grown);
        assert!(e.begin_drag(cx, cy));
        assert_eq!(e.end_drag(), None);
        assert_eq!(e.session.undo_depth(), depth + 1, "nothing was added to undo");
    }

    #[test]
    fn pressing_on_empty_picture_chooses_no_mark_and_pressing_on_a_mark_chooses_it() {
        let (mut e, start) = with_a_rectangle();
        assert!(!e.begin_drag(2, 2));
        assert!(e.selected_mark().is_none());
        let (cx, cy) = middle_of(&start);
        assert!(e.begin_drag(cx, cy));
        assert_eq!(e.selected_mark().map(|m| m.id), Some(start.id));
    }

    #[test]
    fn choosing_another_step_forgets_the_chosen_mark_and_a_text_mark_keeps_its_words() {
        let (mut e, _) = with_a_rectangle();
        let first = e.selected_id().unwrap().to_string();
        let next = e.neighbour(1).unwrap();
        e.select(&next);
        assert!(e.selected_mark().is_none());
        e.select(&first);
        assert!(e.selected_mark().is_none(), "and it is not chosen again by itself");
        e.add_mark(Tool::Text);
        assert_eq!(e.set_mark_text("Press here").as_deref(), Some("The words are now \"Press here\"."));
        assert_eq!(e.selected_mark().unwrap().text.as_deref(), Some("Press here"));
        e.sync_mark_words();
        assert_eq!(e.mark_words, "Press here");
    }

    #[test]
    fn marks_are_kept_on_the_picture_whatever_is_asked() {
        let (mut e, _) = with_a_rectangle();
        let (w, h) = e.picture_size().unwrap();
        for _ in 0..200 {
            e.nudge_mark(-5000, -5000);
        }
        let m = e.selected_mark().unwrap();
        assert_eq!((m.x, m.y), (0, 0));
        e.nudge_mark(100_000, 100_000);
        let m = e.selected_mark().unwrap();
        assert!(m.x + m.width.unwrap() <= w as i32 && m.y + m.height.unwrap() <= h as i32);
    }

    #[test]
    fn each_new_step_number_is_the_next_one() {
        let mut e = editor();
        e.add_mark(Tool::Number);
        assert_eq!(e.selected_mark().unwrap().number, Some(1));
        e.add_mark(Tool::Number);
        assert_eq!(e.selected_mark().unwrap().number, Some(2));
        e.add_mark(Tool::Rectangle);
        e.add_mark(Tool::Number);
        assert_eq!(e.selected_mark().unwrap().number, Some(3), "other kinds of mark do not count");
    }
}
