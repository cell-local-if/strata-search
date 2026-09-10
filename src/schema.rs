use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::Document;

/// The type a schema declares for a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Integer,
    Boolean,
}

impl fmt::Display for FieldType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
        };
        formatter.write_str(name)
    }
}

/// A typed field value stored on a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
}

impl FieldValue {
    pub fn field_type(&self) -> FieldType {
        match self {
            Self::Text(_) => FieldType::Text,
            Self::Integer(_) => FieldType::Integer,
            Self::Boolean(_) => FieldType::Boolean,
        }
    }
}

impl From<String> for FieldValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for FieldValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<i64> for FieldValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for FieldValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

/// One field declaration: name, type, and whether a value is required.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSchema {
    name: String,
    field_type: FieldType,
    required: bool,
}

impl FieldSchema {
    pub fn new(name: impl Into<String>, field_type: FieldType, required: bool) -> Self {
        Self {
            name: name.into(),
            field_type,
            required,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn field_type(&self) -> FieldType {
        self.field_type
    }

    pub fn is_required(&self) -> bool {
        self.required
    }
}

/// A set of field declarations used to validate documents before insertion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schema {
    fields: BTreeMap<String, FieldSchema>,
}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a field declaration, keyed by field name.
    pub fn add_field(&mut self, field: FieldSchema) -> &mut Self {
        self.fields.insert(field.name.clone(), field);
        self
    }

    pub fn field(&self, name: &str) -> Option<&FieldSchema> {
        self.fields.get(name)
    }

    /// Checks a document against every declared field without modifying anything.
    ///
    /// Unknown fields and type mismatches are reported in field-name order,
    /// then missing required fields; the first failure is returned.
    pub fn validate(&self, document: &Document) -> Result<(), SchemaError> {
        for (name, value) in document.fields() {
            match self.fields.get(name) {
                None => return Err(SchemaError::UnknownField(name.to_owned())),
                Some(field) => {
                    let expected = field.field_type();
                    let actual = value.field_type();
                    if actual != expected {
                        return Err(SchemaError::TypeMismatch {
                            field: name.to_owned(),
                            expected,
                            actual,
                        });
                    }
                }
            }
        }
        for field in self.fields.values() {
            if field.is_required() && document.field(field.name()).is_none() {
                return Err(SchemaError::MissingField(field.name().to_owned()));
            }
        }
        Ok(())
    }
}

/// Why a document failed schema validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    /// The document carries a field the schema does not declare.
    UnknownField(String),
    /// A required field has no value on the document.
    MissingField(String),
    /// A field value does not match the declared type.
    TypeMismatch {
        field: String,
        expected: FieldType,
        actual: FieldType,
    },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField(name) => write!(formatter, "unknown field: {name}"),
            Self::MissingField(name) => write!(formatter, "missing required field: {name}"),
            Self::TypeMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "field {field} expects {expected} but holds {actual}"
            ),
        }
    }
}

impl Error for SchemaError {}
