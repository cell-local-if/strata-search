use strata_search::{
    BatchError, Document, FieldFilter, FieldSchema, FieldType, FieldValue, Schema, SchemaError,
    SearchIndex, SortDirection,
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

fn typed(id: &str, text: &str, title: &str, year: i64) -> Document {
    document(id, text)
        .with_field("title", title)
        .with_field("year", year)
}

fn ids<'a>(hits: &[&'a Document]) -> Vec<&'a str> {
    hits.iter().map(|doc| doc.id()).collect()
}

#[test]
fn empty_batches_change_nothing() {
    let mut index = SearchIndex::new();
    index.insert(document("keep", "original text"));

    let replaced = index.insert_batch(Vec::new());
    assert_eq!(replaced, Vec::<Option<Document>>::new());

    let replaced = index
        .insert_batch_with_schema(Vec::new(), &schema())
        .unwrap();
    assert_eq!(replaced, Vec::<Option<Document>>::new());

    assert_eq!(index.len(), 1);
    assert_eq!(index.get("keep").unwrap().text(), "original text");
}

#[test]
fn plain_batch_inserts_in_order_and_reports_replacements() {
    let mut index = SearchIndex::new();
    index.insert(document("old", "stale alpha"));

    let replaced = index.insert_batch(vec![
        document("new-a", "fresh alpha"),
        document("old", "replacement beta"),
        document("new-b", "fresh beta"),
    ]);

    // One entry per input position, in input order.
    assert_eq!(replaced.len(), 3);
    assert_eq!(replaced[0], None);
    assert_eq!(replaced[1].as_ref().unwrap().text(), "stale alpha");
    assert_eq!(replaced[2], None);

    assert_eq!(index.len(), 3);
    // Term postings reflect every document, including the replacement.
    assert_eq!(ids(&index.search("fresh")), ["new-a", "new-b"]);
    assert_eq!(ids(&index.search("alpha")), ["new-a"]);
    assert_eq!(ids(&index.search("beta")), ["new-b", "old"]);
    assert!(index.search("stale").is_empty());
}

#[test]
fn plain_batch_equivalent_to_repeated_single_inserts() {
    let docs = || {
        vec![
            document("a", "shared one"),
            document("b", "shared two"),
            document("a", "shared three"),
        ]
    };

    let mut batched = SearchIndex::new();
    let batch_returns = batched.insert_batch(docs());

    let mut stepped = SearchIndex::new();
    let step_returns: Vec<_> = docs().into_iter().map(|doc| stepped.insert(doc)).collect();

    // Same observable results, including the within-batch replacement.
    assert_eq!(batch_returns.len(), 3);
    assert_eq!(batch_returns[2].as_ref().unwrap().text(), "shared one");
    assert_eq!(step_returns, batch_returns);
    assert_eq!(batched.len(), stepped.len());
    for key in ["a", "b"] {
        assert_eq!(batched.get(key), stepped.get(key));
    }
    assert_eq!(ids(&batched.search("shared")), ["a", "b"]);
    assert_eq!(ids(&batched.search("three")), ["a"]);
}

#[test]
fn schema_batch_success_updates_every_query_entry_point() {
    let schema = schema();
    let mut index = SearchIndex::new();
    index
        .insert_with_schema(typed("guide", "old material", "Old", 2020), &schema)
        .unwrap();

    let replaced = index
        .insert_batch_with_schema(
            vec![
                typed("guide", "fresh material", "New", 2026).with_field("published", true),
                typed("draft", "fresh notes", "Draft", 2025).with_field("published", false),
            ],
            &schema,
        )
        .unwrap();

    assert_eq!(replaced.len(), 2);
    assert_eq!(
        replaced[0].as_ref().unwrap().field("title"),
        Some(&FieldValue::Text("Old".to_owned()))
    );
    assert_eq!(replaced[1], None);

    // Body search sees the new text only.
    assert_eq!(ids(&index.search("fresh")), ["draft", "guide"]);
    assert!(index.search("old").is_empty());

    // Field filtering sees the new values only.
    let year_filter = [FieldFilter::new("year", 2026_i64)];
    assert_eq!(
        ids(&index.filter_with_fields(&year_filter, &schema).unwrap()),
        ["guide"]
    );
    let published_filter = [FieldFilter::new("published", false)];
    assert_eq!(
        ids(&index
            .search_with_fields("fresh", &published_filter, &schema)
            .unwrap()),
        ["draft"]
    );

    // Facet counts track the replaced values.
    let counts = index
        .facet_counts("year", None, Vec::<FieldFilter>::new(), &schema)
        .unwrap();
    assert_eq!(
        counts
            .iter()
            .map(|entry| (entry.value().clone(), entry.count()))
            .collect::<Vec<_>>(),
        vec![
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 1)
        ]
    );

    // Sorted retrieval follows the new values.
    let sorted = index
        .search_sorted(
            "year",
            SortDirection::Descending,
            None,
            Vec::<FieldFilter>::new(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&sorted), ["guide", "draft"]);
}

#[test]
fn schema_batch_rejects_duplicate_keys_with_position() {
    let schema = schema();
    let mut index = SearchIndex::new();
    index
        .insert_with_schema(typed("keep", "original text", "Keep", 2020), &schema)
        .unwrap();

    let error = index
        .insert_batch_with_schema(
            vec![
                typed("new", "first copy", "A", 2021),
                typed("other", "unrelated", "B", 2022),
                typed("new", "second copy", "C", 2023),
            ],
            &schema,
        )
        .unwrap_err();
    assert_eq!(
        error,
        BatchError::DuplicateKey {
            index: 2,
            key: "new".to_owned()
        }
    );
    assert_eq!(error.index(), 2);
    assert_eq!(error.key(), "new");

    // Nothing from the batch was applied.
    assert_eq!(index.len(), 1);
    assert!(index.get("new").is_none());
    assert!(index.get("other").is_none());
}

