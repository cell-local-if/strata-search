use std::error::Error;
use std::fmt;

use crate::{FieldType, FieldValue};

/// An exact-match condition on one declared schema field.
///
/// Filters are expressed with typed values rather than query strings: a
/// document matches only when it carries a field with this name holding a
/// value equal (in type and value) to `value`. Multiple filters passed to
/// [`SearchIndex::filter_with_fields`](crate::SearchIndex::filter_with_fields)
/// are combined with AND.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldFilter {
    name: String,
    value: FieldValue,
}

impl FieldFilter {
    pub fn new(name: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &FieldValue {
        &self.value
    }
}

/// Why a typed field filter could not be applied.
///
/// Filters are checked against the [`Schema`](crate::Schema) supplied with the
/// query: unknown fields and value types that do not match the declared type
/// are reported as distinct variants, before any documents are examined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterError {
    /// The filter names a field the schema does not declare.
    UnknownField(String),
    /// The filter value does not match the declared field type.
    TypeMismatch {
        field: String,
        expected: FieldType,
        actual: FieldType,
    },
}

impl fmt::Display for FilterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField(name) => write!(formatter, "unknown filter field: {name}"),
            Self::TypeMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "filter on {field} expects {expected} but was given {actual}"
            ),
        }
    }
}

impl Error for FilterError {}
