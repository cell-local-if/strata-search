use std::collections::{BTreeMap, BTreeSet};

use crate::{Document, Schema, SchemaError};

/// An in-memory collection. Search currently scans the stored documents.
#[derive(Debug, Default)]
pub struct SearchIndex {
    documents: BTreeMap<String, Document>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Replaces an existing key, returning its previous document if present.
    pub fn insert(&mut self, document: Document) -> Option<Document> {
        self.documents.insert(document.id().to_owned(), document)
    }

    /// Validates the document against the schema, then inserts it.
    ///
    /// Validation runs completely before the index is touched: on failure the
    /// error is returned and the index is left exactly as it was, including
    /// any document already stored under the same key.
    pub fn insert_with_schema(
        &mut self,
        document: Document,
        schema: &Schema,
    ) -> Result<Option<Document>, SchemaError> {
        schema.validate(&document)?;
        Ok(self.insert(document))
    }

    pub fn get(&self, id: &str) -> Option<&Document> {
        self.documents.get(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Document> {
        self.documents.remove(id)
    }

    /// Matches all query tokens, returning unique documents in key order.
    /// An empty or whitespace-only query returns no documents.
    pub fn search(&self, query: &str) -> Vec<&Document> {
        let query_tokens = tokens(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }
        self.documents
            .values()
            .filter(|document| query_tokens.is_subset(&tokens(document.text())))
            .collect()
    }
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}
