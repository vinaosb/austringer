//! Shared helpers for austringer-search integration tests.
//!
//! The corpus generator is deliberately dependency-free: a hand-rolled
//! xorshift64* PRNG with a fixed seed keeps the synthetic corpora
//! deterministic across runs (no flaky tests, no rand-version dependence).
#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;

/// Deterministic xorshift64* PRNG (Marsaglia). Never used outside tests.
pub struct XorShift64(u64);

impl XorShift64 {
    /// New generator. The seed is forced odd so the state never hits zero.
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// Next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish value in `0..n` (modulo bias is irrelevant for test data).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

const WORDS: &[&str] = &[
    "alpha", "beacon", "cascade", "drift", "ember", "fjord", "gale", "halo", "iris", "jade",
    "kite", "lumen", "moss", "nova", "onyx", "pyre", "quartz", "rune", "sable", "tide",
];

/// Deterministic synthetic Rust-ish module body for corpus file `i`.
/// Every file defines the unique symbol `widget_{i}` so symbol queries have
/// exactly one right answer, plus shared filler vocabulary for BM25 texture.
pub fn synth_file_content(i: u32, rng: &mut XorShift64) -> String {
    let w1 = WORDS[(i % WORDS.len() as u32) as usize];
    let w2 = WORDS[((i * 7 + 3) % WORDS.len() as u32) as usize];
    let w3 = WORDS[((i * 13 + rng.below(3) as u32) % WORDS.len() as u32) as usize];
    format!(
        "//! synthetic module {i}\n\
         use crate::prelude::*;\n\n\
         pub struct Widget{i} {{ pub tag: String }}\n\n\
         impl Widget{i} {{\n\
         \x20   pub fn widget_{i}(&self, input: &[u8]) -> String {{\n\
         \x20       let filler = format!(\"{w1}-{w2}-{w3}\");\n\
         \x20       filler.repeat(2)\n\
         \x20   }}\n\
         }}\n\n\
         // notes {w1} {w2} {w3} index {i}\n"
    )
}

/// Writes `count` synthetic files under `root`, spread over 100 directories.
pub fn write_synth_corpus(root: &Path, count: u32, rng: &mut XorShift64) {
    for i in 0..count {
        let dir = root.join(format!("mod_{:03}", i % 100));
        fs::create_dir_all(&dir).expect("create corpus dir");
        fs::write(
            dir.join(format!("file_{i:05}.rs")),
            synth_file_content(i, rng),
        )
        .expect("write corpus file");
    }
}

/// UTF-8 text file relative to `root`, parent dirs created as needed.
pub fn write_file(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    fs::write(path, contents).expect("write file");
}

/// Raw-bytes file relative to `root`, parent dirs created as needed.
pub fn write_bytes(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    fs::write(path, bytes).expect("write bytes file");
}

/// Adversarial files that must never panic the index or grep:
/// one NUL-containing binary, one invalid-UTF-8 text, one huge single line.
/// Only `utf8` and `huge_line` are indexable; the binary must be skipped.
pub fn write_adversarial_files(root: &Path) {
    write_bytes(
        root,
        "blob.bin",
        b"\x00\x01binary_needle_payload\x00\x02\xff more NULs\x00",
    );
    write_bytes(
        root,
        "weird_utf8.txt",
        b"plain head\nneedle_toxic\xff\xfe invalid tail\n",
    );
    let mut huge = String::with_capacity(300_000);
    huge.push_str(&"x".repeat(150_000));
    huge.push_str("huge_needle_alpha");
    huge.push_str(&"x".repeat(149_000));
    huge.push('\n');
    write_file(root, "huge_line.txt", &huge);
}

/// Makes `root` a real git repository with a `.gitignore` excluding
/// `ignored_dir/`, so the `ignore` walker's gitignore semantics apply.
pub fn enable_gitignore(root: &Path) {
    fs::create_dir_all(root.join(".git")).expect("create .git");
    write_file(root, ".gitignore", "ignored_dir/\n");
    // Real repo (not just a .git dir): belt and braces for require_git.
    Command::new("git")
        .arg("init")
        .arg("--quiet")
        .current_dir(root)
        .status()
        .expect("git init");
}
