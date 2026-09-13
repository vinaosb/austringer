//! Skill packs, loader, routing, project rules, and hot-reload for the
//! agent harness (plan todo 30).
//!
//! Design notes reviewers should know:
//!
//! * **SKILL.md format**: agentskills.io-compatible frontmatter plus
//!   extensions (`triggers` incl. non-English phrases, negative scoping
//!   via `do-not-trigger-on`, `languages`, `version`, `allowed-tools`,
//!   per-model guidance, `context_fork`). Parsing is strict on structure
//!   (typed errors with path+line) and tolerant on content (unknown keys
//!   → typed warnings; missing name/description derive with warnings,
//!   matching how opencode loads bare-markdown skills).
//! * **Zero new dependencies**: the frontmatter scanner is a hand-rolled
//!   YAML *subset* (serde_yaml is archived; the corpus shapes are
//!   scalars/sequences/one-level maps), trigger matching is lexical
//!   containment with ASCII word boundaries (no regex), and the only
//!   wildcard needed (`*` in allowed-tools) is ~15 lines in
//!   [`skill::tool_pattern_matches`]. harness-core's glob engine is
//!   deliberately not a dependency while todo 13 reshapes it.
//! * **Bodies are untrusted data**: no code path interprets a body;
//!   [`loader::SkillIndex`] holds metadata only, and bodies load
//!   exclusively through explicit [`loader::SkillIndex::invoke`] calls
//!   (progressive disclosure).
//! * **Vendor compatibility**: cross-vendor dedup-by-name with precedence
//!   `.harness` > `.opencode` > `.claude` > `.agents` > `.cursor`; malformed
//!   entries are typed errors that never abort a scan; nested monorepo
//!   skills are scoped to their subtree automatically.//! * **Consent**: skills are inert for routing until a
//!   [`consent::ConsentGate`] records consent against a generated
//!   plain-language INSTALL_WARNING (Gemini CLI pattern).

#![deny(missing_docs)]

mod frontmatter;
mod index;
mod parse;
mod scan;
mod triggers;
mod trust;

pub mod consent;
pub mod error;
pub mod forge;
pub mod loader;
pub mod lock;
pub mod migrate;
pub mod pack;
pub mod registry;
pub mod routing;
pub mod rules;
pub mod skill;
pub mod watch;

pub use error::SkillError;
