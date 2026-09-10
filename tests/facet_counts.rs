use strata_search::{
    Document, FacetCount, FacetError, FieldFilter, FieldSchema, FieldType, FieldValue, FilterError,
    Schema, SearchIndex,
};

fn schema() -> Schema {
    let mut schema = Schema::new();
    schema.add_field(FieldSchema::new("title", FieldType::Text, true));
    schema.add_field(FieldSchema::new("year", FieldType::Integer, false));
    schema.add_field(FieldSchema::new("published", FieldType::Boolean, false));
    schema.add_field(FieldSchema::new("category", FieldType::Text, false));
    schema
}

fn document(id: &str, text: &str) -> Document {
    Document::new(id, text).expect("nonblank document key")
}

fn ids<'a>(hits: &[&'a Document]) -> Vec<&'a str> {
    hits.iter().map(|doc| doc.id()).collect()
}

fn no_filters() -> Vec<FieldFilter> {
    Vec::new()
}

fn pairs(counts: &[FacetCount]) -> Vec<(FieldValue, usize)> {
    counts
        .iter()
        .map(|entry| (entry.value().clone(), entry.count()))
        .collect()
}

/// Same corpus as the filter tests: every field type is represented, `d`
/// lacks `published`, and `p` enters through the schema-free `insert` path
/// carrying only a `year` field.
fn populated_index() -> SearchIndex {
    let schema = schema();
    let mut index = SearchIndex::new();
    index
        .insert_with_schema(
            document("a", "rust storage guide")
                .with_field("title", "Guide One")
                .with_field("year", 2025_i64)
                .with_field("published", true)
                .with_field("category", "rust"),
            &schema,
        )
        .unwrap();
    index
        .insert_with_schema(
            document("b", "rust networking")
                .with_field("title", "Networking Book")
                .with_field("year", 2026_i64)
                .with_field("published", false)
                .with_field("category", "rust"),
            &schema,
        )
        .unwrap();
    index
        .insert_with_schema(
            document("c", "rust storage deep dive")
                .with_field("title", "Storage Deep Dive")
                .with_field("year", 2026_i64)
                .with_field("published", true)
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    index
        .insert_with_schema(
            document("d", "unrelated material")
                .with_field("title", "Miscellaneous")
                .with_field("year", 2026_i64)
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    index.insert(document("p", "plain only").with_field("year", 2026_i64));
    index
}

#[test]
fn counts_each_field_type_sorted_by_value() {
    let index = populated_index();
    let schema = schema();

    // Text field over the whole index; `p` has no category and is skipped.
    let counts = index
        .facet_counts("category", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Text("rust".to_owned()), 2),
            (FieldValue::Text("storage".to_owned()), 2),
        ]
    );

    // Integer field; every document carries `year`.
    let counts = index
        .facet_counts("year", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 4),
        ]
    );

    // Boolean field; `d` and `p` lack `published` and are not counted.
    let counts = index
        .facet_counts("published", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Boolean(false), 1),
            (FieldValue::Boolean(true), 2),
        ]
    );
}

#[test]
fn keywords_narrow_the_counted_candidates() {
    let index = populated_index();
    let schema = schema();

    // "rust" matches a, b, c; their years are 2025, 2026, 2026.
    let counts = index
        .facet_counts("year", Some("rust"), no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 2),
        ]
    );

    // Tokenization matches `search`: Unicode whitespace, case-insensitive,
    // every token required.
    let counts = index
        .facet_counts(
            "year",
            Some("  RUST\tStorage\u{2003}"),
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 1),
        ]
    );

    // A keyword absent from every body empties the candidate set.
    let counts = index
        .facet_counts("year", Some("missing"), no_filters(), &schema)
        .unwrap();
    assert!(counts.is_empty());
}

#[test]
fn keywords_and_filters_combine() {
    let index = populated_index();
    let schema = schema();

    // Body "rust" (a, b, c) AND published = true (a, c).
    let counts = index
        .facet_counts(
            "year",
            Some("rust"),
            [FieldFilter::new("published", true)],
            &schema,
        )
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 1),
        ]
    );

    // Facet the filter field itself over a keyword-narrowed set.
    let counts = index
        .facet_counts("category", Some("rust"), no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Text("rust".to_owned()), 2),
            (FieldValue::Text("storage".to_owned()), 1),
        ]
    );

    // Filters alone narrow candidates when no keywords are given.
    let counts = index
        .facet_counts(
            "year",
            None,
            [FieldFilter::new("category", "storage")],
            &schema,
        )
        .unwrap();
    assert_eq!(pairs(&counts), [(FieldValue::Integer(2026), 2)]);

    // Keyword AND filter together can leave no candidates.
    let counts = index
        .facet_counts(
            "year",
            Some("unrelated"),
            [FieldFilter::new("published", true)],
            &schema,
        )
        .unwrap();
    assert!(counts.is_empty());
}

#[test]
fn documents_missing_the_target_field_are_not_counted() {
    let index = populated_index();
    let schema = schema();

    // Five documents, but only three carry `published`.
    let total: usize = index
        .facet_counts("published", None, no_filters(), &schema)
        .unwrap()
        .iter()
        .map(FacetCount::count)
        .sum();
    assert_eq!(total, 3);

    // "plain" matches only `p`, which has no title: candidates exist, yet
    // the result is empty because none of them carries the target field.
    let counts = index
        .facet_counts("title", Some("plain"), no_filters(), &schema)
        .unwrap();
    assert!(counts.is_empty());
}

