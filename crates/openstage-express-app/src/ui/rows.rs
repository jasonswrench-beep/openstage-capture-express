// SPDX-License-Identifier: Apache-2.0
//! What the steps list shows: one row for each step, numbered the way the exports number them, and the words a screen reader is
//! given for each. No drawing happens here, so all of it is tested on any computer.

use std::collections::HashMap;

use openstage_express_core::{Guide, StepKind};

/// One line of the steps list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    /// How many steps this one is inside: 0 for a top-level step.
    pub depth: usize,
    /// `1`, `2.1` and so on, as the exports number it. `None` for a note and for a step that is skipped (a skipped step and
    /// everything inside it is left out of every export).
    pub number: Option<String>,
    pub title: String,
    pub kind: StepKind,
    /// This step, or a step it is inside, is skipped.
    pub skipped: bool,
}

impl Row {
    /// The row's words, which are also its name for a screen reader.
    pub fn label(&self) -> String {
        let title = if self.title.trim().is_empty() { "(no title)" } else { self.title.trim() };
        match (&self.number, self.kind, self.skipped) {
            (_, _, true) => format!("{title} (skipped)"),
            (_, StepKind::Block, _) => format!("Note: {title}"),
            (Some(number), _, _) => format!("{number}. {title}"),
            (None, _, _) => title.to_string(),
        }
    }
}

/// The rows for a guide, in reading order.
pub fn rows(guide: &Guide) -> Vec<Row> {
    let parent_of: HashMap<&str, Option<&str>> =
        guide.steps.iter().map(|s| (s.id.as_str(), s.parent.as_deref())).collect();
    // A step is inside the steps above it; one whose parent cannot be found is treated as top level.
    let depth_of = |id: &str| {
        let (mut depth, mut at) = (0, id);
        while let Some(Some(parent)) = parent_of.get(at) {
            if !parent_of.contains_key(parent) || depth > guide.steps.len() {
                break;
            }
            depth += 1;
            at = parent;
        }
        depth
    };
    let mut skipped_ids: Vec<&str> = Vec::new();
    let mut numbers: HashMap<String, String> = HashMap::new();
    let mut counters: HashMap<Option<String>, u32> = HashMap::new();
    let mut out = Vec::with_capacity(guide.steps.len());
    for step in &guide.steps {
        let parent = step.parent.as_deref().filter(|p| parent_of.contains_key(p));
        let skipped = step.skipped || parent.is_some_and(|p| skipped_ids.contains(&p));
        if skipped {
            skipped_ids.push(&step.id);
        }
        let number = if skipped || step.kind == StepKind::Block {
            None
        } else {
            let counter = counters.entry(parent.map(str::to_string)).or_insert(0);
            *counter += 1;
            let own = match parent.and_then(|p| numbers.get(p)) {
                Some(above) => format!("{above}.{counter}"),
                None => counter.to_string(),
            };
            numbers.insert(step.id.clone(), own.clone());
            Some(own)
        };
        out.push(Row {
            id: step.id.clone(),
            depth: depth_of(&step.id),
            number,
            title: step.title.clone(),
            kind: step.kind,
            skipped,
        });
    }
    out
}

/// How many steps a guide has in its exports (notes and skipped steps are not counted).
pub fn step_count(guide: &Guide) -> usize {
    rows(guide).iter().filter(|r| r.number.is_some()).count()
}

/// "1 step", "5 steps".
pub fn steps_words(count: usize) -> String {
    if count == 1 {
        "1 step".to_string()
    } else {
        format!("{count} steps")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_core::Step;
    use openstage_express_export::demo::demo;

    fn guide(steps: Vec<Step>) -> Guide {
        let mut g = Guide::new("g", "Test", "en", "2026-11-01T00:00:00Z");
        g.steps = steps;
        g
    }

    fn step(id: &str, title: &str, parent: Option<&str>) -> Step {
        let mut s = Step::bare(id, StepKind::Image, title);
        s.parent = parent.map(str::to_string);
        s
    }

    #[test]
    fn the_demo_guide_is_numbered_as_the_exports_number_it() {
        let d = demo();
        let labels: Vec<String> = rows(&d.document.guide).iter().map(Row::label).collect();
        assert_eq!(
            labels,
            [
                "1. Click \"Assignments\"",
                "2. Click \"New assignment\"",
                "2.1. Type \"Week 3 reflection\"",
                "3. Select \"Course\"",
                "Click \"Debug\" (skipped)",
                "Note: Tip",
                "4. Click \"Submit\""
            ]
        );
        let depths: Vec<usize> = rows(&d.document.guide).iter().map(|r| r.depth).collect();
        assert_eq!(depths, [0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(step_count(&d.document.guide), 5);
    }

    #[test]
    fn a_skipped_step_and_everything_inside_it_is_not_numbered_and_the_rest_close_up() {
        let mut skipped = step("b", "Second", None);
        skipped.skipped = true;
        let g =
            guide(vec![step("a", "First", None), skipped, step("b1", "Inside", Some("b")), step("c", "Third", None)]);
        let labels: Vec<String> = rows(&g).iter().map(Row::label).collect();
        assert_eq!(labels, ["1. First", "Second (skipped)", "Inside (skipped)", "2. Third"]);
        assert_eq!(step_count(&g), 2);
        assert!(rows(&g)[2].skipped && rows(&g)[2].number.is_none());
    }

    #[test]
    fn substeps_are_numbered_inside_their_parent_and_deeper_ones_keep_going() {
        let g = guide(vec![
            step("a", "A", None),
            step("a1", "A one", Some("a")),
            step("a11", "A one one", Some("a1")),
            step("a2", "A two", Some("a")),
            step("b", "B", None),
        ]);
        let labels: Vec<String> = rows(&g).iter().map(Row::label).collect();
        assert_eq!(labels, ["1. A", "1.1. A one", "1.1.1. A one one", "1.2. A two", "2. B"]);
        assert_eq!(rows(&g).iter().map(|r| r.depth).collect::<Vec<_>>(), [0, 1, 2, 1, 0]);
    }

    #[test]
    fn a_step_whose_parent_is_missing_is_shown_at_the_top_and_a_loop_does_not_hang() {
        let lost = guide(vec![step("a", "A", Some("nowhere")), step("b", "B", None)]);
        assert_eq!(rows(&lost).iter().map(Row::label).collect::<Vec<_>>(), ["1. A", "2. B"]);
        assert_eq!(rows(&lost)[0].depth, 0);
        let looped = guide(vec![step("a", "A", Some("b")), step("b", "B", Some("a"))]);
        let r = rows(&looped);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|row| row.depth <= 4), "a loop gives some depth, not an endless walk");
    }

    #[test]
    fn a_step_with_no_words_still_has_a_name_a_screen_reader_can_say() {
        let g = guide(vec![step("a", "", None), step("b", "   ", None)]);
        assert_eq!(rows(&g).iter().map(Row::label).collect::<Vec<_>>(), ["1. (no title)", "2. (no title)"]);
        let mut note = step("n", "", None);
        note.kind = StepKind::Block;
        assert_eq!(rows(&guide(vec![note]))[0].label(), "Note: (no title)");
    }

    #[test]
    fn counts_in_words() {
        assert_eq!(steps_words(0), "0 steps");
        assert_eq!(steps_words(1), "1 step");
        assert_eq!(steps_words(12), "12 steps");
        assert_eq!(step_count(&guide(vec![])), 0);
    }
}
