//! Seeded synthetic repositories (tests, benches, evidence runs) and a
//! hand-built [`TagTable`] helper for fixtures.
//!
//! Determinism: a splitmix64 stream (below) keyed by `(files, seed)`
//! drives every choice — same arguments, byte-identical repo. Shape
//! mirrors real mid-size code: ~4-8 definitions per file, ~6-15
//! references, a name pool of one symbol per file on average, plus
//! periodic HUB symbols (every 97th pool slot) that many files
//! reference, producing the weight skew PageRank is here for.

use austringer_ast::tags::{Definition, Point, Reference, Span, SymbolKind, TagTable};
use std::path::PathBuf;

/// Deterministic splitmix64.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
}

fn zero_span() -> Span {
    Span {
        start_byte: 0,
        end_byte: 0,
        start: Point { row: 0, column: 0 },
        end: Point { row: 0, column: 0 },
    }
}

fn kind_for(slot: u64) -> SymbolKind {
    match slot % 4 {
        0 => SymbolKind::Function,
        1 => SymbolKind::Struct,
        2 => SymbolKind::Method,
        _ => SymbolKind::Constant,
    }
}

/// Hand-build a tag table from `(name, kind)` definitions and name-only
/// references (zero spans — the graph reads names and kinds only).
pub fn table(defs: &[(&str, SymbolKind)], refs: &[&str]) -> TagTable {
    table_owned(
        defs.iter()
            .map(|(n, k)| ((*n).to_owned(), k.clone()))
            .collect(),
        refs.iter().map(|n| (*n).to_owned()).collect(),
    )
}

fn table_owned(defs: Vec<(String, SymbolKind)>, refs: Vec<String>) -> TagTable {
    TagTable {
        definitions: defs
            .into_iter()
            .map(|(name, kind)| Definition {
                name,
                kind,
                name_span: zero_span(),
                def_span: None,
            })
            .collect(),
        references: refs
            .into_iter()
            .map(|name| Reference {
                name,
                kind: SymbolKind::Call,
                span: zero_span(),
            })
            .collect(),
        truncated: false,
    }
}

/// Generate `files` synthetic tag tables under `src/m<dir>/f<n>.rs`
/// (20 files per directory). Deterministic per `(files, seed)`.
pub fn repo(files: usize, seed: u64) -> Vec<(PathBuf, TagTable)> {
    let pool = files.max(1) as u64;
    let mut rng = Rng(seed ^ (files as u64).wrapping_mul(0x517C_C1B7_2722_0A95));
    let mut out = Vec::with_capacity(files);
    for i in 0..files {
        let dir = i / 20;
        let path = PathBuf::from(format!("src/m{dir}/f{i}.rs"));
        let mut defs = Vec::new();
        for _ in 0..(4 + rng.below(5)) {
            let slot = rng.below(pool);
            defs.push((format!("sym{slot}"), kind_for(slot)));
        }
        let mut refs = Vec::new();
        for _ in 0..(6 + rng.below(10)) {
            let slot = if rng.below(3) == 0 {
                rng.below(pool / 97 + 1) * 97
            } else {
                rng.below(pool)
            };
            refs.push(format!("sym{slot}"));
        }
        out.push((path, table_owned(defs, refs)));
    }
    out
}
