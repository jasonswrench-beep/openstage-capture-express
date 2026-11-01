// SPDX-License-Identifier: Apache-2.0
//! Undo and redo (spec section 9): a stack of whole-document snapshots. A guide's JSON is small and its pictures are
//! never modified, so copying the document before each change is cheap and cannot get out of step.
//!
//! The history has no fixed limit within a session. Emptying the trash is the one thing that cannot be undone, so it
//! clears the history instead of leaving undo steps that point at pictures that are gone.

use crate::ops::{Document, OpError, TrashEntry};

/// A document being edited, with its undo and redo history.
#[derive(Debug, Clone)]
pub struct Session {
    doc: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
    /// The highest `next_step_number` ever seen. Undo and redo never take the counter below it, so a step id is not
    /// handed out twice even if the step that first used it was undone.
    high_water: u32,
}

impl Session {
    pub fn new(doc: Document) -> Session {
        let high_water = doc.guide.next_step_number.max(doc.highest_step_number() + 1);
        let mut session = Session { doc, undo: Vec::new(), redo: Vec::new(), high_water };
        session.doc.guide.next_step_number = session.doc.guide.next_step_number.max(high_water);
        session
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Do something to the document as one undoable change. If it fails nothing changes and nothing is recorded. If
    /// it succeeds but changes nothing, nothing is recorded either.
    pub fn apply<R>(&mut self, change: impl FnOnce(&mut Document) -> Result<R, OpError>) -> Result<R, OpError> {
        let before = self.doc.clone();
        match change(&mut self.doc) {
            Ok(value) => {
                self.note_counter();
                if self.doc != before {
                    self.undo.push(before);
                    self.redo.clear();
                }
                Ok(value)
            }
            Err(error) => {
                let used = self.doc.guide.next_step_number;
                self.doc = before;
                self.doc.guide.next_step_number = self.doc.guide.next_step_number.max(used);
                self.note_counter();
                Err(error)
            }
        }
    }

    /// Several changes as one undo step: if any fails, none of them is kept.
    pub fn apply_group(&mut self, changes: impl FnOnce(&mut Document) -> Result<(), OpError>) -> Result<(), OpError> {
        self.apply(changes)
    }

    /// Go back one change. Returns false if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.doc, previous);
        self.redo.push(current);
        self.note_counter();
        true
    }

