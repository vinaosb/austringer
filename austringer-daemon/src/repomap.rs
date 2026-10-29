//! Session system-prompt composition: the frozen base prompt plus the
//! once-per-session repository map (austringer-repomap wired
//! load-bearing for the first time).

use std::path::Path;

use austringer_ast::analyze::{AnalyzeOutcome, Analyzer};
use austringer_ast::tags::TagTable;
use austringer_ast::walk::{WalkLimits, walk_workspace};

/// Default token budget of the session repomap block.
pub const DEFAULT_REPOMAP_TOKEN_BUDGET: u32 = 1000;

/// Compose the session system prompt: [`SESSION_SYSTEM_PROMPT`](crate::engine::SESSION_SYSTEM_PROMPT)
/// plus a `<repomap>` block when `token_budget` > 0 and the map builds.
///
/// The block is composed ONCE, before engine spawn, and rides the
/// frozen prompt prefix for the session's lifetime — recomposing
/// mid-session would invalidate the provider prefix cache (research
/// §24.6/§32, the same byte-stability contract the base prompt is
/// frozen under). The map itself is bounded hydration — the largest
/// render fitting the token budget, never the whole repo injected —
/// mirroring austringer-intel `routing/search.rs`'s typed-cap pattern.
/// Budget 0, an unbuildable map, or an empty map degrades to the plain
/// prompt (walk failures notice through tracing): session spawn never
/// fails because of the map.
#[must_use]
pub fn compose_system_prompt(workspace_root: &Path, cache_dir: &Path, token_budget: u32) -> String {
    let base = crate::engine::SESSION_SYSTEM_PROMPT;
    if token_budget == 0 {
        return base.to_owned();
    }
    let text = map_text(workspace_root, cache_dir, token_budget);
    if text.is_empty() {
        return base.to_owned();
    }
    format!("{base}\n\n<repomap>\n{text}\n</repomap>")
}

/// The bounded map text for a workspace: the largest render fitting
/// `token_budget`; budget 0, walk failure, or an empty map yield the
/// empty string. Shared by [`compose_system_prompt`] (the byte
/// contract) and [`crate::prompt`] (the builder-routed live path).
pub(crate) fn map_text(workspace_root: &Path, cache_dir: &Path, token_budget: u32) -> String {
    if token_budget == 0 {
        return String::new();
    }
    let files = match walk_workspace(workspace_root, &WalkLimits::default()) {
        Ok(files) => files,
        Err(error) => {
            tracing::warn!(
                root = %workspace_root.display(),
                %error,
                "repomap: workspace walk failed; session spawns with the plain system prompt"
            );
            return String::new();
        }
    };
    let analyzer = Analyzer::with_cache_dir(cache_dir);
    let mut tables = Vec::with_capacity(files.len());
    for relative in &files {
        match analyzer.analyze_path(&workspace_root.join(relative)) {
            Ok(AnalyzeOutcome::Analyzed(analysis)) => tables.push((
                relative.clone(),
                TagTable {
                    definitions: analysis.definitions,
                    references: analysis.references,
                    truncated: analysis.tags_truncated,
                },
            )),
            // Per-file failures skip that file: the map is a bounded
            // approximation by contract, not a mirror of the tree.
            Ok(AnalyzeOutcome::NoGrammar { .. }) | Err(_) => {}
        }
    }
    let options = austringer_repomap::MapOptions {
        token_budget,
        ..austringer_repomap::MapOptions::default()
    };
    austringer_repomap::repo_map(tables, &options).text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_budget_returns_the_plain_prompt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let prompt = compose_system_prompt(dir.path(), dir.path(), 0);
        assert_eq!(prompt, crate::engine::SESSION_SYSTEM_PROMPT);
    }

    #[test]
    fn unbuildable_map_degrades_to_the_plain_prompt() {
        let cache = tempfile::tempdir().expect("tempdir");
        let prompt = compose_system_prompt(Path::new("no-such-root-zq"), cache.path(), 1000);
        assert_eq!(prompt, crate::engine::SESSION_SYSTEM_PROMPT);
    }

    #[test]
    fn empty_workspace_returns_the_plain_prompt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = tempfile::tempdir().expect("tempdir");
        let prompt = compose_system_prompt(dir.path(), cache.path(), 1000);
        assert_eq!(prompt, crate::engine::SESSION_SYSTEM_PROMPT);
    }
}
