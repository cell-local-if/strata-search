use std::error::Error;
use std::fmt;

use crate::{FieldValue, FilterError};

/// One entry of a facet count result: a field value and how many candidate
/// documents carry it.
///
/// Entries are produced by
/// [`SearchIndex::facet_counts`](crate::SearchIndex::facet_counts) in
/// [`FieldValue`] order; documents missing the faceted field never
/// contribute to any entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetCount {
    value: FieldValue,
    count: usize,
}

impl FacetCount {
    pub(crate) fn new(value: FieldValue, count: usize) -> Self {
        Self { value, count }
    }

    pub fn value(&self) -> &FieldValue {
        &self.value
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

/// Why a facet count request could not be served.
///
/// The target field is checked against the [`Schema`](crate::Schema) first,
/// then every filter; each failure is a distinct variant so callers can tell
/// an unknown target field apart from filter problems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FacetError {
    /// The faceted field is not declared in the schema.
    UnknownField(String),
    /// One of the candidate-narrowing filters is invalid.
    Filter(FilterError),
}

impl From<FilterError> for FacetError {
    fn from(error: FilterError) -> Self {
        Self::Filter(error)
    }
}

impl fmt::Display for FacetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField(name) => write!(formatter, "unknown facet field: {name}"),
            Self::Filter(error) => write!(formatter, "invalid facet filter: {error}"),
        }
    }
}

impl Error for FacetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownField(_) => None,
            Self::Filter(error) => Some(error),
        }
    }
}
