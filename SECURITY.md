# Security Policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.1.x (pre-release development on `main`) | security fixes |

## Reporting a vulnerability

We take security bugs seriously. Please do **not** report security
vulnerabilities through public GitHub issues, discussions, or pull requests.

**Private reporting channel:** email **security@example.com**
(placeholder — replace with a monitored mailbox before any public release).

Please include as much of the following as you can:

- The type of issue and its assessed impact.
- Step-by-step reproduction instructions or a proof-of-concept.
- The affected commit(s) and configuration.
- Any ideas for mitigation.

We will acknowledge reports within 7 days and keep reporters updated through
resolution. Coordinated disclosure is preferred: we ask for reasonable time
to remediate before public disclosure. Good-faith research is welcome and
will not be met with legal action.

## Why this file exists from day one

A harness project that shipped without a SECURITY.md had no private reporting
channel when its first sandbox-escape report arrived (the "dsh" post-mortem,
research §6). This scaffold ships the channel before any code that could
need it.
