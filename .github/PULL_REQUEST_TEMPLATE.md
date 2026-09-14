<!-- PR template — fields the review needs (plan todo 46: evidence + guardrail impact). -->

## What changes

One-paragraph summary of the mechanism, fix, or doc change, and the
workspace surfaces it touches (crates, docs, CI).

## Evidence

The receipts rule (CONTRIBUTING.md, evidence culture): behavior changes
cite existing evidence or add it.

- [ ] Tests: new/updated tests are linked below, and for defect fixes the
      RED evidence (the test failing before the fix) is described.
- [ ] Receipts: a research-file section ("research §N"), committed
      baseline, eval artifact, or precedent harness is cited where a
      design decision is involved.
- [ ] Numbers: any performance or benchmark claim in docs/README matches
      `harness-bench/budgets.json`, `harness-bench/baselines/`, or a
      recorded report (budgets are the contract; measurements are labeled
      machine-specific).

Test commands and results (exact):

```
cargo nextest run -p <crate>     # ...
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Guardrail impact

Which documented guardrails does this PR touch (silent model substitution,
embeddings-for-code, whole-file rewrites, telemetry default-on, auto-run
unverified plugins, serialization stability, Windows-first, hermetic
tests)? For each: not touched, or how it is respected. If a guard gains or
loses a test, name it.

## Perf-gate awareness

If this PR can plausibly move a baselined deterministic counter or a
budget measurement (allocation counts, prefix bytes, observation bytes,
page bytes, repomap tokens), say so; if a baseline legitimately moves,
attach the re-record note (procedure: harness-bench/baselines/README.md).

## Docs

Markdown storefront (README/CONTRIBUTING) or AsciiDoc docs/ changes are
xref-clean: `pwsh .github/scripts/linkcheck.ps1` passes, and new AsciiDoc
pages are registered in the build list in `.github/workflows/docs.yml`.
