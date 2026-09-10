//! In-memory document storage with exact token matching.
//!
//! Tokens are separated by Unicode whitespace and converted to lowercase.
//! Every query token must occur in a document. Results use document-key order.
//!
//! Documents may also carry typed fields (text, integer, boolean). A
//! [`Schema`] declares field names, types, and which fields are required;
//! [`SearchIndex::insert_with_schema`] validates a document against the
//! schema before storing it and reports unknown fields, missing required
//! fields, and type mismatches as distinct [`SchemaError`] variants. A failed
//! insert never modifies the index. Plain-text documents inserted with
//! [`SearchIndex::insert`] skip validation entirely.
//!
//! Typed field filtering is offered separately from the query string through
//! [`FieldFilter`] and [`SearchIndex::filter_with_fields`] /
//! [`SearchIndex::search_with_fields`]. Filters perform exact matching against
//! declared schema fields; several filters combine with AND, and a filter can
//! be combined with body keywords. Field values never participate in plain
//! [`SearchIndex::search`].
//!
//! ```
//! use strata_search::{Document, SearchIndex};
//!
//! let mut index = SearchIndex::new();
//! index.insert(Document::new("guide", "Rust storage guide")?);
//! let hits = index.search("RUST guide");
//! assert_eq!(hits[0].id(), "guide");
//! # Ok::<(), strata_search::DocumentError>(())
//! ```
//!
//! ```
//! use strata_search::{Document, FieldSchema, FieldType, Schema, SearchIndex};
//!
//! let mut schema = Schema::new();
//! schema.add_field(FieldSchema::new("title", FieldType::Text, true));
//! schema.add_field(FieldSchema::new("year", FieldType::Integer, false));
//!
//! let mut index = SearchIndex::new();
//! let document = Document::new("guide", "Rust storage guide")?
//!     .with_field("title", "Storage Guide")
//!     .with_field("year", 2026_i64);
//! index.insert_with_schema(document, &schema)?;
//! assert_eq!(index.get("guide").unwrap().field("year").unwrap().field_type(), FieldType::Integer);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ```
//! use strata_search::{Document, FieldFilter, FieldSchema, FieldType, Schema, SearchIndex};
//!
//! let mut schema = Schema::new();
//! schema.add_field(FieldSchema::new("year", FieldType::Integer, false));
//! schema.add_field(FieldSchema::new("published", FieldType::Boolean, false));
//!
//! let mut index = SearchIndex::new();
//! index.insert_with_schema(
//!     Document::new("guide", "Rust storage guide")?
//!         .with_field("year", 2026_i64)
//!         .with_field("published", true),
//!     &schema,
//! )?;
//! index.insert_with_schema(
//!     Document::new("draft", "Rust draft")?
//!         .with_field("year", 2026_i64)
//!         .with_field("published", false),
//!     &schema,
//! )?;
//!
//! let filters = [
//!     FieldFilter::new("year", 2026_i64),
//!     FieldFilter::new("published", true),
//! ];
//! // Body keywords AND field filters, results in key order.
//! let hits = index.search_with_fields("rust", &filters, &schema)?;
//! assert_eq!(hits.iter().map(|doc| doc.id()).collect::<Vec<_>>(), ["guide"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod document;
mod filter;
mod index;
mod schema;

pub use document::{Document, DocumentError};
pub use filter::{FieldFilter, FilterError};
pub use index::SearchIndex;
pub use schema::{FieldSchema, FieldType, FieldValue, Schema, SchemaError};
