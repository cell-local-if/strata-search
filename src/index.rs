use std::collections::{BTreeMap, BTreeSet};

use crate::{Document, Schema, SchemaError};

/// An in-memory collection backed by a private inverted term index.
///
/// `terms` maps each lowercased body token to the set of document keys whose
/// text contains it, and is kept in sync with `documents` on every mutation.
#[derive(Debug, Default)]
pub struct SearchIndex {
    documents: BTreeMap<String, Document>,
    terms: BTreeMap<String, BTreeSet<String>>,
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
        let id = document.id().to_owned();
        let previous = self.documents.insert(id.clone(), document);
        if let Some(old) = &previous {
            self.remove_terms(&id, old.text());
        }
        let text = self
            .documents
            .get(&id)
            .expect("just inserted")
            .text()
            .to_owned();
        self.add_terms(&id, &text);
        previous
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
        let removed = self.documents.remove(id)?;
        self.remove_terms(id, removed.text());
        Some(removed)
    }

    /// Matches all query tokens, returning unique documents in key order.
    /// An empty or whitespace-only query returns no documents.
    pub fn search(&self, query: &str) -> Vec<&Document> {
        let query_tokens = tokens(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }
        let mut hits: Option<BTreeSet<&str>> = None;
        for token in &query_tokens {
            let Some(keys) = self.terms.get(token) else {
                return Vec::new();
            };
            let keys = keys.iter().map(String::as_str).collect();
            hits = Some(match hits {
                None => keys,
                Some(previous) => previous.intersection(&keys).copied().collect(),
            });
        }
        hits.unwrap_or_default()
            .into_iter()
            .filter_map(|id| self.documents.get(id))
            .collect()
    }

    fn add_terms(&mut self, id: &str, text: &str) {
        for token in tokens(text) {
            self.terms.entry(token).or_default().insert(id.to_owned());
        }
    }

    fn remove_terms(&mut self, id: &str, text: &str) {
        for token in tokens(text) {
            if let Some(keys) = self.terms.get_mut(&token) {
                keys.remove(id);
                if keys.is_empty() {
                    self.terms.remove(&token);
                }
            }
        }
    }
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}
