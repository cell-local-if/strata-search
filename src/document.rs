use std::error::Error;
use std::fmt;

/// A document identified by a stable application-provided key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    id: String,
    text: String,
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
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn text(&self) -> &str {
        &self.text
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
