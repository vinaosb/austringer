//! File graph construction: `(path, TagTable)` pairs → name-keyed
//! cross-references → weighted undirected CSR adjacency.
//!
//! # Refs-by-name (forge Lattice §26)
//!
//! The cross-reference index [`FileGraph::refs_by_name`] is keyed by
//! symbol NAME and stores file PATHS — never graph node ids. Node ids
//! exist only inside the derived CSR and never outlive a build, so file
//! renames or node renumbering cannot dangle an edge: rebuilding from
//! the (renamed) tag tables re-joins def sites to references purely by
//! name. Locked by test `rebuild_after_rename_and_reorder_edges_survive_via_name_keys`.
//!
//! # Edge + resolution rules
//!
//! For each symbol name: every referencing file links to EVERY file
//! defining that name (def-file ↔ ref-file, undirected), weighted by
//! the referencing file's reference count. Multiple def sites are NOT
//! disambiguated to one winner: a re-exported symbol genuinely belongs
//! to all its def sites, splitting weights would arbitrarily halve
//! multi-def symbols, and directory-proximity tie-breaks break on
//! re-exports and moved modules. This mirrors the proven aider map
//! behavior and stays fully deterministic.
//!
//! Skipped, counted in [`GraphStats`] (bounded, typed handling — no
//! panics on malformed tables): empty symbol names, intra-file
//! self-references, duplicate paths (first table wins), references to
//! names with no def site in the repo (external symbols — kept out of
//! the index entirely), and names whose references never leave their
//! defining file.

use crate::pagerank::Csr;
use austringer_ast::tags::{SymbolKind, TagTable};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A renderable definition: name + kind (signature text is not carried
/// by tag tables today; the renderer emits `name:kind`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDef {
    /// Lossy-UTF-8 symbol name.
    pub name: String,
    /// What kind of symbol.
    pub kind: SymbolKind,
}

/// One definition site of a cross-referenced symbol, identified by PATH
/// (stable across reindexing — see module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefSite {
    /// File containing the definition.
    pub file: PathBuf,
    /// Kind recorded at the (first) definition in that file.
    pub kind: SymbolKind,
}

/// Cross-references for one symbol name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolLinks {
    /// Files defining the name, sorted by path.
    pub def_sites: Vec<DefSite>,
    /// Referencing file → reference count.
    pub refs: BTreeMap<PathBuf, u64>,
}

/// Build counters surfaced for diagnostics and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphStats {
    /// Distinct files in the graph.
    pub files: usize,
    /// Symbol names kept in the cross-reference index.
    pub symbols: usize,
    /// Undirected edges in the CSR.
    pub edges: usize,
    /// Definitions + references skipped for empty symbol names.
    pub skipped_empty_names: u64,
    /// Tables dropped because their path was already ingested.
    pub duplicate_paths: u64,
}

/// The weighted file graph. Build via [`FileGraph::build`].
#[derive(Debug, Clone, PartialEq)]
pub struct FileGraph {
    files: Vec<PathBuf>,
    file_ids: BTreeMap<PathBuf, u32>,
    definitions: Vec<Vec<SymbolDef>>,
    refs_by_name: BTreeMap<String, SymbolLinks>,
    csr: Csr,
    stats: GraphStats,
}

/// Tree-sitter suffix form of a kind (`function`, `enumMember`, ...)
/// — the inverse of `SymbolKind::from_suffix`, used as the stable
/// rendering/dedup label for every variant including `Other`.
pub fn kind_label(kind: &SymbolKind) -> &str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Macro => "macro",
        SymbolKind::Struct => "struct",
        SymbolKind::Class => "class",
        SymbolKind::Enum => "enum",
        SymbolKind::EnumMember => "enumMember",
        SymbolKind::Interface => "interface",
        SymbolKind::Module => "module",
        SymbolKind::Type => "type",
        SymbolKind::Constant => "constant",
        SymbolKind::Variable => "variable",
        SymbolKind::Property => "property",
        SymbolKind::Section => "section",
        SymbolKind::Call => "call",
        SymbolKind::Other(label) => label,
        // SymbolKind is non_exhaustive; future kinds get a stable label.
        _ => "other",
    }
}

