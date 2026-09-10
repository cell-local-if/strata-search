use strata_search::{
    Document, FieldFilter, FieldSchema, FieldType, FilterError, Schema, SearchIndex,
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

/// Five documents exercising every field type, including a document missing
/// the optional `published` field and one inserted through the schema-free
/// `insert` path.
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
    // Plain inserts skip schema validation but must still feed filter data.
    index.insert(document("p", "plain only").with_field("year", 2026_i64));
    index
}

#[test]
fn single_field_filters_match_each_value_type_in_key_order() {
    let index = populated_index();
    let schema = schema();

    let text = [FieldFilter::new("title", "Networking Book")];
    assert_eq!(
        ids(&index.filter_with_fields(&text, &schema).unwrap()),
        ["b"]
    );

    let integer = [FieldFilter::new("year", 2026_i64)];
    assert_eq!(
        ids(&index.filter_with_fields(&integer, &schema).unwrap()),
        ["b", "c", "d", "p"]
    );
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2025_i64)], &schema)
            .unwrap()),
        ["a"]
    );

    let boolean_true = [FieldFilter::new("published", true)];
    assert_eq!(
        ids(&index.filter_with_fields(&boolean_true, &schema).unwrap()),
        ["a", "c"]
    );
    let boolean_false = [FieldFilter::new("published", false)];
    assert_eq!(
        ids(&index.filter_with_fields(&boolean_false, &schema).unwrap()),
        ["b"]
    );
}

#[test]
fn text_field_filters_are_exact_case_sensitive_value_matches() {
    let index = populated_index();
    let schema = schema();

    for wrong in ["networking book", "Networking", "Book", "networking"] {
        let filter = [FieldFilter::new("title", wrong)];
        assert!(
            index
                .filter_with_fields(&filter, &schema)
                .unwrap()
                .is_empty(),
            "title filter {wrong:?} should not match"
        );
    }
}

#[test]
fn multiple_filters_are_combined_with_and() {
    let index = populated_index();
    let schema = schema();

    let filters = [
        FieldFilter::new("year", 2026_i64),
        FieldFilter::new("published", true),
    ];
    assert_eq!(
        ids(&index.filter_with_fields(&filters, &schema).unwrap()),
        ["c"]
    );

    let filters = [
        FieldFilter::new("year", 2026_i64),
        FieldFilter::new("category", "rust"),
    ];
    assert_eq!(
        ids(&index.filter_with_fields(&filters, &schema).unwrap()),
        ["b"]
    );

    // A filter matching nothing empties the whole intersection.
    let filters = [
        FieldFilter::new("published", true),
        FieldFilter::new("title", "No Such Title"),
    ];
    assert_eq!(
        ids(&index.filter_with_fields(&filters, &schema).unwrap()),
        Vec::<&str>::new()
    );
}

#[test]
fn body_keywords_are_anded_with_field_filters() {
    let index = populated_index();
    let schema = schema();

    let hits = index
        .search_with_fields("rust", [FieldFilter::new("published", true)], &schema)
        .unwrap();
    assert_eq!(ids(&hits), ["a", "c"]);

    let hits = index
        .search_with_fields(
            "storage rust",
            [FieldFilter::new("year", 2026_i64)],
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["c"]);

    // Body match alone is not enough when the filtered field is absent.
    let hits = index
        .search_with_fields("unrelated", [FieldFilter::new("published", true)], &schema)
        .unwrap();
    assert_eq!(ids(&hits), Vec::<&str>::new());

    // Every body token must still be present.
    let hits = index
        .search_with_fields(
            "rust missing",
            [FieldFilter::new("year", 2026_i64)],
            &schema,
        )
        .unwrap();
    assert!(hits.is_empty());
}