#[test]
fn schema_batch_reports_each_schema_error_kind_with_position_and_key() {
    let schema = schema();

    let cases: Vec<(Vec<Document>, SchemaError)> = vec![
        (
            vec![
                typed("ok", "fine", "A", 2021),
                typed("bad", "text", "B", 2022).with_field("surprise", 1_i64),
            ],
            SchemaError::UnknownField("surprise".to_owned()),
        ),
        (
            vec![
                typed("ok", "fine", "A", 2021),
                document("bad", "text").with_field("year", 2022_i64),
            ],
            SchemaError::MissingField("title".to_owned()),
        ),
        (
            vec![
                typed("ok", "fine", "A", 2021),
                typed("bad", "text", "B", 2022).with_field("published", "yes"),
            ],
            SchemaError::TypeMismatch {
                field: "published".to_owned(),
                expected: FieldType::Boolean,
                actual: FieldType::Text,
            },
        ),
    ];

    for (batch, expected) in cases {
        let mut index = SearchIndex::new();
        let error = index.insert_batch_with_schema(batch, &schema).unwrap_err();
        assert_eq!(
            error,
            BatchError::Schema {
                index: 1,
                key: "bad".to_owned(),
                error: expected
            }
        );
        // Even the valid first document was not applied.
        assert!(index.is_empty());
    }
}

#[test]
fn failed_schema_batch_leaves_every_query_entry_point_untouched() {
    let schema = schema();
    let mut index = SearchIndex::new();
    index
        .insert_with_schema(
            typed("guide", "old material", "Old", 2020).with_field("published", true),
            &schema,
        )
        .unwrap();
    index
        .insert_with_schema(
            typed("draft", "old notes", "Draft", 2019).with_field("published", false),
            &schema,
        )
        .unwrap();

    // The batch replaces `guide`, adds `new`, then fails on `broken`:
    // none of it may take effect.
    let batch = vec![
        typed("guide", "new material", "New", 2026).with_field("published", false),
        typed("new", "new arrival", "New", 2026).with_field("published", true),
        document("broken", "broken text").with_field("year", "not an integer"),
    ];
    let error = index.insert_batch_with_schema(batch, &schema).unwrap_err();
    assert_eq!(error.index(), 2);
    assert_eq!(error.key(), "broken");

    // Document table and length.
    assert_eq!(index.len(), 2);
    assert_eq!(index.get("guide").unwrap().text(), "old material");
    assert_eq!(
        index.get("guide").unwrap().field("year"),
        Some(&FieldValue::Integer(2020))
    );
    assert!(index.get("new").is_none());
    assert!(index.get("broken").is_none());

    // Body search.
    assert_eq!(ids(&index.search("old")), ["draft", "guide"]);
    assert!(index.search("new").is_empty());
    assert!(index.search("broken").is_empty());

    // Field filtering and combined search.
    let year_filter = [FieldFilter::new("year", 2020_i64)];
    assert_eq!(
        ids(&index.filter_with_fields(&year_filter, &schema).unwrap()),
        ["guide"]
    );
    let published_filter = [FieldFilter::new("published", true)];
    assert_eq!(
        ids(&index
            .search_with_fields("old", &published_filter, &schema)
            .unwrap()),
        ["guide"]
    );

    // Facet counts.
    let counts = index
        .facet_counts("year", None, Vec::<FieldFilter>::new(), &schema)
        .unwrap();
    assert_eq!(
        counts
            .iter()
            .map(|entry| (entry.value().clone(), entry.count()))
            .collect::<Vec<_>>(),
        vec![
            (FieldValue::Integer(2019), 1),
            (FieldValue::Integer(2020), 1)
        ]
    );

    // Sorted retrieval.
    let sorted = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            Vec::<FieldFilter>::new(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&sorted), ["draft", "guide"]);

    // Removal still observes the pre-batch documents.
    assert_eq!(index.remove("guide").unwrap().text(), "old material");
    assert_eq!(index.remove("draft").unwrap().text(), "old notes");
    assert!(index.is_empty());
}

#[test]
fn single_document_apis_still_work_alongside_batches() {
    let schema = schema();
    let mut index = SearchIndex::new();

    // Plain single insert, then a batch, then schema-validated single insert.
    index.insert(document("plain", "plain text"));
    let replaced = index.insert_batch(vec![document("plain", "batch text")]);
    assert_eq!(replaced[0].as_ref().unwrap().text(), "plain text");
    index
        .insert_with_schema(typed("typed", "typed text", "T", 2026), &schema)
        .unwrap();

    assert_eq!(index.len(), 2);
    assert_eq!(index.get("plain").unwrap().text(), "batch text");
    assert_eq!(ids(&index.search("text")), ["plain", "typed"]);

    // Single-document failure semantics are unchanged.
    let invalid = document("typed", "bad").with_field("unknown", 1_i64);
    assert_eq!(
        index.insert_with_schema(invalid, &schema),
        Err(SchemaError::UnknownField("unknown".to_owned()))
    );
    assert_eq!(index.get("typed").unwrap().text(), "typed text");

    assert_eq!(index.remove("plain").unwrap().text(), "batch text");
    assert_eq!(index.len(), 1);
}
