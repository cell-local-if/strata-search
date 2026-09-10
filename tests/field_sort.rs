use strata_search::{
    Document, FieldFilter, FieldSchema, FieldType, FieldValue, FilterError, Schema, SearchIndex,
    SortDirection, SortError,
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

/// Same corpus as the facet tests: every field type is represented, `d`
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
fn sorts_each_field_type_ascending_and_descending() {
    let index = populated_index();
    let schema = schema();

    // Integer: 2025 first ascending, last descending; the four 2026
    // documents keep key order in both directions.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "b", "c", "d", "p"]);
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Descending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["b", "c", "d", "p", "a"]);

    // Text: "rust" before "storage" ascending, reversed descending; `p`
    // has no category and stays last either way.
    let hits = index
        .search_sorted(
            "category",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "b", "c", "d", "p"]);
    let hits = index
        .search_sorted(
            "category",
            SortDirection::Descending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["c", "d", "a", "b", "p"]);

    // Boolean: false before true ascending, reversed descending; `d` and
    // `p` lack `published` and stay last either way.
    let hits = index
        .search_sorted(
            "published",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["b", "a", "c", "d", "p"]);
    let hits = index
        .search_sorted(
            "published",
            SortDirection::Descending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "c", "b", "d", "p"]);
}

#[test]
fn keywords_and_filters_narrow_candidates_before_sorting() {
    let index = populated_index();
    let schema = schema();

    // Body "rust" (a, b, c) AND published = true (a, c), sorted by year.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            Some("rust"),
            [FieldFilter::new("published", true)],
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "c"]);

    // Same candidates descending: equal years keep key order.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Descending,
            Some("rust storage"),
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["c", "a"]);

    // Filters alone narrow candidates when no keywords are given.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            [FieldFilter::new("category", "storage")],
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["c", "d"]);

    // A query that tokenizes to nothing applies no keyword constraint.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            Some("  \t "),
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "b", "c", "d", "p"]);

    // Keywords and filters together can leave no candidates.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            Some("unrelated"),
            [FieldFilter::new("published", true)],
            &schema,
        )
        .unwrap();
    assert!(hits.is_empty());
}

#[test]
fn documents_missing_the_sort_field_come_last_in_key_order() {
    let index = populated_index();
    let schema = schema();

    // `d` and `p` lack `published`: they trail the valued documents in
    // both directions, always in ascending key order.
    for direction in [SortDirection::Ascending, SortDirection::Descending] {
        let hits = index
            .search_sorted("published", direction, None, no_filters(), &schema)
            .unwrap();
        assert_eq!(&ids(&hits)[3..], ["d", "p"]);
    }

    // Sorting by `title`, which only `p` lacks. Descending: "Storage Deep
    // Dive" > "Networking Book" > "Miscellaneous" > "Guide One".
    let hits = index
        .search_sorted(
            "title",
            SortDirection::Descending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["c", "b", "d", "a", "p"]);

    // Candidates exist but none carries the sort field: key order only.
    let hits = index
        .search_sorted(
            "title",
            SortDirection::Ascending,
            Some("plain"),
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["p"]);
}

#[test]
fn equal_values_keep_key_order_even_descending() {
    let schema = schema();
    let mut index = SearchIndex::new();
    // Insert in an order that would expose any instability.
    for id in ["m", "b", "k", "a"] {
        index
            .insert_with_schema(
                document(id, "rust")
                    .with_field("title", "T")
                    .with_field("year", 2026_i64),
                &schema,
            )
            .unwrap();
    }

    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "b", "k", "m"]);

    // Descending reverses the value order only; with one shared value the
    // key order is identical to ascending.
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Descending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "b", "k", "m"]);
}

#[test]
fn empty_index_and_absent_values_yield_empty_or_key_ordered_results() {
    let schema = schema();
    let index = SearchIndex::new();
    assert!(
        index
            .search_sorted(
                "year",
                SortDirection::Ascending,
                None,
                no_filters(),
                &schema
            )
            .unwrap()
            .is_empty()
    );
    assert!(
        index
            .search_sorted(
                "year",
                SortDirection::Descending,
                Some("anything"),
                no_filters(),
                &schema,
            )
            .unwrap()
            .is_empty()
    );

    // A filter that matches nothing leaves no candidates to sort.
    let index = populated_index();
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            [FieldFilter::new("year", 1999_i64)],
            &schema,
        )
        .unwrap();
    assert!(hits.is_empty());
}

