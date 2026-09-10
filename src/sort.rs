use std::error::Error;
use std::fmt;

use crate::FilterError;

/// The direction a sorted search orders field values in.
///
/// Documents missing the sort field always come after documents carrying a
/// value, in either direction; descending reverses only the order of the
/// values themselves, never the document-key order within one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Why a sorted search could not be served.
///
/// The sort field is checked against the [`Schema`](crate::Schema) first,
/// then every filter; each failure is a distinct variant so callers can tell
/// an unknown sort field apart from filter problems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortError {
    /// The sort field is not declared in the schema.
    UnknownField(String),
    /// One of the candidate-narrowing filters is invalid.
    Filter(FilterError),
}

impl From<FilterError> for SortError {
    fn from(error: FilterError) -> Self {
        Self::Filter(error)
    }
}

impl fmt::Display for SortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField(name) => write!(formatter, "unknown sort field: {name}"),
            Self::Filter(error) => write!(formatter, "invalid sort filter: {error}"),
        }
    }
}

impl Error for SortError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownField(_) => None,
            Self::Filter(error) => Some(error),
        }
    }
}
