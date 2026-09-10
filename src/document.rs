use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::FieldValue;

/// A document identified by a stable application-provided key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    id: String,
    text: String,
    fields: BTreeMap<String, FieldValue>,
}

impl Document {
    /// Rejects blank keys and preserves the supplied key and text unchanged.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Result<Self, DocumentError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(DocumentError::EmptyId);
        }
        Ok(Self {
            id,
            text: text.into(),
            fields: BTreeMap::new(),
        })
    }

    /// Attaches a typed field value, replacing any existing value for the name.
    pub fn with_field(mut self, name: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.insert(name.into(), value.into());
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the value stored under `name`, if any.
    pub fn field(&self, name: &str) -> Option<&FieldValue> {
        self.fields.get(name)
    }

    /// Iterates over all fields in name order.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &FieldValue)> {
        self.fields
            .iter()
            .map(|(name, value)| (name.as_str(), value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentError {
    EmptyId,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("document key must not be blank"),
        }
    }
}

impl Error for DocumentError {}
