// SPDX-License-Identifier: Apache-2.0
//! Bringing older guide files up to date (spec section 7.2): a file with an older `schemaVersion` is changed, in
//! memory, one step at a time until it matches the version this program writes; a file with a newer one is refused.
//!
//! There is only one format version so far (0.1.0), so [`MIGRATIONS`] is empty. Every migration added later must come
//! with a fixture file of the old format and a test that opens it.

use serde_json::Value;

use crate::error::StoreError;

/// One step from one format version to the next.
pub struct Migration {
    pub from: &'static str,
    pub to: &'static str,
    /// Change the whole file's JSON from the old format to the new one. The version number itself is set afterwards.
    pub apply: fn(&mut Value) -> Result<(), String>,
}

/// Every migration this program knows, oldest first. Empty until the format changes.
pub const MIGRATIONS: &[Migration] = &[];

/// Fields a guide or trash file once had and that this program no longer keeps, taken out of every object in the file wherever
/// it is: `typed`, the text of a `Type` step (the security review of 2026-11-13). It is taken out whatever the file's version, so
/// that a guide made before the change, or a copy someone shared, loses it the moment it is read, and the next save writes it
/// without it. A step's unknown fields are otherwise kept (`extra`), which would have kept this one too.
pub const RETIRED_FIELDS: &[&str] = &["typed"];

/// Take the retired fields out of `value`, everywhere in it. Returns how many were taken out.
pub fn drop_retired_fields(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut dropped = 0;
            for name in RETIRED_FIELDS {
                if map.remove(*name).is_some() {
                    dropped += 1;
                }
            }
            dropped + map.values_mut().map(drop_retired_fields).sum::<usize>()
        }
        Value::Array(items) => items.iter_mut().map(drop_retired_fields).sum(),
        _ => 0,
    }
}

