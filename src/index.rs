use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Document, FacetCount, FacetError, FieldFilter, FieldValue, FilterError, Schema, SchemaError,
    SortDirection, SortError,
};

/// An in-memory collection backed by private inverted indexes.
///
/// `terms` maps each lowercased body token to the set of document keys whose
/// text contains it; `field_values` maps a field name to each of its values
/// and the keys carrying that value. Both maps are kept in sync with
/// `documents` on every mutation, including plain inserts and replacements.
#[derive(Debug, Default)]
pub struct SearchIndex {
    documents: BTreeMap<String, Document>,
    terms: BTreeMap<String, BTreeSet<String>>,
    field_values: BTreeMap<String, BTreeMap<FieldValue, BTreeSet<String>>>,
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
            let old_fields: Vec<_> = old
                .fields()
                .map(|(name, value)| (name.to_owned(), value.clone()))
                .collect();
            self.remove_terms(&id, old.text());
            self.remove_field_values(&id, old_fields);
        }
        let stored = self.documents.get(&id).expect("just inserted");
        let text = stored.text().to_owned();
        let fields = stored
            .fields()
            .map(|(name, value)| (name.to_owned(), value.clone()))
            .collect();
        self.add_terms(&id, &text);
        self.add_field_values(&id, fields);
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
        let fields = removed
            .fields()
            .map(|(name, value)| (name.to_owned(), value.clone()))
            .collect();
        self.remove_field_values(id, fields);
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
        self.collect_documents(hits.unwrap_or_default())
    }

    /// Returns documents that exactly match every filter, in key order.
    ///
    /// Matching is exact on both type and value, and filters combine with AND.
    /// A document missing any filtered field does not match. Every filter is
    /// checked against `schema` first: an unknown field or a value whose type
    /// disagrees with the schema returns a distinct [`FilterError`] without
    /// examining documents. An empty filter list matches every document.
    pub fn filter_with_fields<I, F>(
        &self,
        filters: I,
        schema: &Schema,
    ) -> Result<Vec<&Document>, FilterError>
    where
        I: IntoIterator<Item = F>,
        F: Borrow<FieldFilter>,
    {
        let hits = self.matching_keys(filters, schema)?;
        Ok(self.collect_documents(hits))
    }

    /// Returns documents matching all body query tokens AND every filter, in
    /// key order.
    ///
    /// Body search behaves exactly like [`SearchIndex::search`], including the
    /// empty-query rule: an empty or whitespace-only `query` matches no
    /// documents, even with an empty filter list. Field values never count as
    /// body tokens. Filter validation follows the same rules as
    /// [`SearchIndex::filter_with_fields`] and runs before any search.
    pub fn search_with_fields<I, F>(
        &self,
        query: &str,
        filters: I,
        schema: &Schema,
    ) -> Result<Vec<&Document>, FilterError>
    where
        I: IntoIterator<Item = F>,
        F: Borrow<FieldFilter>,
    {
        let mut hits = self.matching_keys(filters, schema)?;
        let query_tokens = tokens(query);
        if query_tokens.is_empty() {
            return Ok(Vec::new());
        }
        for token in &query_tokens {
            let Some(keys) = self.terms.get(token) else {
                return Ok(Vec::new());
            };
            let term_keys: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
            hits = hits.intersection(&term_keys).copied().collect();
        }
        Ok(self.collect_documents(hits))
    }

    /// Counts how many candidate documents carry each value of `field`,
    /// returning entries sorted by [`FieldValue`].
    ///
    /// Candidates are the whole index, optionally narrowed by body keywords
    /// and/or field filters: `query` uses the same Unicode-whitespace
    /// tokenization, lowercasing, and all-tokens matching as
    /// [`SearchIndex::search`], and `filters` follow the same exact-match AND
    /// rules as [`SearchIndex::filter_with_fields`]. A `None` query or one
    /// that tokenizes to nothing applies no keyword constraint, so the
    /// filtered set is counted as-is.
    ///
    /// Documents missing `field` never contribute. With no candidates, or
    /// candidates that all lack `field`, the result is empty. Validation runs
    /// before any counting: an undeclared `field` yields
    /// [`FacetError::UnknownField`], while filter problems surface as
    /// [`FacetError::Filter`].
    pub fn facet_counts<I, F>(
        &self,
        field: &str,
        query: Option<&str>,
        filters: I,
        schema: &Schema,
    ) -> Result<Vec<FacetCount>, FacetError>
    where
        I: IntoIterator<Item = F>,
        F: Borrow<FieldFilter>,
    {
        if schema.field(field).is_none() {
            return Err(FacetError::UnknownField(field.to_owned()));
        }
        let hits = self.candidate_keys(query, filters, schema)?;
        let counts = self
            .field_values
            .get(field)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(value, keys)| {
                        let count = keys.iter().filter(|id| hits.contains(id.as_str())).count();
                        (count > 0).then(|| FacetCount::new(value.clone(), count))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(counts)
    }

    /// Returns candidate documents ordered by the value of `field`.
    ///
    /// Candidates are selected exactly as in [`SearchIndex::facet_counts`]:
    /// the whole index, optionally narrowed by body keywords (`query` uses the
    /// same tokenization and all-tokens matching as [`SearchIndex::search`]; a
    /// `None` or tokenless query applies no keyword constraint) and/or exact
    /// [`FieldFilter`]s combined with AND.
    ///
    /// Documents carrying `field` come first, ordered by their [`FieldValue`]
    /// in `direction`; documents missing `field` follow, in document-key
    /// order, in either direction. Documents sharing one value are ordered by
    /// document key, and [`SortDirection::Descending`] reverses only the
    /// value order, never that key order. Validation runs before any sorting:
    /// an undeclared `field` yields [`SortError::UnknownField`], while filter
    /// problems surface as [`SortError::Filter`].
    pub fn search_sorted<I, F>(
        &self,
        field: &str,
        direction: SortDirection,
        query: Option<&str>,
        filters: I,
        schema: &Schema,
    ) -> Result<Vec<&Document>, SortError>
    where
        I: IntoIterator<Item = F>,
        F: Borrow<FieldFilter>,
    {
        if schema.field(field).is_none() {
            return Err(SortError::UnknownField(field.to_owned()));
        }
        let hits = self.candidate_keys(query, filters, schema)?;
        let mut sorted: Vec<&str> = Vec::with_capacity(hits.len());
        if let Some(values) = self.field_values.get(field) {
            let values: Vec<&BTreeSet<String>> = match direction {
                SortDirection::Ascending => values.values().collect(),
                SortDirection::Descending => values.values().rev().collect(),
            };
            for keys in values {
                sorted.extend(
                    keys.iter()
                        .map(String::as_str)
                        .filter(|id| hits.contains(id)),
                );
            }
        }
        let valued: BTreeSet<&str> = sorted.iter().copied().collect();
        sorted.extend(hits.iter().copied().filter(|id| !valued.contains(id)));
        Ok(sorted
            .into_iter()
            .filter_map(|id| self.documents.get(id))
            .collect())
    }

    /// Narrows the whole index by optional body keywords and field filters.
    /// A `None` or tokenless `query` applies no keyword constraint.
    fn candidate_keys<'s, I, F>(
        &'s self,
        query: Option<&str>,
        filters: I,
        schema: &Schema,
    ) -> Result<BTreeSet<&'s str>, FilterError>
    where
        I: IntoIterator<Item = F>,
        F: Borrow<FieldFilter>,
    {
        let mut hits = self.matching_keys(filters, schema)?;
        let query_tokens = query.map(tokens).unwrap_or_default();
        if !query_tokens.is_empty() {
            let mut matched: Option<BTreeSet<&str>> = None;
            for token in &query_tokens {
                let Some(keys) = self.terms.get(token) else {
                    return Ok(BTreeSet::new());
                };
                let keys: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
                matched = Some(match matched {
                    None => keys,
                    Some(previous) => previous.intersection(&keys).copied().collect(),
                });
            }
            hits = hits
                .intersection(&matched.unwrap_or_default())
                .copied()
                .collect();
        }
        Ok(hits)
    }

    /// Validates every filter against the schema, then intersects their
    /// posting lists. With no filters every stored key is a candidate.
    fn matching_keys<'s, I, F>(
        &'s self,
        filters: I,
        schema: &Schema,
    ) -> Result<BTreeSet<&'s str>, FilterError>
    where
        I: IntoIterator<Item = F>,
        F: Borrow<FieldFilter>,
    {
        let mut hits: Option<BTreeSet<&str>> = None;
        for filter in filters {
            let filter = filter.borrow();
            let Some(declaration) = schema.field(filter.name()) else {
                return Err(FilterError::UnknownField(filter.name().to_owned()));
            };
            let expected = declaration.field_type();
            let actual = filter.value().field_type();
            if actual != expected {
                return Err(FilterError::TypeMismatch {
                    field: filter.name().to_owned(),
                    expected,
                    actual,
                });
            }
            let keys = self
                .field_values
                .get(filter.name())
                .and_then(|values| values.get(filter.value()))
                .map(|postings| postings.iter().map(String::as_str).collect())
                .unwrap_or_default();
            hits = Some(match hits {
                None => keys,
                Some(previous) => previous.intersection(&keys).copied().collect(),
            });
        }
        Ok(hits.unwrap_or_else(|| self.documents.keys().map(String::as_str).collect()))
    }

    fn collect_documents(&self, keys: BTreeSet<&str>) -> Vec<&Document> {
        keys.into_iter()
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

    fn add_field_values(&mut self, id: &str, fields: Vec<(String, FieldValue)>) {
        for (name, value) in fields {
            self.field_values
                .entry(name)
                .or_default()
                .entry(value)
                .or_default()
                .insert(id.to_owned());
        }
    }

    fn remove_field_values(&mut self, id: &str, fields: Vec<(String, FieldValue)>) {
        for (name, value) in fields {
            if let Some(values) = self.field_values.get_mut(&name) {
                if let Some(keys) = values.get_mut(&value) {
                    keys.remove(id);
                    if keys.is_empty() {
                        values.remove(&value);
                    }
                }
                if values.is_empty() {
                    self.field_values.remove(&name);
                }
            }
        }
    }
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}
