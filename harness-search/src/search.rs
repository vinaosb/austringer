//! BM25 search over the code index: definition-boosted ranking with
//! bounded plain-text snippets extracted from the stored file head.

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::snippet::SnippetGenerator;

use crate::error::SearchError;
use crate::index::CodeIndex;

/// Score multiplier applied to the `defs` field so definition sites outrank
/// call sites regardless of raw term frequency.
pub const DEFINITION_BOOST: f32 = 5.0;
/// Snippet length cap in characters for [`SearchHit::snippet`].
pub const SNIPPET_MAX_CHARS: usize = 200;

/// One BM25 search result.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Root-relative path with `/` separators.
    pub path: String,
    /// BM25 score of the hit.
    pub score: f32,
    /// Bounded snippet from the head of the file, query terms included.
    pub snippet: String,
}

fn engine(err: impl std::fmt::Display) -> SearchError {
    SearchError::Engine(err.to_string())
}

impl CodeIndex {
    /// Full-text BM25 search ranked definitions-first.
    ///
    /// The query uses tantivy's grammar (`symbol`, `"exact phrase"`,
    /// `+required -excluded`, `field:term`). Malformed queries return
    /// [`SearchError::InvalidQuery`] instead of panicking.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, SearchError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let searcher = self.reader.searcher();
        let mut parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.defs, self.fields.head, self.fields.content],
        );
        parser.set_field_boost(self.fields.defs, DEFINITION_BOOST);
        let parsed = parser
            .parse_query(query)
            .map_err(|err| SearchError::InvalidQuery(err.to_string()))?;

        let top = searcher
            .search(&parsed, &TopDocs::with_limit(limit).order_by_score())
            .map_err(engine)?;

        let mut generator = SnippetGenerator::create(&searcher, parsed.as_ref(), self.fields.head)
            .map_err(engine)?;
        generator.set_max_num_chars(SNIPPET_MAX_CHARS);

        let mut hits = Vec::with_capacity(top.len());
        for (score, address) in top {
            let doc: tantivy::TantivyDocument = searcher.doc(address).map_err(engine)?;
            let path = doc
                .get_first(self.fields.path)
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let snippet = generator.snippet_from_doc(&doc).fragment().to_string();
            hits.push(SearchHit {
                path,
                score,
                snippet,
            });
        }
        Ok(hits)
    }
}
