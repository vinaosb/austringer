//! Action-specific check layers shared by the engine (todo 27):
//! sandbox path ceilings (protected paths, writable roots, secret
//! reads), the execute pipeline (program resolution → hard blocklist
//! → prefix coverage), URL egress + SSRF, malformed-shape typing, and
//! the audit summarizer. HARD checks (deny-only) run before grants;
//! SOFT checks (asks, coverage, stances) run after.

use std::path::Path;

use crate::egress::EgressPolicy;
use crate::perm::commands::{self, CommandRules};
use crate::perm::floor::AdminFloor;
use crate::perm::grammar::{Action, GrammarVerdict, Rule, Target, UrlTarget};
use crate::perm::hardline;
use crate::perm::profile::SecurityProfile;
use crate::perm::reasons::PermissionReason;
use crate::perm::resolve::{ProgramResolver, Resolution};
use crate::policy::decl::SandboxPolicy;

/// Hard (deny-only) path ceilings: protected paths and writable
/// roots. Runs before grants so no grant can dodge the sandbox.
#[must_use]
pub(crate) fn path_denies(
    action: Action,
    path: &Path,
    sandbox: &SandboxPolicy,
) -> Option<PermissionReason> {
    if !matches!(action, Action::Write | Action::Edit) {
        return None;
    }
    if sandbox.protected_paths.iter().any(|p| path.starts_with(p)) {
        return Some(PermissionReason::ProtectedPath);
    }
    if !sandbox.writable_roots.iter().any(|r| path.starts_with(r)) {
        return Some(PermissionReason::WriteOutsideRoot);
    }
    None
}

/// Soft path outcomes, evaluated after grants/grammar produced a
/// candidate.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SoftPath {
    /// No soft outcome.
    None,
    /// Secret read: ask unless an allow candidate exists (then the
    /// redaction notice rides along).
    SecretAsk(PermissionReason, Option<String>),
    /// Rule-less read inside granted roots.
    ReadOnlyStance,
}

#[must_use]
pub(crate) fn path_soft(
    action: Action,
    path: &Path,
    sandbox: &SandboxPolicy,
    secret_patterns: &[String],
) -> SoftPath {
    if action == Action::Read && is_secret(path, secret_patterns) {
        return SoftPath::SecretAsk(
            PermissionReason::ContainsRedaction,
            Some("[REDACTED] secret contents masked in transcript".into()),
        );
    }
    if action == Action::Read
        && sandbox
            .writable_roots
            .iter()
            .chain(sandbox.readable_roots.iter())
            .any(|r| path.starts_with(r))
    {
        return SoftPath::ReadOnlyStance;
    }
    SoftPath::None
}

#[must_use]
pub(crate) fn is_secret(path: &Path, patterns: &[String]) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    patterns
        .iter()
        .any(|p| wildcard_match(p, &normalized) || wildcard_match(p, basename))
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return true;
    };
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    for part in parts {
        if part.is_empty() {
            continue;
        }
        let Some(at) = rest.find(part) else {
            return false;
        };
        rest = &rest[at + part.len()..];
    }
    true
}

/// Hard (deny-only) execute checks: malformed shape, program
/// resolution (fail closed), resolved-program blocklist, profile
/// program ceiling, and the command deny-prefix list. Runs before
/// grants.
#[must_use]
pub(crate) fn execute_denies(
    command: &str,
    rules: &CommandRules,
    resolver: &dyn ProgramResolver,
    profile: &SecurityProfile,
) -> Option<PermissionReason> {
    let segments = match commands::split_compound(command) {
        Ok(segments) => segments,
        Err(_) => return Some(PermissionReason::MalformedCommand),
    };
    for segment in &segments {
        match resolver.resolve(&segment.program_word) {
            Resolution::NotFound | Resolution::Suspicious { .. } => {
                return Some(PermissionReason::ResolutionFailed);
            }
            Resolution::Resolved { program, wrapper } => {
                if hardline::program_blocked(&program)
                    || wrapper.as_deref().is_some_and(hardline::program_blocked)
                {
                    return Some(PermissionReason::BlocklistResolved);
                }
                if profile
                    .denied_programs
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(&program))
                {
                    return Some(PermissionReason::ProfileCeiling);
                }
            }
        }
    }
    match rules.evaluate(command) {
        Ok(commands::CommandVerdict::DeniedByPrefix { .. }) => {
            Some(PermissionReason::CommandPrefixDenied)
        }
        Ok(_) => None,
        Err(_) => Some(PermissionReason::MalformedCommand),
    }
}

/// Soft execute outcome: prefix coverage (the Roo substitution
/// guard — every segment must be independently allowed).
#[must_use]
pub(crate) fn execute_coverage(command: &str, rules: &CommandRules) -> Option<PermissionReason> {
    match rules.evaluate(command) {
        Ok(commands::CommandVerdict::SubstitutionBlocked { .. }) => {
            Some(PermissionReason::SubstitutionGuard)
        }
        _ => None,
    }
}

