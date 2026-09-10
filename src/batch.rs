use std::error::Error;
use std::fmt;

use crate::SchemaError;

/// Why a schema-validated batch insert failed.
///
/// Every variant carries the position of the offending document within the
/// batch (`index`) and its document key, so callers can locate the failure
/// without re-validating. A failed batch leaves the index completely
/// untouched: validation of the whole batch finishes before any document is
/// stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchError {
    /// The document at this position failed schema validation; `error` keeps
    /// the unknown-field, missing-field, and type-mismatch cases distinct.
    Schema {
        index: usize,
        key: String,
        error: SchemaError,
    },
    /// The document at this position repeats a key that already appeared
    /// earlier in the same batch.
    DuplicateKey { index: usize, key: String },
}

impl BatchError {
    /// The position of the offending document within the submitted batch.
    pub fn index(&self) -> usize {
        match self {
            Self::Schema { index, .. } | Self::DuplicateKey { index, .. } => *index,
        }
    }

    /// The key of the offending document.
    pub fn key(&self) -> &str {
        match self {
            Self::Schema { key, .. } | Self::DuplicateKey { key, .. } => key,
        }
    }
}

impl fmt::Display for BatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema { index, key, error } => {
                write!(formatter, "batch document {index} ({key}): {error}")
            }
            Self::DuplicateKey { index, key } => {
                write!(formatter, "batch document {index} repeats key: {key}")
            }
        }
    }
}

impl Error for BatchError {}
