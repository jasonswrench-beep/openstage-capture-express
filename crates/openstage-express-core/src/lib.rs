// SPDX-License-Identifier: Apache-2.0
//! The heart of OpenStage Capture Express, with no Windows code in it.
//!
//! Built so far (milestones M1 to M3): the guide model ([`model`]), the things a person can do to a guide ([`ops`]),
//! undo and redo ([`session`]), bringing older files up to date ([`migrate`]), saving to a guide folder ([`store`]),
//! smart step titles ([`titles`]) and the step builder that turns clicks and typing into step drafts ([`builder`]),
//! the drawing of marks on a picture ([`render`]) and the focused view arithmetic ([`focus`]). Exports are in their own crate
//! (M4), and the capture engine that gives the step builder its pictures ([`capture`]) is the start of M6.
//! The format is written down in `docs/guide-format.md` and the two must always agree.

pub mod builder;
pub mod capture;
pub mod error;
pub mod focus;
pub mod fsutil;
pub mod marks;
pub mod migrate;
pub mod model;
pub mod ops;
pub mod render;
pub mod session;
pub mod store;
pub mod titles;

pub use builder::{CaptureMode, Draft, DraftKind, Host, Input, StepBuilder};
pub use error::StoreError;
pub use model::{
    Annotation, AnnotationKind, Button, Click, Control, Guide, Rect, Step, StepImage, StepKind, SCHEMA_VERSION,
};
pub use ops::{Document, OpError, Position, StepDraft, Trash, TrashEntry};
pub use session::Session;
pub use store::{Access, GuideFolder, Opened, SaveReport};
pub use titles::Language;
