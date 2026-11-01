// SPDX-License-Identifier: Apache-2.0
//! The things a person can do to a guide (spec sections 5.2 and 9): add, edit, reorder, nest, duplicate, skip, delete
//! to the trash and restore. Each is a method on [`Document`], which is a guide plus its trash.
//!
//! The steps are kept in one flat list in depth-first order: a step is followed by all its substeps, then by its next
//! sibling. Every structural operation edits a tree of ids and then writes the list out again in that order, so the
//! rule "a child always comes after its parent" holds after every operation by construction.
//!
//! Ids are never reused: `next_step_number` only goes up (and [`crate::Session`] keeps it up across undo).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::{
    step_number, Annotation, Click, Control, Extra, Guide, Rect, Step, StepImage, StepKind, SCHEMA_VERSION,
};

/// Why an operation could not be done. The text is written for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    #[error("There is no step {0}.")]
    NoSuchStep(String),
    #[error("There is nothing in the trash for step {0}.")]
    NotInTrash(String),
    #[error("A step cannot be placed inside itself or inside one of its own substeps.")]
    WouldNestInsideItself,
    #[error("There is no earlier step at the same level to put this step under.")]
    NothingToNestUnder,
    #[error("This step is already at the top level.")]
    AlreadyTopLevel,
    #[error("A picture step needs a picture, and the other kinds of step cannot have one.")]
    PictureMismatch,
    #[error("The mark {0} does not exist on this step.")]
    NoSuchMark(String),
    #[error("This step already has a mark called {0}.")]
    MarkExists(String),
    #[error("The step number {0} is already used, so the step cannot be put back.")]
    IdInUse(String),
}

