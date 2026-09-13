---
name: maths
description: "Mathematical working guidance: show derivations step by step, verify symbolically before substituting numbers, sanity-check limits and dimensions, and state results with their assumptions."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["English", "Portuguese", "Spanish", "French", "German", "LaTeX"]
allowed-tools: ["view_window", "edit_lines", "tool_search"]
triggers:
  - integral
  - derivative
  - differential equation
  - linear algebra
  - eigenvalue
  - matrix determinant
  - proof by induction
  - convergence
  - probability distribution
  - bayesian
  - statistical significance
  - fourier transform
  - équation différentielle
  - ableitung
  - integral definida
do-not-trigger-on:
  - react component
  - docker compose
  - lease agreement
  - electrical panel
  - trial balance
  - financial model
  - code review
  - pull request
model-guidance:
  glm-5: Verify each manipulation symbolically (limit cases, dimension checks) before numeric substitution; never skip the assumption statement.
---

# Maths pack

{{shared:pack-conventions}}

## Workflow

1. **Restate the problem** with symbols defined (what is given, what is
   wanted, in what domain — real/complex, discrete/continuous).
2. **Plan the attack**: name the technique before computing (substitution,
   integration by parts, diagonalization, induction, dominated
   convergence, conditioning on a partition…).
3. **Carry symbols, not numbers**: keep the derivation symbolic; substitute
   values only at the end.
4. **Verify cheaply**: check special/limiting cases, dimensions/units,
   symmetry, and asymptotic behavior. A wrong sign or missing factor
   usually dies here.
5. **State assumptions and the exact result class**: e.g. "converges
   absolutely for Re(s) > 1", not "converges".
6. **Numerics last**: if a numeric answer is required, bound the error
   (round-off, truncation, conditioning) and report significant digits
   honestly.

## Worked example

Request: *"Does sum 1/n^2 from 1 to infinity converge, and to what?"`

```text
Technique: comparison (integral test / Fourier series).
Step 1  For n ≥ 1: 1/n² ≤ ∫[n-1,n] 1/x² dx   (x↦1/x² decreasing)
Step 2  Summing: Σ_{n=2}^{∞} 1/n² ≤ ∫[1,∞] 1/x² dx = 1
        ⇒ partial sums bounded above by 2 and increasing ⇒ converges.
Verify:  terms positive, decreasing — hypotheses of the test hold.
Value:   Parseval on f(x)=x over (-π,π): Σ 1/n² = π²/6 ≈ 1.6449.
Check:   partial sum to n=10 is 1.5498 < π²/6 < 2 = bound. ✓
```

Result stated with its class: the series converges absolutely; value
π²/6.

## Eval scenario

Prompt: `"Prove this integral converges"`. Expected routing: `maths`
(trigger: `integral`). First actions: identify the integral class,
choose the comparison/convergence technique, bound and verify with a
limiting case — full derivation kept symbolic until the final value is
needed.