impl FileGraph {
    /// Build the graph from per-file tag tables. Files are sorted by
    /// path (stable node ids regardless of ingestion order); a path
    /// ingested twice keeps its FIRST table and counts a collision.
    pub fn build(tables: impl IntoIterator<Item = (PathBuf, TagTable)>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut per_file: Vec<(PathBuf, Vec<SymbolDef>)> = Vec::new();
        // name -> def file -> kind-at-first-def ; name -> ref file -> count
        let mut def_kinds: BTreeMap<String, BTreeMap<PathBuf, SymbolKind>> = BTreeMap::new();
        let mut ref_counts: BTreeMap<String, BTreeMap<PathBuf, u64>> = BTreeMap::new();
        let mut stats = GraphStats::default();

        for (path, table) in tables {
            if !seen.insert(path.clone()) {
                stats.duplicate_paths += 1;
                continue;
            }
            let mut file_defs = Vec::with_capacity(table.definitions.len());
            for definition in &table.definitions {
                if definition.name.is_empty() {
                    stats.skipped_empty_names += 1;
                    continue;
                }
                file_defs.push(SymbolDef {
                    name: definition.name.clone(),
                    kind: definition.kind.clone(),
                });
                def_kinds
                    .entry(definition.name.clone())
                    .or_default()
                    .entry(path.clone())
                    .or_insert_with(|| definition.kind.clone());
            }
            file_defs.sort_by(|a, b| {
                a.name
                    .cmp(&b.name)
                    .then_with(|| kind_label(&a.kind).cmp(kind_label(&b.kind)))
            });
            file_defs
                .dedup_by(|a, b| a.name == b.name && kind_label(&a.kind) == kind_label(&b.kind));
            per_file.push((path.clone(), file_defs));
            for reference in &table.references {
                if reference.name.is_empty() {
                    stats.skipped_empty_names += 1;
                    continue;
                }
                *ref_counts
                    .entry(reference.name.clone())
                    .or_default()
                    .entry(path.clone())
                    .or_insert(0) += 1;
            }
        }
        per_file.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let files: Vec<PathBuf> = per_file.iter().map(|(path, _)| path.clone()).collect();
        let file_ids: BTreeMap<PathBuf, u32> = files
            .iter()
            .enumerate()
            .map(|(id, path)| (path.clone(), id as u32))
            .collect();
        let definitions: Vec<Vec<SymbolDef>> = per_file.into_iter().map(|(_, defs)| defs).collect();

        // Keep only names that actually produce a cross-file edge; the
        // loop consumes ref_counts so its maps move into the index
        // without a full clone at 10k-file scale.
        let mut refs_by_name = BTreeMap::new();
        let mut edges: Vec<(u32, u32, f64)> = Vec::new();
        for (name, counts) in ref_counts {
            let Some(defs) = def_kinds.get(&name) else {
                continue;
            };
            let cross = counts
                .keys()
                .any(|ref_file| defs.keys().any(|def_file| def_file != ref_file));
            if !cross {
                continue;
            }
            let def_sites: Vec<DefSite> = defs
                .iter()
                .map(|(file, kind)| DefSite {
                    file: file.clone(),
                    kind: kind.clone(),
                })
                .collect();
            for (ref_file, count) in &counts {
                let Some(&ref_id) = file_ids.get(ref_file) else {
                    continue;
                };
                for def_site in &def_sites {
                    let Some(&def_id) = file_ids.get(&def_site.file) else {
                        continue;
                    };
                    if ref_id != def_id {
                        edges.push((ref_id, def_id, *count as f64));
                    }
                }
            }
            refs_by_name.insert(
                name,
                SymbolLinks {
                    def_sites,
                    refs: counts,
                },
            );
        }

        stats.files = files.len();
        stats.symbols = refs_by_name.len();
        let csr = Csr::from_undirected(files.len() as u32, &edges);
        stats.edges = csr.entries() / 2;
        FileGraph {
            files,
            file_ids,
            definitions,
            refs_by_name,
            csr,
            stats,
        }
    }

    /// Files in node-id order (sorted by path).
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Node id for a file path, if present.
    pub fn node_id(&self, path: &Path) -> Option<u32> {
        self.file_ids.get(path).copied()
    }

    /// Renderable definitions of one file (name-sorted, deduped by
    /// name+kind); empty slice for an unknown id.
    pub fn definitions_of(&self, file: u32) -> &[SymbolDef] {
        self.definitions
            .get(file as usize)
            .map_or(&[], Vec::as_slice)
    }

    /// The name-keyed cross-reference index (see module docs).
    pub fn refs_by_name(&self) -> &BTreeMap<String, SymbolLinks> {
        &self.refs_by_name
    }

    /// The weighted undirected adjacency.
    pub fn csr(&self) -> &Csr {
        &self.csr
    }

    /// Undirected edge weight between two files.
    pub fn edge_weight(&self, a: u32, b: u32) -> Option<f64> {
        self.csr.weight(a, b)
    }

    /// Build counters.
    pub fn stats(&self) -> GraphStats {
        self.stats
    }
}