#[test]
fn empty_index_and_absent_values_yield_empty_results() {
    let schema = schema();
    let index = SearchIndex::new();
    assert!(
        index
            .facet_counts("year", None, no_filters(), &schema)
            .unwrap()
            .is_empty()
    );
    assert!(
        index
            .facet_counts("year", Some("anything"), no_filters(), &schema)
            .unwrap()
            .is_empty()
    );

    // A filter that matches nothing leaves no candidates to count.
    let index = populated_index();
    let counts = index
        .facet_counts("year", None, [FieldFilter::new("year", 1999_i64)], &schema)
        .unwrap();
    assert!(counts.is_empty());
}

#[test]
fn unknown_target_and_filter_fields_are_distinct_errors() {
    let index = populated_index();
    let schema = schema();

    assert_eq!(
        index.facet_counts("undeclared", None, no_filters(), &schema),
        Err(FacetError::UnknownField("undeclared".to_owned()))
    );

    // An unknown filter field surfaces through the Filter variant, keeping
    // it distinguishable from an unknown target field.
    let filter = [FieldFilter::new("undeclared", 2026_i64)];
    assert_eq!(
        index.facet_counts("year", None, &filter, &schema),
        Err(FacetError::Filter(FilterError::UnknownField(
            "undeclared".to_owned()
        )))
    );
    assert_eq!(
        index.facet_counts("year", Some("rust"), &filter, &schema),
        Err(FacetError::Filter(FilterError::UnknownField(
            "undeclared".to_owned()
        )))
    );
}

#[test]
fn filter_type_mismatches_are_rejected_before_counting() {
    let index = populated_index();
    let schema = schema();

    let filter = [FieldFilter::new("year", "2026")];
    assert_eq!(
        index.facet_counts("category", None, &filter, &schema),
        Err(FacetError::Filter(FilterError::TypeMismatch {
            field: "year".to_owned(),
            expected: FieldType::Integer,
            actual: FieldType::Text,
        }))
    );

    // The target field is validated first, so its error wins over a bad filter.
    assert_eq!(
        index.facet_counts("undeclared", None, &filter, &schema),
        Err(FacetError::UnknownField("undeclared".to_owned()))
    );
}

#[test]
fn replacement_and_removal_keep_counts_in_sync() {
    let schema = schema();
    let mut index = populated_index();

    // Replacement changes a value: 2026 loses `c`, 2027 gains it.
    index
        .insert_with_schema(
            document("c", "rust storage deep dive")
                .with_field("title", "Storage Deep Dive")
                .with_field("year", 2027_i64)
                .with_field("published", true)
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    let counts = index
        .facet_counts("year", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 3),
            (FieldValue::Integer(2027), 1),
        ]
    );

    // Replacement that drops the target field removes its contribution.
    index
        .insert_with_schema(
            document("c", "rust archive")
                .with_field("title", "Storage Deep Dive")
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    let counts = index
        .facet_counts("year", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 3),
        ]
    );

    // Plain-insert replacement keeps counts in sync as well.
    index.insert(document("p", "plain only").with_field("year", 2030_i64));
    let counts = index
        .facet_counts("year", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(
        pairs(&counts),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 2),
            (FieldValue::Integer(2030), 1),
        ]
    );

    // Removal shrinks counts and empties a value once its last carrier goes.
    index.remove("a");
    index.remove("p");
    let counts = index
        .facet_counts("year", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(pairs(&counts), [(FieldValue::Integer(2026), 2)]);

    index.remove("b");
    index.remove("d");
    assert!(
        index
            .facet_counts("year", None, no_filters(), &schema)
            .unwrap()
            .is_empty()
    );

    // Counts rebuild cleanly after everything was removed.
    index
        .insert_with_schema(
            document("e", "rust storage")
                .with_field("title", "Rebuilt")
                .with_field("year", 2026_i64),
            &schema,
        )
        .unwrap();
    let counts = index
        .facet_counts("year", None, no_filters(), &schema)
        .unwrap();
    assert_eq!(pairs(&counts), [(FieldValue::Integer(2026), 1)]);
}

#[test]
fn legacy_calls_keep_their_semantics_alongside_facets() {
    let schema = schema();
    let mut index = SearchIndex::new();
    assert!(index.is_empty());

    index.insert(document("z", "rust guide"));
    index.insert(document("a", "RUST storage"));
    assert_eq!(index.len(), 2);

    assert_eq!(ids(&index.search("rust")), ["a", "z"]);
    assert!(index.search("").is_empty());
    assert_eq!(index.get("a").unwrap().text(), "RUST storage");
    assert!(index.get("missing").is_none());

    let old = index.insert(document("a", "networking guide")).unwrap();
    assert_eq!(old.text(), "RUST storage");
    assert!(index.search("storage").is_empty());
    assert_eq!(ids(&index.search("rust")), ["z"]);
    assert_eq!(ids(&index.search("guide")), ["a", "z"]);

    assert_eq!(index.remove("z").unwrap().id(), "z");
    assert!(index.remove("z").is_none());
    assert_eq!(index.len(), 1);
    assert_eq!(ids(&index.search("guide")), ["a"]);

    // Filtered search paths are untouched by the new facet entry point.
    let index = populated_index();
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()),
        ["b", "c", "d", "p"]
    );
    let hits = index
        .search_with_fields("rust", [FieldFilter::new("published", true)], &schema)
        .unwrap();
    assert_eq!(ids(&hits), ["a", "c"]);
    assert!(
        index
            .search_with_fields("", no_filters(), &schema)
            .unwrap()
            .is_empty()
    );
}