    /// Go forward one change that was undone. Returns false if there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.doc, next);
        self.undo.push(current);
        self.note_counter();
        true
    }

    /// Empty the trash. This cannot be undone, so the undo and redo history is cleared. Returns what was in the trash.
    pub fn empty_trash(&mut self, now: &str) -> Vec<TrashEntry> {
        let entries = self.doc.empty_trash(now);
        if !entries.is_empty() {
            self.undo.clear();
            self.redo.clear();
        }
        entries
    }

    /// Keep the step counter at or above the highest value seen.
    fn note_counter(&mut self) {
        self.high_water = self.high_water.max(self.doc.guide.next_step_number);
        self.doc.guide.next_step_number = self.high_water;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Guide;
    use crate::ops::tests::{doc, shape, LATER, NOW};
    use crate::ops::{Position, StepDraft};

    fn session() -> Session {
        Session::new(doc())
    }

    fn add(session: &mut Session, title: &str) -> String {
        session.apply(|d| d.add_step(StepDraft::empty(title), Position::End, NOW)).unwrap()
    }

    #[test]
    fn undo_and_redo_walk_back_and_forward() {
        let mut s = session();
        add(&mut s, "A");
        add(&mut s, "B");
        assert_eq!(shape(s.doc()), ["A", "B"]);
        assert!(s.undo());
        assert_eq!(shape(s.doc()), ["A"]);
        assert!(s.undo());
        assert_eq!(shape(s.doc()), Vec::<String>::new());
        assert!(!s.undo(), "nothing left to undo");
        assert!(s.redo());
        assert!(s.redo());
        assert_eq!(shape(s.doc()), ["A", "B"]);
        assert!(!s.redo());
    }

    #[test]
    fn a_new_change_clears_redo() {
        let mut s = session();
        add(&mut s, "A");
        s.undo();
        assert!(s.can_redo());
        add(&mut s, "B");
        assert!(!s.can_redo());
    }

    #[test]
    fn a_failed_change_leaves_no_trace() {
        let mut s = session();
        add(&mut s, "A");
        let depth = s.undo_depth();
        let snapshot = s.doc().clone();
        let result = s.apply(|d| {
            d.add_step(StepDraft::empty("half done"), Position::End, LATER)?;
            d.delete_step("step-99", LATER)
        });
        assert!(result.is_err());
        assert_eq!(s.undo_depth(), depth);
        assert_eq!(shape(s.doc()), shape(&snapshot));
        assert_eq!(s.doc().guide.steps, snapshot.guide.steps);
        assert!(s.doc().guide.next_step_number > snapshot.guide.next_step_number, "the burned number is not reused");
    }

    #[test]
    fn a_change_that_does_nothing_is_not_recorded() {
        let mut s = session();
        let id = add(&mut s, "A");
        let depth = s.undo_depth();
        s.apply(|d| d.set_title(&id, "A", LATER)).unwrap();
        assert_eq!(s.undo_depth(), depth);
    }

    #[test]
    fn ids_are_not_reused_after_undo() {
        let mut s = session();
        assert_eq!(add(&mut s, "A"), "step-1");
        s.undo();
        assert_eq!(add(&mut s, "B"), "step-2", "step-1 was undone but its number stays used");
        s.undo();
        s.redo();
        assert_eq!(add(&mut s, "C"), "step-3");
        assert_eq!(shape(s.doc()), ["B", "C"]);
    }

    #[test]
    fn ids_are_not_reused_after_delete_and_undo() {
        let mut s = session();
        let a = add(&mut s, "A");
        s.apply(|d| d.delete_step(&a, LATER)).unwrap();
        s.undo();
        assert_eq!(shape(s.doc()), ["A"]);
        assert_eq!(add(&mut s, "B"), "step-2");
    }

    #[test]
    fn a_group_is_one_undo_step() {
        let mut s = session();
        add(&mut s, "A");
        add(&mut s, "B");
        s.apply_group(|d| {
            d.set_skipped("step-1", true, LATER)?;
            d.set_skipped("step-2", true, LATER)
        })
        .unwrap();
        assert!(s.doc().guide.steps.iter().all(|x| x.skipped));
        s.undo();
        assert!(s.doc().guide.steps.iter().all(|x| !x.skipped));
    }

    #[test]
    fn emptying_the_trash_clears_the_history() {
        let mut s = session();
        let a = add(&mut s, "A");
        s.apply(|d| d.delete_step(&a, LATER)).unwrap();
        let gone = s.empty_trash(LATER);
        assert_eq!(gone.len(), 1);
        assert!(!s.can_undo() && !s.can_redo());
        assert_eq!(add(&mut s, "B"), "step-2");
    }

    #[test]
    fn a_session_opened_on_a_guide_with_a_low_counter_repairs_it() {
        let mut g = Guide::new("g", "T", "en", NOW);
        g.steps.push(crate::model::Step::bare("step-7", crate::model::StepKind::Empty, "x"));
        g.next_step_number = 3;
        let mut s = Session::new(Document::new(g));
        assert_eq!(add(&mut s, "new"), "step-8");
    }

    /// A small deterministic random number generator, so the random walk is the same on every computer.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// The document as the person sees it. The id counter is left out because undo deliberately never lowers it.
    fn visible(doc: &Document) -> Document {
        let mut copy = doc.clone();
        copy.guide.next_step_number = 0;
        copy
    }

    fn random_change(s: &mut Session, rng: &mut Rng, step: usize) {
        let now = format!("2026-10-31T16:{:02}:{:02}Z", (step / 60) % 60, step % 60);
        let ids: Vec<String> = s.doc().guide.steps.iter().map(|x| x.id.clone()).collect();
        let pick = |rng: &mut Rng| ids[rng.below(ids.len())].clone();
        let trashed: Vec<String> = s.doc().trash.entries.iter().map(|e| e.step.id.clone()).collect();
        let kind = if ids.is_empty() { 0 } else { rng.below(11) };
        let _ = match kind {
            0 | 1 => {
                let title = format!("step {step}");
                let position = if ids.is_empty() || rng.below(3) == 0 { None } else { Some(pick(rng)) };
                s.apply(|d| {
                    let pos = match &position {
                        Some(id) if rng_flag(step) => Position::ChildOf(id),
                        Some(id) => Position::After(id),
                        None => Position::End,
                    };
                    d.add_step(StepDraft::empty(&title), pos, &now).map(|_| ())
                })
            }
            2 => {
                let id = pick(rng);
                let by = rng.below(5) as i32 - 2;
                s.apply(|d| d.move_step_by(&id, by, &now))
            }
            3 => {
                let id = pick(rng);
                s.apply(|d| d.indent(&id, &now))
            }
            4 => {
                let id = pick(rng);
                s.apply(|d| d.outdent(&id, &now))
            }
            5 => {
                let id = pick(rng);
                let target = if rng.below(2) == 0 { None } else { Some(pick(rng)) };
                s.apply(|d| d.change_parent(&id, target.as_deref(), &now))
            }
            6 => {
                let id = pick(rng);
                s.apply(|d| d.duplicate_step(&id, &now).map(|_| ()))
            }
            7 => {
                let id = pick(rng);
                s.apply(|d| d.delete_step(&id, &now))
            }
            8 if !trashed.is_empty() => {
                let id = trashed[rng.below(trashed.len())].clone();
                s.apply(|d| d.restore_step(&id, &now))
            }
            9 => {
                let id = pick(rng);
                let skipped = rng.below(2) == 0;
                s.apply(|d| d.set_skipped(&id, skipped, &now))
            }
            _ => {
                let id = pick(rng);
                let title = format!("title {step}");
                s.apply(|d| d.set_title(&id, &title, &now))
            }
        };
    }

    fn rng_flag(step: usize) -> bool {
        step.is_multiple_of(2)
    }

    #[test]
    fn a_long_random_walk_of_changes_and_undos_returns_to_every_earlier_state() {
        for seed in [1u64, 7, 12345, 987654321] {
            let mut rng = Rng(seed);
            let mut s = session();
            let mut states = vec![visible(s.doc())];
            for step in 0..300 {
                let depth = s.undo_depth();
                random_change(&mut s, &mut rng, step);
                assert!(s.doc().guide.problems().is_empty(), "seed {seed} step {step}: {:?}", s.doc().guide.problems());
                if s.undo_depth() > depth {
                    states.push(visible(s.doc()));
                }
            }
            assert_eq!(s.undo_depth(), states.len() - 1, "every recorded change is one undo step");
            // all the way back, comparing with each earlier state
            for expected in states.iter().rev().skip(1) {
                assert!(s.undo());
                assert_eq!(&visible(s.doc()), expected, "seed {seed}: undo did not return to the earlier state");
            }
            assert!(!s.undo());
            // and all the way forward again
            for expected in states.iter().skip(1) {
                assert!(s.redo());
                assert_eq!(&visible(s.doc()), expected, "seed {seed}: redo did not return to the later state");
            }
            assert!(!s.redo());
            assert!(s.doc().guide.problems().is_empty());
        }
    }

    #[test]
    fn the_random_walk_really_exercises_the_operations() {
        let mut rng = Rng(42);
        let mut s = session();
        for step in 0..300 {
            random_change(&mut s, &mut rng, step);
        }
        let d = s.doc();
        assert!(d.guide.steps.len() > 10, "only {} steps", d.guide.steps.len());
        assert!(d.guide.steps.iter().any(|x| x.parent.is_some()), "no nesting happened");
        assert!(!d.trash.entries.is_empty() || d.guide.next_step_number > 30, "nothing was deleted");
        assert!(s.undo_depth() > 100, "only {} changes recorded", s.undo_depth());
    }
}
