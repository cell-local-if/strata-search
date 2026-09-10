use strata_search::{Document, DocumentError, SearchIndex};

fn document(id: &str, text: &str) -> Document {
    Document::new(id, text).expect("nonblank document key")
}

#[test]
fn rejects_blank_keys_without_rewriting_valid_keys() {
    for key in ["", " ", "\t\n", "\u{2003}"] {
        assert_eq!(Document::new(key, "text"), Err(DocumentError::EmptyId));
    }
    let doc = document(" guide ", "Original\nText");
    assert_eq!(doc.id(), " guide ");
    assert_eq!(doc.text(), "Original\nText");
}

#[test]
fn replacement_updates_search_and_retains_one_document() {
    let mut index = SearchIndex::new();
    assert!(index.insert(document("guide", "old material")).is_none());
    let old = index.insert(document("guide", "new material")).unwrap();
    assert_eq!(old.text(), "old material");
    assert_eq!(index.len(), 1);
    assert!(index.search("old").is_empty());
    assert_eq!(index.search("new")[0].id(), "guide");
    assert_eq!(index.get("guide").unwrap().text(), "new material");
}

#[test]
fn requires_all_query_tokens_and_returns_unique_hits_in_key_order() {
    let mut index = SearchIndex::new();
    index.insert(document("z", "Rust storage storage"));
    index.insert(document("b", "Rust networking"));
    index.insert(document("a", "storage rust"));
    let ids: Vec<_> = index
        .search("RUST storage storage")
        .iter()
        .map(|doc| doc.id())
        .collect();
    assert_eq!(ids, ["a", "z"]);
    assert!(index.search("rust missing").is_empty());
}

#[test]
fn matches_whole_tokens_and_preserves_punctuation() {
    let mut index = SearchIndex::new();
    index.insert(document("a", "network rust,"));
    assert!(index.search("net").is_empty());
    assert!(index.search("rust").is_empty());
    assert_eq!(index.search("RUST,")[0].id(), "a");
}

#[test]
fn uses_unicode_whitespace_and_lowercase_without_normalizing_original_text() {
    let mut index = SearchIndex::new();
    let original = "CAFÉ\u{2003}索引\nGuide";
    index.insert(document("a", original));
    assert_eq!(index.search("café\t索引 GUIDE")[0].id(), "a");
    assert_eq!(index.get("a").unwrap().text(), original);
    assert!(index.search("cafe").is_empty());
}

#[test]
fn empty_queries_and_empty_documents_do_not_match() {
    let mut index = SearchIndex::new();
    assert!(index.is_empty());
    index.insert(document("empty", ""));
    index.insert(document("text", "guide"));
    for query in ["", " \n\t", "\u{2003}"] {
        assert!(index.search(query).is_empty());
    }
    assert_eq!(index.search("guide").len(), 1);
}

#[test]
fn removal_updates_lookup_and_search_without_affecting_other_keys() {
    let mut index = SearchIndex::new();
    index.insert(document("a", "guide"));
    index.insert(document("b", "guide"));
    assert_eq!(index.remove("a").unwrap().id(), "a");
    assert!(index.remove("a").is_none());
    assert!(index.get("a").is_none());
    assert_eq!(index.search("guide")[0].id(), "b");
    assert_eq!(index.len(), 1);
}
