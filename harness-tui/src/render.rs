//! Screen rendering: a pure function of [`App`] state onto one ratatui
//! frame. Body = permission gate (when open) + collapsed accordions
//! (transcript / tools / team); bottom = one-line HUD + omnibar. The
//! tail of the transcript is kept visible (auto-follow). No widget
//! owns state and nothing here can block — snapshots pin the layout.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, Conn};
use crate::markdown;
use crate::transcript::Entry;

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn gate_lines(app: &App) -> Vec<Line<'static>> {
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
        None => "[y] once  [a] session  [n] deny  [c] cancel".to_owned(),
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

fn body_lines(app: &App, height: u16) -> Vec<Line<'static>> {
    let mut lines = gate_lines(app);
    let entries: Vec<Line<'static>> = app.view.entries().iter().flat_map(entry_lines).collect();
    lines.extend(accordion(
        "transcript",
        app.panels.transcript,
        app.view.entries().len(),
        || entries,
    ));
    let tools: Vec<Line<'static>> = app
        .view
        .tools()
        .iter()
        .map(|t| {
            let result = t.result.as_deref().unwrap_or("...");
            Line::from(format!("{} {} -> {}", t.tool, t.args, result))
        })
        .collect();
    lines.extend(accordion(
        "tools",
        app.panels.tools,
        app.view.tools().len(),
        || tools,
    ));
    lines.extend(accordion(
        "team",
        app.panels.team,
        app.view.children().len(),
        || team_panel(app),
    ));
    for note in app.view.notes() {
        lines.push(Line::styled(format!("* {note}"), dim()));
    }
    if lines.len() > usize::from(height) {
        let keep_from = lines.len() - usize::from(height);
        lines.drain(..keep_from);
    }
    lines
}

fn hud_line(app: &App) -> Line<'static> {
    let conn = match &app.conn {
        Conn::Attached => String::new(),
        Conn::Disconnected => "disconnected (ctrl+r reconnect) ".to_owned(),
        Conn::Connecting => "connecting... ".to_owned(),
        Conn::Failed(err) => format!("failed: {err} "),
    };
    let resync = if app.view.resync_pending {
        "resync... "
    } else {
        ""
    };
    Line::from(Span::raw(format!(
        "{conn}{resync}{} | {} | {}",
        app.hud.context_line(),
        app.hud.cache_line(),
        app.hud.cost_line()
    )))
}

fn completions_popup(f: &mut Frame, app: &App, above: Rect) {
    let items = app.completions_for(8);
    if items.is_empty() {
        return;
    }
    let width = 42.min(above.width);
    let height = (items.len() as u16 + 2).min(above.height);
    let area = Rect {
        x: above.x,
        y: above.y.saturating_sub(height),
        width,
        height,
    };
    let lines: Vec<Line<'static>> = items.into_iter().map(Line::from).collect();
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("completions")),
        area,
    );
}

/// Render the whole interface. Pure with respect to `app`.
pub fn render(f: &mut Frame, app: &App) {
    let [body, hud_area, bar_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .areas(f.area());

    f.render_widget(Paragraph::new(body_lines(app, body.height)), body);
    let hud = if app.show_hud {
        hud_line(app)
    } else {
        Line::from("")
    };
    f.render_widget(Paragraph::new(hud), hud_area);

    let title = if app.bar_focused() {
        "harness (i typed / esc blur / enter send)"
    } else {
        "harness (i to type, q quits)"
    };
    let border = if app.bar_focused() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let cursor = if app.bar_focused() { "|" } else { "" };
    let text = format!("{}{cursor}", app.bar.text());
    f.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(border)
                .title(title),
        ),
        bar_area,
    );
    completions_popup(f, app, body);
}