#[test]
fn documents_missing_a_filtered_field_do_not_match() {
    let index = populated_index();
    let schema = schema();

    // `d` and `p` carry no `published` value and must be excluded.
    let hits = index
        .filter_with_fields([FieldFilter::new("published", false)], &schema)
        .unwrap();
    assert_eq!(ids(&hits), ["b"]);

    // `p` has no title at all.
    let hits = index
        .search_with_fields(
            "plain",
            [FieldFilter::new("title", "Miscellaneous")],
            &schema,
        )
        .unwrap();
    assert!(hits.is_empty());
}

#[test]
fn no_matching_values_returns_empty_vec() {
    let index = populated_index();
    let schema = schema();

    let none: Vec<&str> = Vec::new();
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 1999_i64)], &schema)
            .unwrap()),
        none
    );
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("category", "music")], &schema)
            .unwrap()),
        none
    );
}

#[test]
fn empty_filter_lists_match_every_document_and_compose_with_search() {
    let index = populated_index();
    let schema = schema();

    let no_filters: Vec<FieldFilter> = Vec::new();
    assert_eq!(
        ids(&index.filter_with_fields(&no_filters, &schema).unwrap()),
        ["a", "b", "c", "d", "p"]
    );

    let hits = index
        .search_with_fields("rust", std::iter::empty::<&FieldFilter>(), &schema)
        .unwrap();
    assert_eq!(ids(&hits), ids(&index.search("rust")));
    assert_eq!(ids(&hits), ["a", "b", "c"]);

    // The empty-query rule still applies even when no filter narrows things.
    for query in ["", "  \n\t", "\u{2003}"] {
        let hits = index
            .search_with_fields(query, std::iter::empty::<&FieldFilter>(), &schema)
            .unwrap();
        assert!(hits.is_empty(), "query {query:?} matched");
    }
}

#[test]
fn unknown_filter_fields_are_rejected_distinctly() {
    let index = populated_index();
    let schema = schema();

    let filter = [FieldFilter::new("undeclared", 2026_i64)];
    assert_eq!(
        index.filter_with_fields(&filter, &schema),
        Err(FilterError::UnknownField("undeclared".to_owned()))
    );
    assert_eq!(
        index.search_with_fields("rust", &filter, &schema),
        Err(FilterError::UnknownField("undeclared".to_owned()))
    );

    // The failed query changes nothing and later queries still work.
    assert_eq!(index.len(), 5);
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()),
        ["b", "c", "d", "p"]
    );
}

#[test]
fn filter_value_type_mismatches_are_rejected_with_expected_and_actual() {
    let index = populated_index();
    let schema = schema();

    let cases = [
        (
            FieldFilter::new("title", 42_i64),
            FieldType::Text,
            FieldType::Integer,
        ),
        (
            FieldFilter::new("year", "2026"),
            FieldType::Integer,
            FieldType::Text,
        ),
        (
            FieldFilter::new("published", 1_i64),
            FieldType::Boolean,
            FieldType::Integer,
        ),
    ];
    for (filter, expected, actual) in cases {
        let error = FilterError::TypeMismatch {
            field: filter.name().to_owned(),
            expected,
            actual,
        };
        assert_eq!(
            index.filter_with_fields([&filter], &schema),
            Err(error.clone())
        );
        assert_eq!(
            index.search_with_fields("rust", [&filter], &schema),
            Err(error)
        );
    }

    // A bad filter later in the list is rejected as well.
    let filters = [
        FieldFilter::new("year", 2026_i64),
        FieldFilter::new("published", "yes"),
    ];
    assert_eq!(
        index.filter_with_fields(&filters, &schema),
        Err(FilterError::TypeMismatch {
            field: "published".to_owned(),
            expected: FieldType::Boolean,
            actual: FieldType::Text,
        })
    );
}

