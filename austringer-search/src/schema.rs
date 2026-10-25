//! Tantivy schema and field handles for the code index.
//!
//! Layout: `path` is the exact document key (delete-by-term), `mtime`/`size`
//! drive incremental refresh, `content` carries the full BM25 text, `head`
//! is a stored copy of the file head for snippet generation, and `defs`
//! holds lexical definition symbols so definition sites can be boosted.

use tantivy::schema::{Field, NumericOptions, STORED, STRING, Schema, TEXT};

use crate::error::SearchError;

pub(crate) struct Fields {
    pub(crate) path: Field,
    pub(crate) mtime: Field,
    pub(crate) size: Field,
    pub(crate) content: Field,
    pub(crate) head: Field,
    pub(crate) defs: Field,
}

impl Fields {
    pub(crate) fn from_schema(schema: &Schema) -> Result<Self, SearchError> {
        let fields = || -> Option<Self> {
            Some(Self {
                path: schema.get_field("path").ok()?,
                mtime: schema.get_field("mtime").ok()?,
                size: schema.get_field("size").ok()?,
                content: schema.get_field("content").ok()?,
                head: schema.get_field("head").ok()?,
                defs: schema.get_field("defs").ok()?,
            })
        };
        fields().ok_or_else(|| SearchError::Engine("schema field lookup failed".into()))
    }
}

pub(crate) fn schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("path", STRING | STORED);
    builder.add_i64_field("mtime", NumericOptions::default().set_stored());
    builder.add_u64_field("size", NumericOptions::default().set_stored());
    builder.add_text_field("content", TEXT);
    builder.add_text_field("head", TEXT | STORED);
    builder.add_text_field("defs", TEXT);
    builder.build()
}

/// `head`-window slice capped at `max_bytes` on a UTF-8 char boundary.
pub(crate) fn head_of(content: &str, max_bytes: usize) -> &str {
    if content.len() <= max_bytes {
        return content;
    }
    let mut end = max_bytes;
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    &content[..end]
}
