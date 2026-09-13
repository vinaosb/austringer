### Findings schema (exemplar subset)

Every finding MUST carry these fields. The full v3.2 squad template adds
trending fields (`fingerprint`, `status`, `first_seen`, `last_seen`, …)
and delta tracking; this subset keeps the port fixture-quality.

```json
{
  "id": "SEC-0001",
  "severity": "critical",
  "title": "SQL injection in search endpoint",
  "root_cause": "unparameterized concat in SqlBuilder",
  "impact": "attacker reads arbitrary table rows",
  "evidence": "src/api/search.rs:42 (quoted snippet)",
  "fix": "bind user input as a parameter; add regression test",
  "verification_command": "cargo test sql_injection_regression",
  "confidence": 0.9,
  "files_examined": ["src/api/search.rs", "src/db/mod.rs"]
}
```

Rules: `severity` ∈ `critical|high|medium|low`; a finding missing any
required field is REJECTED at consolidation time; findings are never
collapsed to counts; every finding carries file:line evidence and a
self-assessed false-positive risk.