/// Hard (deny-only) URL checks: SSRF guard, then the
/// permissions-derived egress allowlist.
#[must_use]
pub(crate) fn url_denies(
    url: &UrlTarget,
    admin: &AdminFloor,
    egress: &EgressPolicy,
) -> Option<PermissionReason> {
    if let Some(kind) = crate::perm::floor::ssrf_violation(&url.host, Some(admin)) {
        return Some(PermissionReason::SsrfGuard { kind });
    }
    if egress.verdict(&url.host, url.port).is_denied() {
        return Some(PermissionReason::NotInEgressAllowlist);
    }
    None
}

/// Typed malformed-shape gate before any evaluation.
#[must_use]
pub(crate) fn shape(action: Action, target: &Target) -> Option<PermissionReason> {
    match (action, target) {
        (Action::Execute, Target::Command(c)) if c.trim().is_empty() => {
            Some(PermissionReason::MalformedCommand)
        }
        (Action::Execute, Target::Command(_)) => None,
        (Action::Execute, _) => Some(PermissionReason::MalformedTarget),
        (a, Target::Url(_)) if !a.is_url_action() => Some(PermissionReason::MalformedTarget),
        (a, Target::Command(_)) if a.is_url_action() => Some(PermissionReason::MalformedTarget),
        _ => None,
    }
}

/// Redacted audit summary for a target.
#[must_use]
pub(crate) fn summarize(target: &Target, secret_patterns: &[String]) -> String {
    match target {
        Target::Path(p) => {
            if is_secret(p, secret_patterns) {
                "[REDACTED:secret-path]".into()
            } else {
                p.to_string_lossy().replace('\\', "/")
            }
        }
        Target::Command(c) => c.clone(),
        Target::Url(u) => format!("{}://{}", u.scheme, u.host),
        Target::Tool(t) => t.clone(),
    }
}

/// The stricter of two grammar verdicts (deny > ask > allow).
pub(crate) fn strictest(a: GrammarVerdict, b: GrammarVerdict) -> GrammarVerdict {
    if verdict_rank(&b) > verdict_rank(&a) {
        b
    } else {
        a
    }
}

/// Grammar verdict over BOTH the absolute target and its root-relative
/// form (rules are authored relative to the worktree; the strictest of
/// the two verdicts wins so an absolute-form deny can never be dodged
/// by relativization).
pub(crate) fn grammar_verdict(
    rules: &[Rule],
    admin: &AdminFloor,
    sandbox: &SandboxPolicy,
    action: &Action,
    target: &Target,
) -> GrammarVerdict {
    let absolute = admin.enforce(rules, action, target);
    match relativize(sandbox, target) {
        Some(rel) if &rel != target => strictest(absolute, admin.enforce(rules, action, &rel)),
        _ => absolute,
    }
}

/// The target rebased onto its sandbox root, when it lives under one
/// (path targets only).
pub(crate) fn relativize(sandbox: &SandboxPolicy, target: &Target) -> Option<Target> {
    let Target::Path(path) = target else {
        return None;
    };
    sandbox
        .writable_roots
        .iter()
        .chain(sandbox.readable_roots.iter())
        .find_map(|root| path.strip_prefix(root).ok())
        .map(|rel| Target::Path(rel.to_path_buf()))
}

fn verdict_rank(verdict: &GrammarVerdict) -> u8 {
    match verdict {
        GrammarVerdict::Denied { admin: true, .. } => 4,
        GrammarVerdict::Denied { .. } => 3,
        GrammarVerdict::Asked { .. } => 2,
        GrammarVerdict::Allowed { .. } => 1,
        GrammarVerdict::NoOpinion => 0,
    }
}

/// Whether yolo may auto-approve this ask. Coverage gaps
/// (substitution), breaker trips, and profile ceilings survive yolo.
pub(crate) fn yolo_convertible(reason: &PermissionReason) -> bool {
    !matches!(
        reason,
        PermissionReason::SubstitutionGuard
            | PermissionReason::CircuitBreakerTripped { .. }
            | PermissionReason::ProfileClamp
            | PermissionReason::ProfileCeiling
    )
}

/// The profile-clamp key for a target: URL host, first command word,
/// normalized path, or tool name.
pub(crate) fn grant_key(target: &Target) -> String {
    match target {
        Target::Url(u) => u.host.clone(),
        Target::Command(c) => c.split_whitespace().next().unwrap_or_default().to_string(),
        Target::Path(p) => p.to_string_lossy().replace('\\', "/"),
        Target::Tool(t) => t.clone(),
    }
}
