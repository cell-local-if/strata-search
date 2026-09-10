//! In-memory document storage with exact token matching.
//!
//! Tokens are separated by Unicode whitespace and converted to lowercase.
//! Every query token must occur in a document. Results use document-key order.
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

mod document;
mod index;

pub use document::{Document, DocumentError};
pub use index::SearchIndex;
