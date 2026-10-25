//! Body-panel rendering (todo 19): ask modals (permission, fallback,
//! plan), the collapsed-by-default accordions (transcript / tools /
//! diffs / team), and client notes. One line per tool with its
//! result summary — hunk detail lives in the diffs accordion
//! (Amp/Antigravity/Claude/Devin/goose convergence). Pure functions
//! of [`App`]; nothing here can block.

use austringer_daemon::grammar::{ALLOW_SCOPES, VOCAB_AGENTS};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

use crate::app::App;
use crate::diffs::{self, HunkDecision};
use crate::markdown;
use crate::transcript::Entry;

/// Plan-artifact preview bound (lines shown in the modal).
const PLAN_PREVIEW_LINES: usize = 8;

/// Result-text bound on the one-line-per-tool row.
const TOOL_RESULT_CHARS: usize = 48;

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn modal_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = permission_gate_lines(app);
    lines.extend(fallback_lines(app));
    lines.extend(plan_lines(app));
    lines
}

fn permission_gate_lines(app: &App) -> Vec<Line<'static>> {
    let Some(ask) = app.gate() else {
        return Vec::new();
    };
    let mut lines = vec![
        Line::styled(
            format!("permission: {} ", ask.tool),
            Style::default().fg(Color::Yellow),
        ),
        Line::styled(ask.reason.clone(), dim()),
    ];
    if let Some(target) = &ask.target {
        lines.push(Line::styled(format!("target: {target}"), dim()));
    }
    let status = match app.pending_answer_label() {
        Some(label) => format!("answered: {label} — waiting for resolution"),
        None => ALLOW_SCOPES.join("  "),
    };
    lines.push(Line::from(status));
    lines.push(Line::from(""));
    lines
}

fn fallback_lines(app: &App) -> Vec<Line<'static>> {
    let Some(ask) = app.fallback_gate() else {
        return Vec::new();
    };
    let mut lines = vec![
        Line::styled(
            format!(
                "fallback proposed: {} -> {}",
                ask.current.model, ask.proposed.model
            ),
            Style::default().fg(Color::Yellow),
        ),
        Line::styled(ask.reason.clone(), dim()),
    ];
    let status = match app.fallback_pending_label() {
        Some(label) => format!("answered: {label} — waiting for resolution"),
        None => format!(
            "[y] hop to {}  [n] keep {}",
            ask.proposed.model, ask.current.model
        ),
    };
    lines.push(Line::from(status));
    lines.push(Line::from(""));
    lines
}

fn plan_lines(app: &App) -> Vec<Line<'static>> {
    let Some(ask) = app.plan_gate() else {
        return Vec::new();
    };
    let mut lines = vec![Line::styled(
        format!(
            "plan {} (revision {}) awaiting approval",
            ask.plan_id, ask.revision
        ),
        Style::default().fg(Color::Yellow),
    )];
    lines.extend(
        ask.artifact
            .lines()
            .take(PLAN_PREVIEW_LINES)
            .map(|line| Line::styled(line.to_owned(), dim())),
    );
    if ask.artifact.lines().count() > PLAN_PREVIEW_LINES {
        lines.push(Line::styled("\u{2026}", dim()));
    }
    let status = match app.plan_pending_label() {
        Some(label) => format!("answered: {label} — waiting for resolution"),
        None => "[y] approve  [n] reject (plan mode stays active)".to_owned(),
    };
    lines.push(Line::from(status));
    lines.push(Line::from(""));
    lines
}

fn entry_lines(entry: &Entry) -> Vec<Line<'static>> {
    match entry {
        Entry::User(text) => vec![Line::from(format!("you: {text}"))],
        Entry::Assistant(text) => markdown::render(text),
        Entry::System(text) => vec![Line::styled(format!("sys: {text}"), dim())],
        Entry::Notice(text) => vec![Line::styled(format!("note: {text}"), dim())],
        Entry::Condensed(summary) => {
            vec![Line::styled(format!("~ condensed: {summary}"), dim())]
        }
    }
}

fn accordion(
    title: &str,
    open: bool,
    count: usize,
    body: impl FnOnce() -> Vec<Line<'static>>,
) -> Vec<Line<'static>> {
    let marker = if open { "v" } else { ">" };
    let mut lines = vec![Line::styled(
        format!("{marker} {title} ({count})"),
        Style::default().fg(Color::Cyan),
    )];
    if open {
        lines.extend(body());
    }
    lines
}

