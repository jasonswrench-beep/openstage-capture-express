// SPDX-License-Identifier: Apache-2.0
//! The editor's state, with no drawing in it: which step is selected, what has been typed but not yet kept, and every change a
//! person can make (each one undoable). The screens call these; the tests call them too.
//!
//! What is typed into the title, the description or the picture description is held in a buffer and kept as *one* change when
//! the person leaves the box, picks another step, or does anything else, so undo takes back a whole edit and never one letter.

use eframe::egui;
use openstage_capture_render::BlockText;
use openstage_express_core::focus::{auto_focus, DEFAULT_ZOOM_PERCENT};
use openstage_express_core::fsutil::utc_now_iso;
use openstage_express_core::render::render_step;
use openstage_express_core::{Document, Position, Session, Step, StepDraft};
use openstage_express_export::demo::Demo;
use openstage_express_export::picture::{MemoryPictures, PictureSource};

use super::rows::{rows, Row};

/// What has been typed for the selected step.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Buffer {
    /// The step these words are for.
    pub for_step: Option<String>,
    pub title: String,
    pub description: String,
    pub picture_description: String,
}

pub struct EditorState {
    session: Session,
    pictures: MemoryPictures,
    selected: Option<String>,
    pub buffer: Buffer,
    texture: Option<(String, egui::TextureHandle)>,
    /// Set by F2: the screen moves the keyboard to the title box.
    pub focus_title_next: bool,
    /// Set when Up or Down (or a click) chose a step: the screen moves the keyboard to that row.
    pub focus_row_next: bool,
}

/// The words in the status line after something happens (a screen reader says them).
pub type Said = Option<String>;

impl EditorState {
    /// An editor over the example guide.
    pub fn new(demo: Demo) -> EditorState {
        let mut state = EditorState {
            session: Session::new(demo.document),
            pictures: demo.pictures,
            selected: None,
            buffer: Buffer::default(),
            texture: None,
            focus_title_next: false,
            focus_row_next: false,
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
            self.selected = Some(id.to_string());
        }
        self.reload_buffer();
    }

    fn reload_buffer(&mut self) {
        self.buffer = match self.selected_step() {
            Some(step) => Buffer {
                for_step: Some(step.id.clone()),
                title: step.title.clone(),
                description: step.description.clone(),
                picture_description: step.image_description.clone(),
            },
            None => Buffer::default(),
        };
    }

    /// Keep what has been typed, as one undoable change for each box that differs from the guide.
    pub fn commit_pending(&mut self) {
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

    /// The selected step's picture with its marks, as a texture the window can draw. Made again when the step, its marks or
    /// its focused view changes.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let step = self.selected_step()?.clone();
        let image = step.image.as_ref()?;
        let key = format!(
            "{}|{:?}|{}",
            step.id,
            step.focus.as_ref().map(|f| (f.x, f.y, f.width, f.height)),
            step.annotations.len()
        );
        if let Some((have, texture)) = &self.texture {
            if *have == key {
                return Some(texture.clone());
            }
        }
        let picture = self.pictures.picture(&image.file)?;
        let rendered = render_step(&picture, &step, &BlockText);
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
        assert_eq!(e.buffer, Buffer::default());
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
}