#[test]
fn unknown_sort_field_and_filter_problems_are_distinct_errors() {
    let index = populated_index();
    let schema = schema();

    assert_eq!(
        index.search_sorted(
            "undeclared",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema
        ),
        Err(SortError::UnknownField("undeclared".to_owned()))
    );

    // An unknown filter field surfaces through the Filter variant, keeping
    // it distinguishable from an unknown sort field.
    let filter = [FieldFilter::new("undeclared", 2026_i64)];
    assert_eq!(
        index.search_sorted("year", SortDirection::Ascending, None, &filter, &schema),
        Err(SortError::Filter(FilterError::UnknownField(
            "undeclared".to_owned()
        )))
    );
    assert_eq!(
        index.search_sorted(
            "year",
            SortDirection::Descending,
            Some("rust"),
            &filter,
            &schema
        ),
        Err(SortError::Filter(FilterError::UnknownField(
            "undeclared".to_owned()
        )))
    );

    // A filter value whose type disagrees with the schema is its own error.
    let filter = [FieldFilter::new("year", "2026")];
    assert_eq!(
        index.search_sorted("category", SortDirection::Ascending, None, &filter, &schema),
        Err(SortError::Filter(FilterError::TypeMismatch {
            field: "year".to_owned(),
            expected: FieldType::Integer,
            actual: FieldType::Text,
        }))
    );

    // The sort field is validated first, so its error wins over a bad filter.
    assert_eq!(
        index.search_sorted(
            "undeclared",
            SortDirection::Ascending,
            None,
            &filter,
            &schema
        ),
        Err(SortError::UnknownField("undeclared".to_owned()))
    );
}

#[test]
fn replacement_and_removal_keep_sorting_in_sync() {
    let schema = schema();
    let mut index = populated_index();

    // Replacement changes a value: `c` moves from the 2026 group to the front.
    index
        .insert_with_schema(
            document("c", "rust storage deep dive")
                .with_field("title", "Storage Deep Dive")
                .with_field("year", 2024_i64)
                .with_field("published", true)
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["c", "a", "b", "d", "p"]);

    // Replacement that drops the sort field moves the document to the
    // missing-field tail.
    index
        .insert_with_schema(
            document("c", "rust archive")
                .with_field("title", "Storage Deep Dive")
                .with_field("category", "storage"),
            &schema,
        )
        .unwrap();
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["a", "b", "d", "p", "c"]);

    // Plain-insert replacement keeps sorting in sync as well.
    index.insert(document("p", "plain only").with_field("year", 2024_i64));
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["p", "a", "b", "d", "c"]);

    // Removal drops the document from every position.
    index.remove("a");
    index.remove("c");
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Descending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["b", "d", "p"]);

    // Sorting still works after everything was removed and rebuilt.
    index.remove("b");
    index.remove("d");
    index.remove("p");
    index
        .insert_with_schema(
            document("e", "rust storage")
                .with_field("title", "Rebuilt")
                .with_field("year", 2026_i64),
            &schema,
        )
        .unwrap();
    let hits = index
        .search_sorted(
            "year",
            SortDirection::Ascending,
            None,
            no_filters(),
            &schema,
        )
        .unwrap();
    assert_eq!(ids(&hits), ["e"]);
}

#[test]
fn legacy_calls_keep_their_semantics_alongside_sorting() {
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

    // Filtered search and facet paths are untouched by the new sort entry
    // point.
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

    let counts = index
        .facet_counts("year", Some("rust"), no_filters(), &schema)
        .unwrap();
    assert_eq!(
        counts
            .iter()
            .map(|entry| (entry.value().clone(), entry.count()))
            .collect::<Vec<_>>(),
        [
            (FieldValue::Integer(2025), 1),
            (FieldValue::Integer(2026), 2)
        ]
    );
}
