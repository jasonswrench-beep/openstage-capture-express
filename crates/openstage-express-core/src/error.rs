// SPDX-License-Identifier: Apache-2.0
//! What can go wrong when a guide is opened or saved. The messages are written for the person: what happened, and
//! what is safe (spec section 4.3).

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(
        "This guide was made by a newer Express. Update Express to open it. The guide itself has not been changed."
    )]
    TooNew,
    #[error(
        "This guide uses the format version {0}, which this Express cannot bring up to date. The guide itself has not \
         been changed."
    )]
    NoMigrationPath(String),
    #[error("{0} is not an Express guide: it has no guide.json file inside.")]
    NotAGuide(String),
    #[error("{what} could not be read: {detail}. Nothing was changed.")]
    Damaged { what: String, detail: String },
    #[error("This guide is open read-only because another copy of Express has it open, so it cannot be saved.")]
    ReadOnly,
    #[error("{what}: {source}. Everything saved before this is safe.")]
    Io {
        what: String,
        #[source]
        source: io::Error,
    },
    #[error("A picture must be a PNG file.")]
    NotAPng,
}

/// Wrap an I/O error with a plain description of what was being done.
pub(crate) fn io_error(what: impl Into<String>) -> impl FnOnce(io::Error) -> StoreError {
    let what = what.into();
    move |source| StoreError::Io { what, source }
}