/// A step that was deleted and can be put back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashEntry {
    pub step: Step,
    pub deleted_at: String,
    /// The parent the step had when it was deleted.
    pub was_parent: Option<String>,
    /// Its place among its brothers and sisters.
    pub was_index: usize,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Deleted steps, kept until the person empties the trash (`trash/trash.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trash {
    pub schema_version: String,
    #[serde(default)]
    pub entries: Vec<TrashEntry>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Trash {
    pub fn new() -> Trash {
        Trash { schema_version: SCHEMA_VERSION.to_string(), entries: Vec::new(), extra: Extra::new() }
    }
}

impl Default for Trash {
    fn default() -> Trash {
        Trash::new()
    }
}

/// A guide and its trash: the thing that is edited, saved, and snapshotted for undo.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub guide: Guide,
    pub trash: Trash,
}

/// Where a new step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position<'a> {
    /// At the very end, at the top level (what capturing does).
    End,
    /// Straight after this step and all its substeps, at the same level.
    After(&'a str),
    /// As the last substep of this step.
    ChildOf(&'a str),
}

/// Everything a new step has, except its id.
#[derive(Debug, Clone)]
pub struct StepDraft {
    pub kind: StepKind,
    pub title: String,
    pub description: String,
    pub image: Option<StepImage>,
    pub image_description: String,
    pub click: Option<Click>,
    pub control: Option<Control>,
    pub typed: Option<String>,
    pub program: Option<String>,
    pub focus: Option<Rect>,
    pub annotations: Vec<Annotation>,
    pub captured_at: Option<String>,
}

impl StepDraft {
    /// A step with no picture.
    pub fn empty(title: &str) -> StepDraft {
        StepDraft {
            kind: StepKind::Empty,
            title: title.to_string(),
            description: String::new(),
            image: None,
            image_description: String::new(),
            click: None,
            control: None,
            typed: None,
            program: None,
            focus: None,
            annotations: Vec::new(),
            captured_at: None,
        }
    }

    /// A picture step.
    pub fn with_picture(title: &str, image: StepImage) -> StepDraft {
        StepDraft { kind: StepKind::Image, image: Some(image), ..StepDraft::empty(title) }
    }
}

/// The steps as a tree of ids, so that moving and nesting never needs index arithmetic on the flat list.
struct Tree {
    kids: BTreeMap<Option<String>, Vec<String>>,
}

impl Tree {
    /// Read the tree from the steps. A step whose parent is missing, or is itself, becomes top level.
    fn from_steps(steps: &[Step]) -> Tree {
        let ids: BTreeSet<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        let mut kids: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
        for step in steps {
            let parent = step.parent.as_ref().filter(|p| *p != &step.id && ids.contains(p.as_str())).cloned();
            kids.entry(parent).or_default().push(step.id.clone());
        }
        Tree { kids }
    }

    fn children(&self, parent: &Option<String>) -> &[String] {
        self.kids.get(parent).map(Vec::as_slice).unwrap_or(&[])
    }

    fn siblings_mut(&mut self, parent: &Option<String>) -> &mut Vec<String> {
        self.kids.entry(parent.clone()).or_default()
    }

    /// The parent of `id` and its index among the parent's children.
    fn locate(&self, id: &str) -> Option<(Option<String>, usize)> {
        for (parent, list) in &self.kids {
            if let Some(index) = list.iter().position(|k| k == id) {
                return Some((parent.clone(), index));
            }
        }
        None
    }

    fn is_inside(&self, ancestor: &str, candidate: &str) -> bool {
        let mut stack = vec![ancestor.to_string()];
        while let Some(current) = stack.pop() {
            if current == candidate {
                return true;
            }
            stack.extend(self.children(&Some(current)).iter().cloned());
        }
        false
    }

    /// The step and everything under it, in order.
    fn subtree(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(current) = stack.pop() {
            out.push(current.clone());
            let kids = self.children(&Some(current));
            stack.extend(kids.iter().rev().cloned());
        }
        out
    }

    /// Write the steps out in depth-first order with their `parent` fields set from the tree. Steps the tree does not
    /// reach (only possible from a damaged file) are put at the end at the top level, so no step is ever lost.
    fn write_out(&self, steps: Vec<Step>) -> Vec<Step> {
        let mut by_id: BTreeMap<String, Step> = steps.into_iter().map(|s| (s.id.clone(), s)).collect();
        let mut out = Vec::with_capacity(by_id.len());
        let mut stack: Vec<(Option<String>, String)> =
            self.children(&None).iter().rev().map(|id| (None, id.clone())).collect();
        while let Some((parent, id)) = stack.pop() {
            if let Some(mut step) = by_id.remove(&id) {
                step.parent = parent;
                for kid in self.children(&Some(id.clone())).iter().rev() {
                    stack.push((Some(id.clone()), kid.clone()));
                }
                out.push(step);
            }
        }
        for (_, mut step) in by_id {
            step.parent = None;
            out.push(step);
        }
        out
    }
}

impl Document {
    pub fn new(guide: Guide) -> Document {
        Document { guide, trash: Trash::new() }
    }

    fn touch(&mut self, now: &str) {
        self.guide.updated_at = now.to_string();
    }

    fn require(&self, id: &str) -> Result<(), OpError> {
        if self.guide.step(id).is_some() {
            Ok(())
        } else {
            Err(OpError::NoSuchStep(id.to_string()))
        }
    }

    fn step_mut(&mut self, id: &str) -> Result<&mut Step, OpError> {
        self.guide.steps.iter_mut().find(|s| s.id == id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))
    }

    /// Take the next free step id. It is never given out twice.
    fn allocate_id(&mut self) -> String {
        let id = format!("step-{}", self.guide.next_step_number);
        self.guide.next_step_number += 1;
        id
    }

    /// Rewrite the step list from an edited tree.
    fn commit_tree(&mut self, tree: &Tree) {
        let steps = std::mem::take(&mut self.guide.steps);
        self.guide.steps = tree.write_out(steps);
    }

    // ---- adding

    /// Add a step and return its new id.
    pub fn add_step(&mut self, draft: StepDraft, position: Position<'_>, now: &str) -> Result<String, OpError> {
        if (draft.kind == StepKind::Image) != draft.image.is_some() {
            return Err(OpError::PictureMismatch);
        }
        let mut tree = Tree::from_steps(&self.guide.steps);
        let (parent, index) = match position {
            Position::End => (None, tree.children(&None).len()),
            Position::After(id) => {
                self.require(id)?;
                let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
                (parent, index + 1)
            }
            Position::ChildOf(id) => {
                self.require(id)?;
                let parent = Some(id.to_string());
                let end = tree.children(&parent).len();
                (parent, end)
            }
        };
        let id = self.allocate_id();
        let mut step = Step::bare(&id, draft.kind, &draft.title);
        step.description = draft.description;
        step.image = draft.image;
        step.image_description = draft.image_description;
        step.click = draft.click;
        step.control = draft.control;
        step.typed = draft.typed;
        step.program = draft.program;
        step.focus = draft.focus;
        step.annotations = draft.annotations;
        step.captured_at = draft.captured_at;
        step.parent = parent.clone();
        self.guide.steps.push(step);
        tree.siblings_mut(&parent).insert(index, id.clone());
        self.commit_tree(&tree);
        self.touch(now);
        Ok(id)
    }

    // ---- editing a step's words and settings

    pub fn set_title(&mut self, id: &str, title: &str, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(id)?;
        if step.title != title {
            step.title = title.to_string();
            self.touch(now);
        }
        Ok(())
    }

    pub fn set_description(&mut self, id: &str, text: &str, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(id)?;
        if step.description != text {
            step.description = text.to_string();
            self.touch(now);
        }
        Ok(())
    }

    pub fn set_image_description(&mut self, id: &str, text: &str, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(id)?;
        if step.image_description != text {
            step.image_description = text.to_string();
            self.touch(now);
        }
        Ok(())
    }

    /// Leave a step out of every export, or put it back.
    pub fn set_skipped(&mut self, id: &str, skipped: bool, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(id)?;
        if step.skipped != skipped {
            step.skipped = skipped;
            self.touch(now);
        }
        Ok(())
    }

    /// Set the focused view crop of one step. None shows the whole picture.
    pub fn set_focus(&mut self, id: &str, focus: Option<Rect>, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(id)?;
        if step.focus != focus {
            step.focus = focus;
            self.touch(now);
        }
        Ok(())
    }

    pub fn set_guide_title(&mut self, title: &str, now: &str) {
        if self.guide.title != title {
            self.guide.title = title.to_string();
            self.touch(now);
        }
    }

    pub fn set_guide_description(&mut self, text: &str, now: &str) {
        if self.guide.description != text {
            self.guide.description = text.to_string();
            self.touch(now);
        }
    }

    pub fn set_guide_language(&mut self, language: &str, now: &str) {
        if self.guide.language != language {
            self.guide.language = language.to_string();
            self.touch(now);
        }
    }

    /// The default for the guide's focused view (each step can still choose for itself).
    pub fn set_focused_view(&mut self, on: bool, now: &str) {
        if self.guide.focused_view != on {
            self.guide.focused_view = on;
            self.touch(now);
        }
    }

    // ---- marks on a picture

    /// Add a mark to a step. A mark with an empty id gets the next free one (`a1`, `a2`, ...). Returns its id.
    pub fn add_annotation(&mut self, step_id: &str, mut mark: Annotation, now: &str) -> Result<String, OpError> {
        let step = self.step_mut(step_id)?;
        if mark.id.is_empty() {
            let highest = step
                .annotations
                .iter()
                .filter_map(|a| a.id.strip_prefix('a').and_then(|n| n.parse::<u32>().ok()))
                .max()
                .unwrap_or(0);
            mark.id = format!("a{}", highest + 1);
        } else if step.annotations.iter().any(|a| a.id == mark.id) {
            return Err(OpError::MarkExists(mark.id));
        }
        let id = mark.id.clone();
        step.annotations.push(mark);
        self.touch(now);
        Ok(id)
    }

    /// Replace the mark that has the same id (to move it, resize it or restyle it).
    pub fn replace_annotation(&mut self, step_id: &str, mark: Annotation, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(step_id)?;
        let slot = step
            .annotations
            .iter_mut()
            .find(|a| a.id == mark.id)
            .ok_or_else(|| OpError::NoSuchMark(mark.id.clone()))?;
        if *slot != mark {
            *slot = mark;
            self.touch(now);
        }
        Ok(())
    }

    pub fn remove_annotation(&mut self, step_id: &str, mark_id: &str, now: &str) -> Result<(), OpError> {
        let step = self.step_mut(step_id)?;
        let before = step.annotations.len();
        step.annotations.retain(|a| a.id != mark_id);
        if step.annotations.len() == before {
            return Err(OpError::NoSuchMark(mark_id.to_string()));
        }
        self.touch(now);
        Ok(())
    }

    // ---- order and nesting

    /// Move a step (with its substeps) up or down among its brothers and sisters. Stops at the ends.
    pub fn move_step_by(&mut self, id: &str, delta: i32, now: &str) -> Result<(), OpError> {
        self.require(id)?;
        let tree = Tree::from_steps(&self.guide.steps);
        let (_, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        let target = (index as i64 + delta as i64).max(0) as usize;
        self.move_step_to(id, target, now)
    }

    /// Move a step to place `index` among its brothers and sisters (0 is first). A number past the end means last.
    pub fn move_step_to(&mut self, id: &str, index: usize, now: &str) -> Result<(), OpError> {
        self.require(id)?;
        let mut tree = Tree::from_steps(&self.guide.steps);
        let (parent, old) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        let list = tree.siblings_mut(&parent);
        let new = index.min(list.len() - 1);
        if new == old {
            return Ok(());
        }
        let moved = list.remove(old);
        list.insert(new, moved);
        self.commit_tree(&tree);
        self.touch(now);
        Ok(())
    }

    /// Move several steps (each with its substeps) up (`delta` below 0) or down (above 0) one place at a time, `delta` times, among
    /// their brothers and sisters, as one change. A step chosen together with the step it is inside is carried by that step. Chosen
    /// steps that sit together move past the step beside them as a block; a chosen step already at the end of its level stays, and
    /// so do the chosen steps packed against it. Returns whether anything moved (if not, nothing is changed, not even the time).
    pub fn move_steps_by(&mut self, ids: &[String], delta: i32, now: &str) -> Result<bool, OpError> {
        for id in ids {
            self.require(id)?;
        }
        let mut tree = Tree::from_steps(&self.guide.steps);
        let tops = self.top_chosen(&tree, ids);
        let chosen: BTreeSet<&str> = tops.iter().map(String::as_str).collect();
        let mut parents: Vec<Option<String>> = tops.iter().filter_map(|id| tree.locate(id).map(|(p, _)| p)).collect();
        parents.sort();
        parents.dedup();
        let mut moved = false;
        for _ in 0..delta.unsigned_abs() {
            for parent in &parents {
                let list = tree.siblings_mut(parent);
                if delta < 0 {
                    for i in 1..list.len() {
                        if chosen.contains(list[i].as_str()) && !chosen.contains(list[i - 1].as_str()) {
                            list.swap(i, i - 1);
                            moved = true;
                        }
                    }
                } else {
                    for i in (0..list.len().saturating_sub(1)).rev() {
                        if chosen.contains(list[i].as_str()) && !chosen.contains(list[i + 1].as_str()) {
                            list.swap(i, i + 1);
                            moved = true;
                        }
                    }
                }
            }
        }
        if moved {
            self.commit_tree(&tree);
            self.touch(now);
        }
        Ok(moved)
    }

    /// Make a step a substep of the one just before it at the same level.
    pub fn indent(&mut self, id: &str, now: &str) -> Result<(), OpError> {
        self.require(id)?;
        let mut tree = Tree::from_steps(&self.guide.steps);
        let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        if index == 0 {
            return Err(OpError::NothingToNestUnder);
        }
        let above = tree.children(&parent)[index - 1].clone();
        tree.siblings_mut(&parent).remove(index);
        tree.siblings_mut(&Some(above)).push(id.to_string());
        self.commit_tree(&tree);
        self.touch(now);
        Ok(())
    }

    /// Take a substep out of its parent and put it straight after the parent.
    pub fn outdent(&mut self, id: &str, now: &str) -> Result<(), OpError> {
        self.require(id)?;
        let mut tree = Tree::from_steps(&self.guide.steps);
        let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        let Some(parent_id) = parent.clone() else {
            return Err(OpError::AlreadyTopLevel);
        };
        let (grandparent, parent_index) =
            tree.locate(&parent_id).ok_or_else(|| OpError::NoSuchStep(parent_id.clone()))?;
        tree.siblings_mut(&parent).remove(index);
        tree.siblings_mut(&grandparent).insert(parent_index + 1, id.to_string());
        self.commit_tree(&tree);
        self.touch(now);
        Ok(())
    }

    /// Move a step (with its substeps) to be the last substep of `new_parent`, or the last top-level step if none.
    pub fn change_parent(&mut self, id: &str, new_parent: Option<&str>, now: &str) -> Result<(), OpError> {
        self.require(id)?;
        if let Some(target) = new_parent {
            self.require(target)?;
        }
        let mut tree = Tree::from_steps(&self.guide.steps);
        if let Some(target) = new_parent {
            if tree.is_inside(id, target) {
                return Err(OpError::WouldNestInsideItself);
            }
        }
        let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        let destination = new_parent.map(str::to_string);
        if parent == destination && index + 1 == tree.children(&parent).len() {
            return Ok(());
        }
        tree.siblings_mut(&parent).remove(index);
        tree.siblings_mut(&destination).push(id.to_string());
        self.commit_tree(&tree);
        self.touch(now);
        Ok(())
    }

    // ---- duplicate, delete, restore

    /// Copy a step and its substeps, straight after the original. Every copy gets a new id and keeps its marks and
    /// its picture file (pictures are never modified, so copies can share one). Returns the new id of the step.
    pub fn duplicate_step(&mut self, id: &str, now: &str) -> Result<String, OpError> {
        self.require(id)?;
        let mut tree = Tree::from_steps(&self.guide.steps);
        let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        let top = self.copy_subtree(&mut tree, id, &parent)?;
        tree.siblings_mut(&parent).insert(index + 1, top.clone());
        self.commit_tree(&tree);
        self.touch(now);
        Ok(top)
    }

    /// Copy a step and everything under it with new ids, adding the copies to the step list and to `tree` (the copy's own
    /// substeps are in the tree, but the copy itself is not in any list of brothers and sisters yet: the caller puts it
    /// where it goes). `parent` is the parent the copy gets. Returns the copy's id.
    fn copy_subtree(&mut self, tree: &mut Tree, id: &str, parent: &Option<String>) -> Result<String, OpError> {
        let originals = tree.subtree(id);
        let mut renamed: BTreeMap<String, String> = BTreeMap::new();
        for old in &originals {
            let new = self.allocate_id();
            renamed.insert(old.clone(), new);
        }
        for old in &originals {
            let mut copy = self.guide.step(old).cloned().ok_or_else(|| OpError::NoSuchStep(old.clone()))?;
            copy.id = renamed[old].clone();
            copy.parent = copy.parent.as_ref().and_then(|p| renamed.get(p).cloned()).or_else(|| parent.clone());
            self.guide.steps.push(copy);
            let kids: Vec<String> =
                tree.children(&Some(old.clone())).iter().filter_map(|k| renamed.get(k).cloned()).collect();
            if !kids.is_empty() {
                tree.kids.insert(Some(renamed[old].clone()), kids);
            }
        }
        Ok(renamed[id].clone())
    }

    /// Copy several steps (each with its substeps) as one change. A step chosen together with the step it is inside is carried by
    /// that step, so it is not copied twice. Steps that sit next to each other at one level are copied as a block, straight after
    /// the last of them; steps that are apart are each copied straight after themselves. Returns the new ids of the copies, in
    /// the order of the list.
    pub fn duplicate_steps(&mut self, ids: &[String], now: &str) -> Result<Vec<String>, OpError> {
        for id in ids {
            self.require(id)?;
        }
        let mut tree = Tree::from_steps(&self.guide.steps);
        let tops = self.top_chosen(&tree, ids);
        if tops.is_empty() {
            return Ok(Vec::new());
        }
        let places: Vec<(Option<String>, usize)> = tops.iter().filter_map(|id| tree.locate(id)).collect();
        let as_a_block =
            places.len() == tops.len() && places.windows(2).all(|w| w[0].0 == w[1].0 && w[1].1 == w[0].1 + 1);
        let mut copies = Vec::with_capacity(tops.len());
        for (id, (parent, _)) in tops.iter().zip(&places) {
            copies.push(self.copy_subtree(&mut tree, id, parent)?);
        }
        if as_a_block {
            let (parent, first) = places[0].clone();
            let list = tree.siblings_mut(&parent);
            for (n, copy) in copies.iter().enumerate() {
                list.insert(first + tops.len() + n, copy.clone());
            }
        } else {
            for (id, copy) in tops.iter().zip(&copies) {
                let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.clone()))?;
                tree.siblings_mut(&parent).insert(index + 1, copy.clone());
            }
        }
        self.commit_tree(&tree);
        self.touch(now);
        Ok(copies)
    }

    /// The chosen steps that are not inside another chosen step, in the order of the list.
    fn top_chosen(&self, tree: &Tree, ids: &[String]) -> Vec<String> {
        let chosen: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
        let inside_a_chosen = |id: &str| {
            let mut at = id.to_string();
            // The walk up is cut short after as many steps as there are steps, so a damaged file with a loop cannot hang it.
            for _ in 0..self.guide.steps.len() {
                let Some((Some(parent), _)) = tree.locate(&at) else { return false };
                if chosen.contains(parent.as_str()) {
                    return true;
                }
                at = parent;
            }
            false
        };
        self.guide
            .steps
            .iter()
            .filter(|s| chosen.contains(s.id.as_str()) && !inside_a_chosen(&s.id))
            .map(|s| s.id.clone())
            .collect()
    }

    /// Delete a step to the trash. Its substeps move up one level, into its place. Its picture stays where it is.
    pub fn delete_step(&mut self, id: &str, now: &str) -> Result<(), OpError> {
        self.require(id)?;
        let mut tree = Tree::from_steps(&self.guide.steps);
        let (parent, index) = tree.locate(id).ok_or_else(|| OpError::NoSuchStep(id.to_string()))?;
        let orphans = tree.kids.remove(&Some(id.to_string())).unwrap_or_default();
        let list = tree.siblings_mut(&parent);
        list.splice(index..=index, orphans);
        let position = self.guide.steps.iter().position(|s| s.id == id).expect("checked above");
        let mut step = self.guide.steps.remove(position);
        step.parent = None;
        self.trash.entries.push(TrashEntry {
            step,
            deleted_at: now.to_string(),
            was_parent: parent,
            was_index: index,
            extra: Extra::new(),
        });
        self.commit_tree(&tree);
        self.touch(now);
        Ok(())
    }

    /// Put a step back from the trash where it was (or at the top level if its parent is gone). Substeps that moved
    /// up when it was deleted stay where they are.
    pub fn restore_step(&mut self, id: &str, now: &str) -> Result<(), OpError> {
        let position = self
            .trash
            .entries
            .iter()
            .position(|e| e.step.id == id)
            .ok_or_else(|| OpError::NotInTrash(id.to_string()))?;
        if self.guide.step(id).is_some() {
            return Err(OpError::IdInUse(id.to_string()));
        }
        let entry = self.trash.entries.remove(position);
        let mut tree = Tree::from_steps(&self.guide.steps);
        let parent = entry.was_parent.filter(|p| self.guide.step(p).is_some());
        let list = tree.siblings_mut(&parent);
        let index = entry.was_index.min(list.len());
        list.insert(index, id.to_string());
        let mut step = entry.step;
        step.parent = parent;
        self.guide.steps.push(step);
        self.commit_tree(&tree);
        self.touch(now);
        Ok(())
    }

    /// Empty the trash and hand back what was in it, so the caller can delete the pictures nothing uses any more.
    /// This is the one operation that cannot be undone.
    pub fn empty_trash(&mut self, now: &str) -> Vec<TrashEntry> {
        let entries = std::mem::take(&mut self.trash.entries);
        if !entries.is_empty() {
            self.touch(now);
        }
        entries
    }

    /// The picture files (`images/...`) the live steps use, in order, without repeats.
    pub fn live_pictures(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for step in &self.guide.steps {
            if let Some(image) = &step.image {
                if !seen.contains(&image.file) {
                    seen.push(image.file.clone());
                }
            }
        }
        seen
    }

    /// The picture files only the trash uses.
    pub fn trash_only_pictures(&self) -> Vec<String> {
        let live = self.live_pictures();
        let mut out: Vec<String> = Vec::new();
        for entry in &self.trash.entries {
            if let Some(image) = &entry.step.image {
                if !live.contains(&image.file) && !out.contains(&image.file) {
                    out.push(image.file.clone());
                }
            }
        }
        out
    }

    /// The highest step number in the guide or its trash (0 if none).
    pub fn highest_step_number(&self) -> u32 {
        self.guide
            .steps
            .iter()
            .chain(self.trash.entries.iter().map(|e| &e.step))
            .filter_map(|s| step_number(&s.id))
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const NOW: &str = "2026-10-31T15:00:00Z";
    pub(crate) const LATER: &str = "2026-10-31T15:05:00Z";

    pub(crate) fn doc() -> Document {
        Document::new(Guide::new("g-test", "Test guide", "en", "2026-10-31T14:00:00Z"))
    }

    pub(crate) fn add(doc: &mut Document, title: &str, position: Position<'_>) -> String {
        doc.add_step(StepDraft::empty(title), position, NOW).unwrap()
    }

    /// `title(parent)` pairs in list order, such as `["A", "B(A)", "C"]`, to compare shapes quickly.
    pub(crate) fn shape(doc: &Document) -> Vec<String> {
        doc.guide
            .steps
            .iter()
            .map(|s| match &s.parent {
                Some(p) => format!("{}({})", s.title, doc.guide.step(p).map(|x| x.title.clone()).unwrap_or("?".into())),
                None => s.title.clone(),
            })
            .collect()
    }

    pub(crate) fn assert_sound(doc: &Document) {
        let problems = doc.guide.problems();
        assert!(problems.is_empty(), "guide has problems: {problems:?}");
    }

    /// A, B(A), C(A), D, E(D) as ids step-1 .. step-5.
    fn family() -> Document {
        let mut d = doc();
        let a = add(&mut d, "A", Position::End);
        add(&mut d, "B", Position::ChildOf(&a));
        add(&mut d, "C", Position::ChildOf(&a));
        let dd = add(&mut d, "D", Position::End);
        add(&mut d, "E", Position::ChildOf(&dd));
        d
    }

    #[test]
    fn adding_steps_gives_new_ids_in_order() {
        let mut d = doc();
        let a = add(&mut d, "A", Position::End);
        let b = add(&mut d, "B", Position::End);
        assert_eq!((a.as_str(), b.as_str()), ("step-1", "step-2"));
        assert_eq!(d.guide.next_step_number, 3);
        assert_eq!(d.guide.updated_at, NOW);
        assert_sound(&d);
    }

    #[test]
    fn adding_after_a_step_goes_after_its_substeps() {
        let mut d = family();
        add(&mut d, "X", Position::After("step-1"));
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "X", "D", "E(D)"]);
        add(&mut d, "Y", Position::After("step-2"));
        assert_eq!(shape(&d), ["A", "B(A)", "Y(A)", "C(A)", "X", "D", "E(D)"]);
        assert_sound(&d);
    }

    #[test]
    fn adding_a_substep_goes_last() {
        let mut d = family();
        add(&mut d, "F", Position::ChildOf("step-1"));
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "F(A)", "D", "E(D)"]);
        assert_sound(&d);
    }

    #[test]
    fn adding_next_to_a_missing_step_is_refused() {
        let mut d = doc();
        assert_eq!(
            d.add_step(StepDraft::empty("x"), Position::After("step-9"), NOW),
            Err(OpError::NoSuchStep("step-9".into()))
        );
        assert_eq!(d.guide.next_step_number, 1, "a refused step must not use a number");
    }

    #[test]
    fn a_picture_step_needs_a_picture_and_others_cannot_have_one() {
        let mut d = doc();
        let mut bad = StepDraft::empty("x");
        bad.kind = StepKind::Image;
        assert_eq!(d.add_step(bad, Position::End, NOW), Err(OpError::PictureMismatch));
        let mut bad = StepDraft::with_picture("x", StepImage::new("images/step-0001.png", 10, 10));
        bad.kind = StepKind::Empty;
        assert_eq!(d.add_step(bad, Position::End, NOW), Err(OpError::PictureMismatch));
        let ok = StepDraft::with_picture("x", StepImage::new("images/step-0001.png", 10, 10));
        assert!(d.add_step(ok, Position::End, NOW).is_ok());
    }

    #[test]
    fn editing_words_only_touches_the_time_when_something_changed() {
        let mut d = doc();
        let a = add(&mut d, "A", Position::End);
        d.set_title(&a, "A", LATER).unwrap();
        assert_eq!(d.guide.updated_at, NOW);
        d.set_title(&a, "A2", LATER).unwrap();
        assert_eq!(d.guide.updated_at, LATER);
        assert_eq!(d.guide.step(&a).unwrap().title, "A2");
        d.set_description(&a, "words", LATER).unwrap();
        d.set_image_description(&a, "alt", LATER).unwrap();
        d.set_skipped(&a, true, LATER).unwrap();
        let s = d.guide.step(&a).unwrap();
        assert_eq!((s.description.as_str(), s.image_description.as_str(), s.skipped), ("words", "alt", true));
        assert_eq!(d.set_title("step-99", "x", LATER), Err(OpError::NoSuchStep("step-99".into())));
    }

    #[test]
    fn skipping_and_unskipping() {
        let mut d = family();
        d.set_skipped("step-2", true, LATER).unwrap();
        assert!(d.guide.step("step-2").unwrap().skipped);
        d.set_skipped("step-2", false, LATER).unwrap();
        assert!(!d.guide.step("step-2").unwrap().skipped);
    }

    #[test]
    fn moving_a_step_moves_its_substeps_with_it() {
        let mut d = family();
        d.move_step_by("step-1", 1, LATER).unwrap();
        assert_eq!(shape(&d), ["D", "E(D)", "A", "B(A)", "C(A)"]);
        d.move_step_by("step-1", -1, LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "D", "E(D)"]);
        assert_sound(&d);
    }

    #[test]
    fn moving_stops_at_the_ends_and_changes_nothing_there() {
        let mut d = family();
        let before = d.clone();
        d.move_step_by("step-1", -5, LATER).unwrap();
        assert_eq!(d, before, "already first: nothing changes, not even the time");
        d.move_step_by("step-4", 5, LATER).unwrap();
        assert_eq!(d, before, "already last");
        d.move_step_by("step-3", 1, LATER).unwrap();
        assert_eq!(d, before, "C is the last substep of A");
    }

    #[test]
    fn moving_among_substeps_only_reorders_that_family() {
        let mut d = family();
        d.move_step_by("step-3", -1, LATER).unwrap();
        assert_eq!(shape(&d), ["A", "C(A)", "B(A)", "D", "E(D)"]);
        d.move_step_to("step-2", 0, LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "D", "E(D)"]);
        d.move_step_to("step-1", 99, LATER).unwrap();
        assert_eq!(shape(&d), ["D", "E(D)", "A", "B(A)", "C(A)"]);
        assert_sound(&d);
    }

    #[test]
    fn indent_and_outdent() {
        let mut d = family();
        d.indent("step-4", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "D(A)", "E(D)"]);
        assert_eq!(d.indent("step-2", LATER), Err(OpError::NothingToNestUnder));
        d.outdent("step-4", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "D", "E(D)"]);
        d.outdent("step-3", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C", "D", "E(D)"]);
        assert_eq!(d.outdent("step-1", LATER), Err(OpError::AlreadyTopLevel));
        assert_sound(&d);
    }

    #[test]
    fn nesting_a_step_inside_itself_or_its_substeps_is_refused() {
        let mut d = family();
        let before = d.clone();
        assert_eq!(d.change_parent("step-1", Some("step-1"), LATER), Err(OpError::WouldNestInsideItself));
        assert_eq!(d.change_parent("step-1", Some("step-3"), LATER), Err(OpError::WouldNestInsideItself));
        assert_eq!(d, before);
        d.change_parent("step-1", Some("step-5"), LATER).unwrap();
        assert_eq!(shape(&d), ["D", "E(D)", "A(E)", "B(A)", "C(A)"]);
        d.change_parent("step-1", None, LATER).unwrap();
        assert_eq!(shape(&d), ["D", "E(D)", "A", "B(A)", "C(A)"]);
        assert_sound(&d);
    }

    #[test]
    fn deleting_a_step_moves_its_substeps_up_one_level() {
        let mut d = family();
        d.delete_step("step-1", LATER).unwrap();
        assert_eq!(shape(&d), ["B", "C", "D", "E(D)"]);
        assert_eq!(d.trash.entries.len(), 1);
        assert_eq!(d.trash.entries[0].step.id, "step-1");
        assert_eq!(d.trash.entries[0].was_index, 0);
        assert_sound(&d);

        let mut d = family();
        d.indent("step-4", LATER).unwrap();
        d.delete_step("step-4", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "E(A)"], "E moves up into D's place under A");
        assert_sound(&d);
    }

    #[test]
    fn restoring_puts_a_step_back_where_it_was() {
        let mut d = family();
        d.delete_step("step-3", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "D", "E(D)"]);
        d.restore_step("step-3", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "D", "E(D)"]);
        assert!(d.trash.entries.is_empty());
        assert_eq!(d.restore_step("step-3", LATER), Err(OpError::NotInTrash("step-3".into())));
    }

    #[test]
    fn restoring_a_step_whose_parent_is_also_gone_puts_it_at_the_top() {
        let mut d = family();
        d.delete_step("step-2", LATER).unwrap();
        d.delete_step("step-1", LATER).unwrap();
        d.restore_step("step-2", LATER).unwrap();
        assert_eq!(shape(&d), ["B", "C", "D", "E(D)"], "B is back at the top; C was promoted earlier");
        d.restore_step("step-1", LATER).unwrap();
        assert_sound(&d);
        assert_eq!(d.guide.steps.len(), 5);
    }

    #[test]
    fn ids_are_never_reused_after_delete() {
        let mut d = doc();
        let a = add(&mut d, "A", Position::End);
        d.delete_step(&a, LATER).unwrap();
        let b = add(&mut d, "B", Position::End);
        assert_eq!((a.as_str(), b.as_str()), ("step-1", "step-2"));
        assert_eq!(d.highest_step_number(), 2);
        d.empty_trash(LATER);
        let c = add(&mut d, "C", Position::End);
        assert_eq!(c, "step-3", "even with the trash emptied");
    }

    #[test]
    fn duplicating_copies_marks_substeps_and_gets_new_ids() {
        let mut d = family();
        let mut mark = Annotation::at("a1", crate::model::AnnotationKind::Rect, 1, 2);
        mark.width = Some(10);
        mark.height = Some(5);
        d.add_annotation("step-1", mark.clone(), NOW).unwrap();
        let copy = d.duplicate_step("step-1", LATER).unwrap();
        assert_eq!(copy, "step-6");
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "A", "B(A)", "C(A)", "D", "E(D)"]);
        let ids: Vec<&str> = d.guide.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["step-1", "step-2", "step-3", "step-6", "step-7", "step-8", "step-4", "step-5"]);
        assert_eq!(d.guide.step("step-6").unwrap().annotations, vec![mark]);
        assert_eq!(d.guide.step("step-7").unwrap().parent.as_deref(), Some("step-6"));
        assert_sound(&d);
    }

    #[test]
    fn duplicating_a_substep_stays_under_the_same_parent() {
        let mut d = family();
        d.duplicate_step("step-2", LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "B(A)", "C(A)", "D", "E(D)"]);
        assert_sound(&d);
    }

    /// A, B, C, D, E at the top level, as step-1 to step-5.
    fn five() -> Document {
        let mut d = doc();
        for title in ["A", "B", "C", "D", "E"] {
            add(&mut d, title, Position::End);
        }
        d
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn several_steps_that_sit_together_move_past_the_step_beside_them_as_a_block() {
        let mut d = five();
        assert_eq!(d.move_steps_by(&ids(&["step-3", "step-4"]), -1, LATER), Ok(true));
        assert_eq!(shape(&d), ["A", "C", "D", "B", "E"]);
        assert_eq!(
            d.move_steps_by(&ids(&["step-4", "step-3"]), -1, LATER),
            Ok(true),
            "the order they are given in does not matter"
        );
        assert_eq!(shape(&d), ["C", "D", "A", "B", "E"]);
        let before = d.clone();
        assert_eq!(d.move_steps_by(&ids(&["step-3", "step-4"]), -1, LATER), Ok(false), "the block is first already");
        assert_eq!(d, before, "nothing changes, not even the time");
        assert_eq!(d.move_steps_by(&ids(&["step-3", "step-4"]), 1, NOW), Ok(true));
        assert_eq!(shape(&d), ["A", "C", "D", "B", "E"]);
        assert_sound(&d);
    }

    #[test]
    fn steps_that_are_apart_each_move_a_place_and_one_already_at_the_end_stays() {
        let mut d = five();
        assert_eq!(d.move_steps_by(&ids(&["step-1", "step-3"]), -1, LATER), Ok(true));
        assert_eq!(shape(&d), ["A", "C", "B", "D", "E"], "A is first and stays, C moves up");
        let mut d = five();
        assert_eq!(d.move_steps_by(&ids(&["step-2", "step-5"]), 1, LATER), Ok(true));
        assert_eq!(shape(&d), ["A", "C", "B", "D", "E"], "E is last and stays, B moves down");
        let mut d = five();
        let before = d.clone();
        assert_eq!(d.move_steps_by(&ids(&["step-4", "step-5"]), 1, LATER), Ok(false));
        assert_eq!(d, before);
    }

    #[test]
    fn moving_by_two_places_and_by_none() {
        let mut d = five();
        assert_eq!(d.move_steps_by(&ids(&["step-5"]), -2, LATER), Ok(true));
        assert_eq!(shape(&d), ["A", "B", "E", "C", "D"]);
        let before = d.clone();
        assert_eq!(d.move_steps_by(&ids(&["step-5"]), 0, NOW), Ok(false));
        assert_eq!(d, before);
    }

    #[test]
    fn a_step_chosen_with_its_own_parent_is_carried_by_it_and_does_not_move_on_its_own() {
        let mut d = family();
        assert_eq!(d.move_steps_by(&ids(&["step-1", "step-2"]), 1, LATER), Ok(true));
        assert_eq!(shape(&d), ["D", "E(D)", "A", "B(A)", "C(A)"], "A moved with both its substeps, in their order");
        assert_sound(&d);
    }

    #[test]
    fn steps_in_different_families_move_only_within_their_own_family() {
        let mut d = family();
        let before = d.clone();
        assert_eq!(d.move_steps_by(&ids(&["step-2", "step-5"]), -1, LATER), Ok(false), "each is first in its family");
        assert_eq!(d, before);
        assert_eq!(d.move_steps_by(&ids(&["step-3", "step-5"]), -1, LATER), Ok(true));
        assert_eq!(shape(&d), ["A", "C(A)", "B(A)", "D", "E(D)"]);
        assert_sound(&d);
    }

    #[test]
    fn moving_nothing_or_a_missing_step_changes_nothing() {
        let mut d = five();
        let before = d.clone();
        assert_eq!(d.move_steps_by(&[], -1, LATER), Ok(false));
        assert_eq!(
            d.move_steps_by(&ids(&["step-2", "step-99"]), -1, LATER),
            Err(OpError::NoSuchStep("step-99".into()))
        );
        assert_eq!(d, before);
    }

    #[test]
    fn a_run_of_steps_is_copied_as_a_block_straight_after_the_run() {
        let mut d = five();
        let copies = d.duplicate_steps(&ids(&["step-3", "step-2"]), LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B", "C", "B", "C", "D", "E"]);
        assert_eq!(copies, ["step-6", "step-7"], "in the order of the list, whatever order the steps were given in");
        let order: Vec<&str> = d.guide.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, ["step-1", "step-2", "step-3", "step-6", "step-7", "step-4", "step-5"]);
        assert_eq!(d.guide.next_step_number, 8, "the ids that were given out are never given out again");
        assert_eq!(d.guide.updated_at, LATER);
        assert_sound(&d);
    }

    #[test]
    fn steps_that_are_apart_are_each_copied_straight_after_themselves() {
        let mut d = five();
        let copies = d.duplicate_steps(&ids(&["step-1", "step-3"]), LATER).unwrap();
        assert_eq!(shape(&d), ["A", "A", "B", "C", "C", "D", "E"]);
        assert_eq!(copies, ["step-6", "step-7"]);
        let order: Vec<&str> = d.guide.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, ["step-1", "step-6", "step-2", "step-3", "step-7", "step-4", "step-5"]);
        assert_sound(&d);
    }

    #[test]
    fn a_step_chosen_with_its_own_parent_is_copied_once_with_the_whole_family() {
        let mut d = family();
        let copies = d.duplicate_steps(&ids(&["step-1", "step-2"]), LATER).unwrap();
        assert_eq!(copies.len(), 1, "B is inside A, so the copy of A carries its own B");
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "A", "B(A)", "C(A)", "D", "E(D)"]);
        assert_sound(&d);
    }

    #[test]
    fn substeps_of_one_parent_are_copied_as_a_block_under_the_same_parent() {
        let mut d = family();
        let copies = d.duplicate_steps(&ids(&["step-2", "step-3"]), LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "C(A)", "B(A)", "C(A)", "D", "E(D)"]);
        assert!(copies.iter().all(|c| d.guide.step(c).unwrap().parent.as_deref() == Some("step-1")));
        assert_sound(&d);
    }

    #[test]
    fn steps_from_two_families_are_each_copied_after_themselves_in_their_own_family() {
        let mut d = family();
        d.duplicate_steps(&ids(&["step-2", "step-5"]), LATER).unwrap();
        assert_eq!(shape(&d), ["A", "B(A)", "B(A)", "C(A)", "D", "E(D)", "E(D)"]);
        assert_sound(&d);
    }

    #[test]
    fn steps_in_different_families_at_neighbouring_places_are_not_taken_for_a_run() {
        // A has substeps A1, A2; B has B1, B2, B3. A2 is second in its family and B3 third in its own: places 1 and 2.
        let mut d = doc();
        let a = add(&mut d, "A", Position::End);
        let b = add(&mut d, "B", Position::End);
        add(&mut d, "A1", Position::ChildOf(&a));
        add(&mut d, "A2", Position::ChildOf(&a));
        for title in ["B1", "B2", "B3"] {
            add(&mut d, title, Position::ChildOf(&b));
        }
        let copies = d.duplicate_steps(&ids(&["step-4", "step-7"]), LATER).unwrap();
        assert_eq!(
            shape(&d),
            ["A", "A1(A)", "A2(A)", "A2(A)", "B", "B1(B)", "B2(B)", "B3(B)", "B3(B)"],
            "each copy went straight after its own step, in its own family"
        );
        assert_eq!(d.guide.step(&copies[0]).unwrap().parent.as_deref(), Some("step-1"));
        assert_eq!(d.guide.step(&copies[1]).unwrap().parent.as_deref(), Some("step-2"));
        assert_sound(&d);
    }

    #[test]
    fn copying_one_step_this_way_is_the_same_as_duplicating_it() {
        let mut one = family();
        let mut several = family();
        let id = one.duplicate_step("step-1", LATER).unwrap();
        let ids = several.duplicate_steps(&["step-1".to_string()], LATER).unwrap();
        assert_eq!(ids, [id]);
        assert_eq!(one, several);
    }

    #[test]
    fn copying_nothing_or_a_missing_step_changes_nothing_and_uses_no_numbers() {
        let mut d = five();
        let before = d.clone();
        assert_eq!(d.duplicate_steps(&[], LATER), Ok(Vec::new()));
        assert_eq!(d.duplicate_steps(&ids(&["step-2", "step-99"]), LATER), Err(OpError::NoSuchStep("step-99".into())));
        assert_eq!(d, before);
    }

    #[test]
    fn marks_can_be_added_replaced_and_removed() {
        let mut d = family();
        let mut a = Annotation::at("", crate::model::AnnotationKind::Oval, 0, 0);
        a.width = Some(4);
        a.height = Some(4);
        let first = d.add_annotation("step-1", a.clone(), NOW).unwrap();
        let second = d.add_annotation("step-1", a.clone(), NOW).unwrap();
        assert_eq!((first.as_str(), second.as_str()), ("a1", "a2"));
        let mut moved = d.guide.step("step-1").unwrap().annotations[0].clone();
        moved.x = 50;
        d.replace_annotation("step-1", moved, LATER).unwrap();
        assert_eq!(d.guide.step("step-1").unwrap().annotations[0].x, 50);
        d.remove_annotation("step-1", "a1", LATER).unwrap();
        let third = d.add_annotation("step-1", a, NOW).unwrap();
        assert_eq!(third, "a3", "a new mark takes one above the highest in use");
        assert_eq!(d.remove_annotation("step-1", "a9", LATER), Err(OpError::NoSuchMark("a9".into())));
        let mut dup = Annotation::at("a2", crate::model::AnnotationKind::Oval, 0, 0);
        dup.width = Some(1);
        dup.height = Some(1);
        assert_eq!(d.add_annotation("step-1", dup, LATER), Err(OpError::MarkExists("a2".into())));
    }

    #[test]
    fn emptying_the_trash_hands_back_its_steps() {
        let mut d = family();
        d.delete_step("step-2", LATER).unwrap();
        d.delete_step("step-4", LATER).unwrap();
        let gone = d.empty_trash(LATER);
        assert_eq!(gone.len(), 2);
        assert!(d.trash.entries.is_empty());
        assert!(d.empty_trash(LATER).is_empty());
    }

    #[test]
    fn picture_lists() {
        let mut d = doc();
        let shared = StepImage::new("images/step-0001.png", 10, 10);
        let a = d.add_step(StepDraft::with_picture("A", shared.clone()), Position::End, NOW).unwrap();
        d.add_step(StepDraft::with_picture("B", StepImage::new("images/step-0002.png", 10, 10)), Position::End, NOW)
            .unwrap();
        let copy = d.duplicate_step(&a, NOW).unwrap();
        assert_eq!(d.live_pictures(), ["images/step-0001.png", "images/step-0002.png"]);
        d.delete_step("step-2", NOW).unwrap();
        assert_eq!(d.live_pictures(), ["images/step-0001.png"]);
        assert_eq!(d.trash_only_pictures(), ["images/step-0002.png"]);
        d.delete_step(&a, NOW).unwrap();
        assert_eq!(d.trash_only_pictures(), ["images/step-0002.png"], "the copy still uses the first picture");
        d.delete_step(&copy, NOW).unwrap();
        assert_eq!(d.trash_only_pictures().len(), 2);
    }

    #[test]
    fn a_damaged_list_is_repaired_without_losing_steps() {
        let mut d = doc();
        for t in ["A", "B", "C"] {
            add(&mut d, t, Position::End);
        }
        // A and B name each other as parent (a loop), and C names a parent that does not exist.
        d.guide.steps[0].parent = Some("step-2".into());
        d.guide.steps[1].parent = Some("step-1".into());
        d.guide.steps[2].parent = Some("step-77".into());
        assert!(!d.guide.problems().is_empty());
        add(&mut d, "D", Position::End);
        assert_eq!(d.guide.steps.len(), 4, "no step may be lost");
        assert_eq!(shape(&d), ["C", "D", "A", "B"], "the loop is broken and everything is top level");
        assert_sound(&d);
    }
}
