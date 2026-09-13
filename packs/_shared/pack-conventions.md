## Pack conventions (shared partial)

This pack is registry content loaded by the harness skills loader — not
code compiled into the binary. Conventions every shipped pack follows:

- **Routing**: this pack matches a prompt when one of its `triggers`
  phrases appears (case-insensitive; single words respect word
  boundaries; phrases, CJK and other scripts match by containment).
- **Negative scope**: `do-not-trigger-on` phrases are hard exclusions.
  When one appears in a prompt, this pack stays out even if a positive
  trigger also matched — cross-domain work belongs to the pack that owns
  the domain.
- **Progressive disclosure**: only metadata (name, description, triggers)
  is loaded at startup; this body loads when the pack is actually
  invoked, with `{{shared:...}}` partials resolved.
- **Structure**: every pack body carries a `## Worked example` (one
  representative interaction or output) and an `## Eval scenario` (the
  prompt used to regression-test routing).
