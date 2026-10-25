---
name: Bug report
about: Something behaves incorrectly or crashes
title: "[bug] "
labels: bug
assignees: ""
---

**What happened**

A clear description of the incorrect behavior. If it is a crash, paste the
typed error output (this repo prefers typed errors over stack traces).

**What you expected**

The behavior you believe the contract (docs, test, or budget) requires.

**Reproduction**

Steps or the smallest command sequence that triggers it:

```sh
```

**Environment**

- OS and shell:
- Rust version (`rustc -V`) and whether it matches the workspace MSRV:
- Austringer commit (`git rev-parse --short HEAD`):
- Frontend involved (CLI / TUI / web / VS Code / daemon):

**Evidence**

Test output, `austringer doctor` output, perf-gate report, or an eval
artifact that demonstrates the defect — anything that turns this report
into a reproducible receipt. If you can pin it with a failing test, even
better (see the test-discipline section of CONTRIBUTING.md).
