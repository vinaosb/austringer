//! Deferred loading: catalog search, closest-match suggestions, and schema
//! hydration into observations.
//!
//! `tool_search(query)` hydrates the FULL schema of matched tools INTO THE
//! OBSERVATION of the search call itself. The serialized base schema (the
//! system-prefix tool section) is never touched — hydration is a tool
//! result, not a prompt mutation.

use super::provider::{ToolDispatchError, ToolSearchError};
use super::registry::ToolRegistry;
use super::schema::ToolSchema;

/// Renders the hydrated observation for a successful search: the complete
/// schemas of every match, compact and byte-stable (sorted keys by
/// construction; no timestamps).
#[must_use]
pub fn hydrate_schemas(matches: &[ToolSchema]) -> String {
    // Infallible by type: ToolSchema is two strings and a Value.
    let body = serde_json::to_string(matches).unwrap_or_default();
    format!("hydrated tool schemas ({}):\n{body}", matches.len())
}

/// Renders the graceful observation for a missed search: the reason, the
/// closest matches, and the aux table for browsing. Never an error string
/// alone — the model always receives actionable steering.
#[must_use]
pub fn render_suggestion(error: &ToolSearchError) -> String {
    let suggestions = if error.suggestions.is_empty() {
        "(none)".to_owned()
    } else {
        error.suggestions.join(", ")
    };
    format!(
        "tool_search miss: {}\nquery: {:?}\nclosest matches: {suggestions}\ndeferred tool catalog (call tool_search with a name to hydrate its full schema):\n{}",
        error.reason, error.query, error.aux_table
    )
}

/// Ranks catalog entries for a non-empty query: exact name matches first,
/// then name or description substring matches; name-sorted within each
/// tier (deterministic).
#[must_use]
pub fn select_matches(catalog: &[ToolSchema], query: &str) -> Vec<ToolSchema> {
    let needle = query.to_lowercase();
    let mut exact = Vec::new();
    let mut partial = Vec::new();
    for schema in catalog {
        let name_hit = schema.name.to_lowercase().contains(&needle);
        if schema.name.eq_ignore_ascii_case(query) {
            exact.push(schema.clone());
        } else if name_hit || schema.description.to_lowercase().contains(&needle) {
            partial.push(schema.clone());
        }
    }
    exact.extend(partial);
    exact
}

/// Closest catalog names for a miss: at most three, ordered by edit
/// distance then name, and only within half the longer length (a garbage
/// query earns no suggestions rather than noise).
#[must_use]
pub fn closest_names(names: &[String], query: &str) -> Vec<String> {
    let query_chars: Vec<char> = query.chars().collect();
    let mut scored: Vec<(usize, &String)> = names
        .iter()
        .map(|name| {
            let name_chars: Vec<char> = name.chars().collect();
            (levenshtein(&query_chars, &name_chars), name)
        })
        .filter(|(distance, name)| {
            let longer = query_chars.len().max(name.chars().count());
            *distance * 2 <= longer.max(1)
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(3)
        .map(|(_, name)| name.clone())
        .collect()
}

/// Two-row Levenshtein edit distance over chars (unicode-safe).
fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (index, a_char) in a.iter().enumerate() {
        current[0] = index + 1;
        for (position, b_char) in b.iter().enumerate() {
            let substitution = usize::from(a_char != b_char);
            current[position + 1] = (previous[position + 1] + 1)
                .min(current[position] + 1)
                .min(previous[position] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

impl ToolRegistry {
    /// Searches the pinned catalog. Matches return the full schemas; a
    /// miss returns a typed error carrying closest matches and the aux
    /// table. (Impl split of [`ToolRegistry`]: deferred loading lives
    /// here, next to the matching algorithms.)
    pub fn search(
        &self,
        pinned: &super::version::PinnedToolSet,
        query: &str,
    ) -> Result<Vec<ToolSchema>, ToolSearchError> {
        if query.is_empty() {
            return Err(ToolSearchError {
                query: String::new(),
                reason: "query must not be empty".to_owned(),
                suggestions: Vec::new(),
                aux_table: self.aux_table(pinned),
            });
        }
        let catalog: Vec<ToolSchema> = pinned
            .tool_names()
            .filter_map(|name| self.pinned_schema(pinned, name).cloned())
            .collect();
        let matches = select_matches(&catalog, query);
        if matches.is_empty() {
            let names: Vec<String> = catalog.iter().map(|schema| schema.name.clone()).collect();
            return Err(ToolSearchError {
                query: query.to_owned(),
                reason: format!("no tool matched {query:?}"),
                suggestions: closest_names(&names, query),
                aux_table: self.aux_table(pinned),
            });
        }
        Ok(matches)
    }

    /// The internal deferred-loading path: hydrates matched schemas into
    /// this call's own result bytes (never the system prefix), or renders
    /// the graceful suggestion for a miss.
    pub(super) fn run_tool_search(
        &self,
        pinned: &super::version::PinnedToolSet,
        arguments: &serde_json::Value,
    ) -> Result<Vec<u8>, ToolDispatchError> {
        let query = arguments
            .get("query")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ToolDispatchError::InvalidArguments {
                tool: super::schema::TOOL_SEARCH.to_owned(),
                message: "arguments must be an object with a string 'query' field".to_owned(),
            })?;
        let observation = match self.search(pinned, query) {
            Ok(matches) => hydrate_schemas(&matches),
            Err(error) => render_suggestion(&error),
        };
        Ok(observation.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_matches_known_distances() {
        fn distance(a: &str, b: &str) -> usize {
            let a_chars: Vec<char> = a.chars().collect();
            let b_chars: Vec<char> = b.chars().collect();
            levenshtein(&a_chars, &b_chars)
        }
        assert_eq!(distance("edit_lines", "edit_lines"), 0);
        assert_eq!(distance("edit_liness", "edit_lines"), 1);
        assert_eq!(distance("view_wind", "view_window"), 2);
        assert_eq!(distance("", "abc"), 3);
        assert_eq!(distance("abc", ""), 3);
    }
}
