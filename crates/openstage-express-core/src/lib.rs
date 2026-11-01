// SPDX-License-Identifier: Apache-2.0
//! The heart of OpenStage Capture Express, with no Windows code in it.
//!
//! Built so far (milestones M1 and M2): the guide model ([`model`]), the things a person can do to a guide ([`ops`]),
//! undo and redo ([`session`]), bringing older files up to date ([`migrate`]), saving to a guide folder ([`store`]),
//! smart step titles ([`titles`]) and the step builder that turns clicks and typing into step drafts ([`builder`]).
//! Still to come: annotation drawing and the focused view (M3).
//! The format is written down in `docs/guide-format.md` and the two must always agree.

pub mod builder;
pub mod error;
pub mod fsutil;
pub mod migrate;
pub mod model;
pub mod ops;
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
