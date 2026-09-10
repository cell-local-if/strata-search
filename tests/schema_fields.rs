use strata_search::{
    Document, FieldSchema, FieldType, FieldValue, Schema, SchemaError, SearchIndex,
};

fn schema() -> Schema {
    let mut schema = Schema::new();
    schema.add_field(FieldSchema::new("title", FieldType::Text, true));
    schema.add_field(FieldSchema::new("year", FieldType::Integer, false));
    schema.add_field(FieldSchema::new("published", FieldType::Boolean, false));
    schema
}

fn document(id: &str, text: &str) -> Document {
    Document::new(id, text).expect("nonblank document key")
}

#[test]
fn accepts_documents_with_valid_fields_and_exposes_values() {
    let mut index = SearchIndex::new();
    let doc = document("guide", "Rust storage guide")
        .with_field("title", "Storage Guide")
        .with_field("year", 2026_i64)
        .with_field("published", true);
    assert!(index.insert_with_schema(doc, &schema()).unwrap().is_none());

    let stored = index.get("guide").unwrap();
    assert_eq!(
        stored.field("title"),
        Some(&FieldValue::Text("Storage Guide".to_owned()))
    );
    assert_eq!(stored.field("year"), Some(&FieldValue::Integer(2026)));
    assert_eq!(stored.field("published"), Some(&FieldValue::Boolean(true)));
    assert_eq!(stored.field("missing"), None);
    assert_eq!(index.len(), 1);
}

#[test]
fn optional_fields_may_be_omitted() {
    let mut index = SearchIndex::new();
    let doc = document("guide", "text").with_field("title", "Only Required");
    assert!(index.insert_with_schema(doc, &schema()).is_ok());
    assert_eq!(index.get("guide").unwrap().field("year"), None);
}

#[test]
fn rejects_unknown_fields() {
    let mut index = SearchIndex::new();
    let doc = document("guide", "text")
        .with_field("title", "T")
        .with_field("surprise", "not declared");
    assert_eq!(
        index.insert_with_schema(doc, &schema()),
        Err(SchemaError::UnknownField("surprise".to_owned()))
    );
    assert!(index.is_empty());
}

#[test]
fn rejects_missing_required_fields() {
    let mut index = SearchIndex::new();
    let doc = document("guide", "text").with_field("year", 2026_i64);
    assert_eq!(
        index.insert_with_schema(doc, &schema()),
        Err(SchemaError::MissingField("title".to_owned()))
    );
    assert!(index.is_empty());
}

#[test]
fn rejects_type_mismatches_with_expected_and_actual_types() {
    let mut index = SearchIndex::new();
    let doc = document("guide", "text")
        .with_field("title", "T")
        .with_field("year", "not an integer");
    assert_eq!(
        index.insert_with_schema(doc, &schema()),
        Err(SchemaError::TypeMismatch {
            field: "year".to_owned(),
            expected: FieldType::Integer,
            actual: FieldType::Text,
        })
    );
    assert!(index.is_empty());
}

#[test]
fn failed_replacement_leaves_existing_document_and_search_untouched() {
    let schema = schema();
    let mut index = SearchIndex::new();
    index
        .insert_with_schema(
            document("guide", "old material").with_field("title", "Old"),
            &schema,
        )
        .unwrap();

    for invalid in [
        document("guide", "new material").with_field("unknown", 1_i64),
        document("guide", "new material"),
        document("guide", "new material").with_field("title", 42_i64),
    ] {
        assert!(index.insert_with_schema(invalid, &schema).is_err());
        assert_eq!(index.len(), 1);
        let stored = index.get("guide").unwrap();
        assert_eq!(stored.text(), "old material");
        assert_eq!(
            stored.field("title"),
            Some(&FieldValue::Text("Old".to_owned()))
        );
        assert_eq!(index.search("old")[0].id(), "guide");
        assert!(index.search("new").is_empty());
    }

    // Removal still observes the original document.
    let removed = index.remove("guide").unwrap();
    assert_eq!(removed.text(), "old material");
    assert!(index.is_empty());
}

#[test]
fn valid_replacement_returns_previous_document_with_its_fields() {
    let schema = schema();
    let mut index = SearchIndex::new();
    index
        .insert_with_schema(document("guide", "old").with_field("title", "Old"), &schema)
        .unwrap();
    let old = index
        .insert_with_schema(document("guide", "new").with_field("title", "New"), &schema)
        .unwrap()
        .unwrap();
    assert_eq!(old.text(), "old");
    assert_eq!(
        old.field("title"),
        Some(&FieldValue::Text("Old".to_owned()))
    );
    assert_eq!(
        index.get("guide").unwrap().field("title"),
        Some(&FieldValue::Text("New".to_owned()))
    );
}

#[test]
fn plain_text_documents_and_search_still_work_without_schema() {
    let mut index = SearchIndex::new();
    // Documents without fields can still be inserted and searched as before.
    index.insert(document("plain", "Rust storage guide"));
    // Schema-validated documents share the same text search semantics.
    index
        .insert_with_schema(
            document("typed", "Rust FIELD guide").with_field("title", "T"),
            &schema(),
        )
        .unwrap();

    let ids: Vec<_> = index
        .search("RUST guide")
        .iter()
        .map(|doc| doc.id())
        .collect();
    assert_eq!(ids, ["plain", "typed"]);
    assert_eq!(index.search("field")[0].id(), "typed");
    assert!(index.get("plain").unwrap().field("title").is_none());
}