/// Read `1.2.3` as three numbers.
pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Bring `value` (the JSON of a guide or trash file) up to `current`. Returns the versions it started at, if it had to
/// change, so the program can say so and keep a copy of the old file.
pub fn bring_up_to_date(
    value: &mut Value,
    current: &str,
    table: &[Migration],
    what: &str,
) -> Result<Option<String>, StoreError> {
    let damaged = |detail: &str| StoreError::Damaged { what: what.to_string(), detail: detail.to_string() };
    let Some(found) = value.get("schemaVersion").and_then(Value::as_str).map(str::to_string) else {
        return Err(damaged("it has no schemaVersion"));
    };
    let target = parse_version(current).expect("the current version is well formed");
    let first = parse_version(&found).ok_or_else(|| damaged(&format!("the version {found:?} is not a number")))?;
    if first > target {
        return Err(StoreError::TooNew);
    }
    let started = found.clone();
    let mut at = found;
    let mut version = first;
    while version < target {
        let Some(step) = table.iter().find(|m| m.from == at) else {
            return Err(StoreError::NoMigrationPath(at));
        };
        (step.apply)(value)
            .map_err(|detail| damaged(&format!("moving from version {} to {} failed: {detail}", step.from, step.to)))?;
        at = step.to.to_string();
        value["schemaVersion"] = Value::String(at.clone());
        let next = parse_version(&at).ok_or_else(|| damaged("a migration set a bad version"))?;
        if next <= version {
            return Err(damaged("a migration did not move the version forward"));
        }
        version = next;
    }
    if version != target {
        return Err(damaged("the migrations did not end at the current version"));
    }
    Ok(if started == current { None } else { Some(started) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_retired_field_is_taken_out_wherever_it_is() {
        let mut value = json!({"schemaVersion": "0.1.0", "steps": [{"id": "s1", "typed": "a", "title": "T"},
            {"id": "s2", "typed": null, "extra": {"typed": "b", "kept": 1}}], "entries": [{"step": {"typed": "c"}}]});
        assert_eq!(drop_retired_fields(&mut value), 4);
        let text = value.to_string();
        assert!(!text.contains("typed"), "{text}");
        assert_eq!(value["steps"][0]["title"], "T");
        assert_eq!(value["steps"][1]["extra"]["kept"], 1);
        assert_eq!(drop_retired_fields(&mut value), 0, "nothing left to take");
    }

    /// Pretend format 0.0.1 called the title `name`, and 0.0.2 had no `focusedView`.
    fn fake_table() -> Vec<Migration> {
        fn rename(value: &mut Value) -> Result<(), String> {
            let object = value.as_object_mut().ok_or("not an object")?;
            let name = object.remove("name").ok_or("no name field")?;
            object.insert("title".into(), name);
            Ok(())
        }
        fn add_focused_view(value: &mut Value) -> Result<(), String> {
            value["focusedView"] = json!(true);
            Ok(())
        }
        vec![
            Migration { from: "0.0.1", to: "0.0.2", apply: rename },
            Migration { from: "0.0.2", to: "0.1.0", apply: add_focused_view },
        ]
    }

    #[test]
    fn versions_parse() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("12.0.3"), Some((12, 0, 3)));
        for bad in ["", "1", "1.2", "1.2.3.4", "a.b.c", "1.2.x", "-1.0.0"] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
        assert!(parse_version("0.10.0") > parse_version("0.9.9"), "compared as numbers, not text");
    }

    #[test]
    fn the_current_version_is_left_alone() {
        let mut value = json!({"schemaVersion": "0.1.0", "title": "T"});
        let before = value.clone();
        assert_eq!(bring_up_to_date(&mut value, "0.1.0", &[], "guide.json").unwrap(), None);
        assert_eq!(value, before);
    }

    #[test]
    fn a_newer_version_is_refused_with_the_plain_message() {
        for newer in ["0.1.1", "0.2.0", "1.0.0"] {
            let mut value = json!({"schemaVersion": newer});
            let error = bring_up_to_date(&mut value, "0.1.0", &[], "guide.json").unwrap_err();
            assert!(matches!(error, StoreError::TooNew), "{newer}");
            assert!(error.to_string().starts_with("This guide was made by a newer Express."));
        }
    }

    #[test]
    fn an_old_guide_is_walked_up_through_every_step() {
        let mut value = json!({"schemaVersion": "0.0.1", "name": "Old title", "steps": []});
        let started = bring_up_to_date(&mut value, "0.1.0", &fake_table(), "guide.json").unwrap();
        assert_eq!(started.as_deref(), Some("0.0.1"));
        assert_eq!(value, json!({"schemaVersion": "0.1.0", "title": "Old title", "focusedView": true, "steps": []}));
    }

    #[test]
    fn starting_part_way_runs_only_the_later_steps() {
        let mut value = json!({"schemaVersion": "0.0.2", "title": "T"});
        bring_up_to_date(&mut value, "0.1.0", &fake_table(), "guide.json").unwrap();
        assert_eq!(value, json!({"schemaVersion": "0.1.0", "title": "T", "focusedView": true}));
    }

    #[test]
    fn a_version_with_no_path_is_refused_and_says_which() {
        let mut value = json!({"schemaVersion": "0.0.9"});
        let error = bring_up_to_date(&mut value, "0.1.0", &fake_table(), "guide.json").unwrap_err();
        assert!(matches!(&error, StoreError::NoMigrationPath(v) if v == "0.0.9"));
        assert!(error.to_string().contains("0.0.9"));
        let mut real = json!({"schemaVersion": "0.0.1"});
        assert!(
            bring_up_to_date(&mut real, "0.1.0", MIGRATIONS, "guide.json").is_err(),
            "no real migrations exist yet"
        );
    }

    #[test]
    fn a_failing_migration_names_the_step_and_the_file() {
        let mut value = json!({"schemaVersion": "0.0.1"});
        let error = bring_up_to_date(&mut value, "0.1.0", &fake_table(), "guide.json").unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("guide.json") && text.contains("0.0.1 to 0.0.2") && text.contains("no name field"),
            "{text}"
        );
    }

    #[test]
    fn a_missing_or_nonsense_version_is_damage() {
        for value in [json!({}), json!({"schemaVersion": 3}), json!({"schemaVersion": "banana"})] {
            let mut value = value;
            assert!(matches!(
                bring_up_to_date(&mut value, "0.1.0", &[], "guide.json"),
                Err(StoreError::Damaged { .. })
            ));
        }
    }
}