fn team_rows(app: &App) -> Vec<Line<'static>> {
    let mut children = app.view.children();
    children.sort_by_key(|c| (c.task.clone(), c.child.clone()));
    children
        .iter()
        .map(|c| {
            if c.detail.is_empty() {
                Line::from(format!("{:<16} {:<10} {}", c.task, c.child, c.state))
            } else {
                Line::from(format!(
                    "{:<16} {:<10} {} {}",
                    c.task, c.child, c.state, c.detail
                ))
            }
        })
        .collect()
}

/// The team accordion body: the Warp-precedence aggregate, the
/// delegation budget, then one row per child (todo 15).
fn team_panel(app: &App) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(
        format!("status: {}", app.view.team_status().as_str()),
        Style::default().fg(Color::Cyan),
    )];
    if let Some(budget) = app.view.team_budget() {
        lines.push(Line::styled(
            format!(
                "budget: {}/{} children | {}/{} tokens",
                budget.spawned, budget.fan_out_cap, budget.tokens_spent, budget.token_budget
            ),
            dim(),
        ));
    }
    lines.extend(team_rows(app));
    lines
}

/// First line of a tool result, bounded for the one-line row.
fn tool_result_summary(text: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    if first.chars().count() > TOOL_RESULT_CHARS {
        let cut: String = first.chars().take(TOOL_RESULT_CHARS - 1).collect();
        format!("{cut}\u{2026}")
    } else {
        first.to_owned()
    }
}

fn tools_panel(app: &App) -> Vec<Line<'static>> {
    app.view
        .tools()
        .iter()
        .map(|t| {
            let result = match t.result.as_deref() {
                Some(text) => tool_result_summary(text),
                None => "...".to_owned(),
            };
            Line::from(format!("{} {} -> {}", t.tool, t.args, result))
        })
        .collect()
}

/// The hunk-review accordion body (todo 19a): one diffstat line per
/// file, then each hunk with its decision marker. Unparseable hunks
/// render a placeholder — never a parse.
fn diffs_panel(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for diff in app.view.diffs() {
        let (added, removed) = diff.stats();
        lines.push(Line::styled(
            format!(
                "{} +{added}/-{removed} ({} pending)",
                diff.path,
                diff.pending_count()
            ),
            Style::default().fg(Color::Cyan),
        ));
        for hunk in &diff.hunks {
            let marker = match hunk.decision {
                HunkDecision::Pending => "pending",
                HunkDecision::Accepted => "accepted",
                HunkDecision::Rejected => "rejected",
            };
            if !diffs::hunk_is_parseable(&hunk.text) {
                lines.push(Line::styled(
                    format!("  [{marker}] {}", diffs::UNPARSEABLE_HUNK),
                    dim(),
                ));
                continue;
            }
            for line in hunk.text.lines() {
                if line.starts_with("@@") {
                    lines.push(Line::styled(format!("  [{marker}] {line}"), dim()));
                } else {
                    lines.push(Line::from(format!("  {line}")));
                }
            }
        }
    }
    lines
}

/// Every body line before height windowing: modals, the four
/// accordions, then client notes.
pub(crate) fn compose_body(app: &App) -> Vec<Line<'static>> {
    let mut lines = modal_lines(app);
    let entries: Vec<Line<'static>> = app.view.entries().iter().flat_map(entry_lines).collect();
    lines.extend(accordion(
        "transcript",
        app.panels.transcript,
        app.view.entries().len(),
        || entries,
    ));
    lines.extend(accordion(
        "tools",
        app.panels.tools,
        app.view.tools().len(),
        || tools_panel(app),
    ));
    lines.extend(accordion(
        "diffs",
        app.panels.diffs,
        app.view.diffs().len(),
        || diffs_panel(app),
    ));
    lines.extend(accordion(
        VOCAB_AGENTS,
        app.panels.team,
        app.view.children().len(),
        || team_panel(app),
    ));
    for note in app.view.notes() {
        lines.push(Line::styled(format!("* {note}"), dim()));
    }
    lines
}

/// Total body lines (the scroll clamp: the offset may not exceed
/// the content it windows over).
pub(crate) fn body_line_count(app: &App) -> usize {
    compose_body(app).len()
}
