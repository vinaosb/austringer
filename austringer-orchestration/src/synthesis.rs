//! Synthesis: structured results merged with dedup and the
//! cross-consistency check (audit-consolidated pattern): duplicate
//! artifacts collapse, divergent content on one path is an inconsistency
//! naming both children, and per-agent telemetry rolls into the report.

use crate::launch::{Artifact, ChildReport};
use crate::telemetry::clamp_summary;

/// One child's position on an inconsistency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// Child id.
    pub child: String,
    /// The child's claim (content hash).
    pub claim: String,
}

/// A cross-consistency finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inconsistency {
    /// The contested topic (artifact path).
    pub topic: String,
    /// The diverging positions.
    pub positions: Vec<Position>,
}

/// One per-agent telemetry line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLine {
    /// Child id.
    pub child: String,
    /// Tokens charged.
    pub tokens: u64,
    /// Cost in micro-USD.
    pub cost_micro_usd: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Failure class wire name, if failed.
    pub failure_class: Option<&'static str>,
}

/// The synthesis result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisReport {
    /// Merged summary (bounded to the 2k band).
    pub merged_summary: String,
    /// Deduped artifacts.
    pub artifacts: Vec<Artifact>,
    /// Cross-consistency findings.
    pub inconsistencies: Vec<Inconsistency>,
    /// Per-agent telemetry.
    pub per_agent: Vec<AgentLine>,
}

/// Merges child reports: dedup identical artifacts, flag divergent
/// content per path, roll telemetry per agent.
pub fn synthesize(reports: &[ChildReport]) -> SynthesisReport {
    let mut by_path: std::collections::BTreeMap<String, Vec<Position>> =
        std::collections::BTreeMap::new();
    for report in reports {
        for artifact in &report.artifacts {
            by_path
                .entry(artifact.path.clone())
                .or_default()
                .push(Position {
                    child: report.child_id.clone(),
                    claim: artifact.content_hash.clone(),
                });
        }
    }
    let mut artifacts = Vec::new();
    let mut inconsistencies = Vec::new();
    for (path, positions) in by_path {
        let distinct: std::collections::BTreeSet<&str> =
            positions.iter().map(|p| p.claim.as_str()).collect();
        if distinct.len() > 1 {
            inconsistencies.push(Inconsistency {
                topic: path.clone(),
                positions,
            });
        } else {
            let claim = positions
                .first()
                .map(|p| p.claim.clone())
                .unwrap_or_default();
            artifacts.push(Artifact {
                path,
                content_hash: claim,
                source: std::path::PathBuf::new(),
            });
        }
    }

    let mut per_child: std::collections::BTreeMap<String, AgentLine> =
        std::collections::BTreeMap::new();
    for report in reports {
        let line = per_child
            .entry(report.child_id.clone())
            .or_insert(AgentLine {
                child: report.child_id.clone(),
                tokens: 0,
                cost_micro_usd: 0,
                duration_ms: 0,
                failure_class: report.telemetry.failure_class.map(|c| c.name()),
            });
        line.tokens = line.tokens.saturating_add(report.telemetry.total_tokens());
        line.cost_micro_usd = line
            .cost_micro_usd
            .saturating_add(report.telemetry.cost_micro_usd);
        line.duration_ms = line.duration_ms.max(report.telemetry.duration_ms);
    }
    let per_agent: Vec<AgentLine> = per_child.into_values().collect();

    let merged = reports
        .iter()
        .map(|r| format!("{}: {}", r.child_id, r.summary))
        .collect::<Vec<_>>()
        .join("\n");

    SynthesisReport {
        merged_summary: clamp_summary(&merged, crate::telemetry::SUMMARY_MAX),
        artifacts,
        inconsistencies,
        per_agent,
    }
}