#[test]
fn field_values_never_participate_in_plain_search() {
    let index = populated_index();

    // Tokens that occur only inside field values never match.
    for query in [
        "2026",
        "2025",
        "true",
        "one",
        "book",
        "miscellaneous",
        "Miscellaneous",
    ] {
        assert!(index.search(query).is_empty(), "query {query:?} matched");
    }

    // `storage` appears in the bodies of `a`/`c` and only as a field value on
    // `d`, so the field value must not add `d`.
    assert_eq!(ids(&index.search("storage")), ["a", "c"]);
    assert_eq!(ids(&index.search("RUST")), ["a", "b", "c"]);
    // `guide` is in `a`'s body and only in a field value elsewhere.
    assert_eq!(ids(&index.search("guide")), ["a"]);
}

#[test]
fn replacement_keeps_field_filters_in_sync() {
    let schema = schema();
    let mut index = populated_index();

    // Schema-validated replacement changes a value.
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
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()),
        ["b", "d", "p"]
    );
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2027_i64)], &schema)
            .unwrap()),
        ["c"]
    );

    // A replacement that drops a field removes the key from that posting.
    index
        .insert_with_schema(
            document("c", "rust archive")
                .with_field("title", "Storage Deep Dive")
                .with_field("year", 2027_i64)
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("published", true)], &schema)
            .unwrap()),
        ["a"]
    );
    // Body + field combination observes the replacement text.
    let hits = index
        .search_with_fields("archive", [FieldFilter::new("year", 2027_i64)], &schema)
        .unwrap();
    assert_eq!(ids(&hits), ["c"]);
    assert!(
        index
            .search_with_fields("storage", [FieldFilter::new("year", 2027_i64)], &schema)
            .unwrap()
            .is_empty()
    );

    // Plain-insert replacement keeps postings in sync as well.
    index.insert(document("p", "plain only").with_field("year", 2030_i64));
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()),
        ["b", "d"]
    );
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2030_i64)], &schema)
            .unwrap()),
        ["p"]
    );
}

#[test]
fn failed_schema_replacement_leaves_filter_results_untouched() {
    let schema = schema();
    let mut index = populated_index();

    for invalid in [
        // Unknown field.
        document("c", "new body")
            .with_field("title", "Storage Deep Dive")
            .with_field("surprise", 1_i64),
        // Missing required title.
        document("c", "new body").with_field("year", 2030_i64),
        // Wrong type.
        document("c", "new body")
            .with_field("title", "Storage Deep Dive")
            .with_field("year", false),
    ] {
        assert!(index.insert_with_schema(invalid, &schema).is_err());
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
    }
}

#[test]
fn removal_keeps_field_filters_in_sync_and_postings_rebuild() {
    let schema = schema();
    let mut index = populated_index();

    assert_eq!(index.remove("c").unwrap().id(), "c");
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()),
        ["b", "d", "p"]
    );
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("published", true)], &schema)
            .unwrap()),
        ["a"]
    );
    let hits = index
        .search_with_fields("rust", [FieldFilter::new("category", "storage")], &schema)
        .unwrap();
    assert!(hits.is_empty());

    index.remove("b");
    index.remove("d");
    index.remove("p");
    // The value posting is empty now and behaves like any other unmatched value.
    assert!(
        index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()
            .is_empty()
    );

    // Postings rebuild cleanly after the old keys and values were cleaned out.
    index
        .insert_with_schema(
            document("e", "rust storage")
                .with_field("title", "Rebuilt")
                .with_field("year", 2026_i64),
            &schema,
        )
        .unwrap();
    assert_eq!(
        ids(&index
            .filter_with_fields([FieldFilter::new("year", 2026_i64)], &schema)
            .unwrap()),
        ["e"]
    );
}

#[test]
fn legacy_calls_keep_their_semantics_alongside_filters() {
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
    // `a` dropped the only "storage" token, but `z` still carries "rust".
    assert!(index.search("storage").is_empty());
    assert_eq!(ids(&index.search("rust")), ["z"]);
    assert_eq!(ids(&index.search("guide")), ["a", "z"]);

    assert_eq!(index.remove("z").unwrap().id(), "z");
    assert!(index.remove("z").is_none());
    assert_eq!(index.len(), 1);
    assert_eq!(ids(&index.search("guide")), ["a"]);
}
