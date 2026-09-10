use strata_search::{Document, FieldSchema, FieldType, Schema, SearchIndex};

fn document(id: &str, text: &str) -> Document {
    Document::new(id, text).expect("nonblank document key")
}

fn ids<'a>(hits: &[&'a Document]) -> Vec<&'a str> {
    hits.iter().map(|doc| doc.id()).collect()
}

#[test]
fn multi_token_query_intersects_posting_lists() {
    let mut index = SearchIndex::new();
    index.insert(document("both", "rust storage guide"));
    index.insert(document("only-rust", "rust networking"));
    index.insert(document("only-storage", "storage engine"));
    index.insert(document("neither", "unrelated text"));

    assert_eq!(ids(&index.search("rust storage")), ["both"]);
    assert_eq!(ids(&index.search("storage rust")), ["both"]);
    assert_eq!(
        ids(&index.search("rust storage missing")),
        Vec::<&str>::new()
    );
}

#[test]
fn replacement_removes_stale_terms_and_indexes_new_text() {
    let mut index = SearchIndex::new();
    index.insert(document("guide", "alpha beta"));
    index.insert(document("other", "alpha gamma"));

    let old = index.insert(document("guide", "delta beta")).unwrap();
    assert_eq!(old.text(), "alpha beta");

    // Old unique term no longer resolves to the replaced document.
    assert_eq!(ids(&index.search("alpha")), ["other"]);
    // New term resolves; shared term still finds both.
    assert_eq!(ids(&index.search("delta")), ["guide"]);
    assert_eq!(ids(&index.search("beta")), ["guide"]);
    assert_eq!(ids(&index.search("alpha beta")), Vec::<&str>::new());
}

#[test]
fn removal_cleans_terms_and_empty_posting_lists() {
    let mut index = SearchIndex::new();
    index.insert(document("a", "shared unique-a"));
    index.insert(document("b", "shared unique-b"));

    assert_eq!(index.remove("a").unwrap().id(), "a");
    // The term only `a` carried is gone entirely; shared terms keep `b`.
    assert!(index.search("unique-a").is_empty());
    assert_eq!(ids(&index.search("shared")), ["b"]);
    assert_eq!(ids(&index.search("unique-b")), ["b"]);

    assert_eq!(index.remove("b").unwrap().id(), "b");
    assert!(index.search("shared").is_empty());
    assert!(index.is_empty());

    // The index still works after being drained.
    index.insert(document("c", "shared again"));
    assert_eq!(ids(&index.search("shared")), ["c"]);
}

#[test]
fn schema_validated_documents_are_indexed_and_searchable() {
    let mut schema = Schema::new();
    schema.add_field(FieldSchema::new("title", FieldType::Text, true));

    let mut index = SearchIndex::new();
    index
        .insert_with_schema(
            document("typed", "rust storage").with_field("title", "Guide"),
            &schema,
        )
        .unwrap();
    index.insert(document("plain", "rust storage"));

    assert_eq!(ids(&index.search("rust storage")), ["plain", "typed"]);

    // Replacement through the schema path also refreshes the term index.
    index
        .insert_with_schema(
            document("typed", "rust archive").with_field("title", "Guide"),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&index.search("storage")), ["plain"]);
    assert_eq!(ids(&index.search("archive")), ["typed"]);
}

#[test]
fn field_values_do_not_participate_in_text_search() {
    let mut schema = Schema::new();
    schema.add_field(FieldSchema::new("title", FieldType::Text, true));
    schema.add_field(FieldSchema::new("year", FieldType::Integer, false));

    let mut index = SearchIndex::new();
    index
        .insert_with_schema(
            document("typed", "body text")
                .with_field("title", "Storage Guide")
                .with_field("year", 2026_i64),
            &schema,
        )
        .unwrap();

    // Terms that only appear in field values never match.
    for query in ["storage", "guide", "Storage", "2026"] {
        assert!(index.search(query).is_empty(), "query {query:?} matched");
    }
    assert_eq!(ids(&index.search("body")), ["typed"]);
}

#[test]
fn failed_schema_insert_leaves_term_index_untouched() {
    let mut schema = Schema::new();
    schema.add_field(FieldSchema::new("title", FieldType::Text, true));

    let mut index = SearchIndex::new();
    index
        .insert_with_schema(
            document("guide", "old material").with_field("title", "T"),
            &schema,
        )
        .unwrap();

    // Missing required field: rejected before any index mutation.
    let invalid = document("guide", "new material");
    assert!(index.insert_with_schema(invalid, &schema).is_err());
    assert_eq!(ids(&index.search("old")), ["guide"]);
    assert!(index.search("new").is_empty());
    assert!(index.search("material").len() == 1);
}

#[test]
fn legacy_query_semantics_are_preserved() {
    let mut index = SearchIndex::new();
    index.insert(document("z", "CAFÉ\u{2003}索引\nrust,"));
    index.insert(document("a", "café rust,"));
    index.insert(document("empty", ""));

    // Unicode whitespace splitting and case folding.
    assert_eq!(ids(&index.search("CAFÉ\t索引")), ["z"]);
    // Whole-token matching: punctuation stays attached to the token.
    assert!(index.search("rust").is_empty());
    assert_eq!(ids(&index.search("RUST,")), ["a", "z"]);
    // Empty and whitespace-only queries match nothing, as do empty documents.
    for query in ["", " \n\t", "\u{2003}"] {
        assert!(index.search(query).is_empty());
    }
}
