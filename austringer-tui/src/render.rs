//! Screen rendering: a pure function of [`App`] state onto one ratatui
//! frame. Body = ask modals + collapsed accordions (transcript /
//! tools / diffs / team) + notes, windowed by height with a
//! scroll offset (0 = auto-follow the tail); bottom = one-line HUD +
//! omnibar. Panel bodies live in `panels`; nothing here can block —
//! snapshots pin the layout.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, Conn};
use crate::panels;

fn body_lines(app: &App, height: u16) -> Vec<Line<'static>> {
    let mut lines = panels::compose_body(app);
    let height = usize::from(height);
    if lines.len() > height {
        let end = lines
            .len()
            .saturating_sub(app.scroll_offset().min(lines.len()));
        let start = end.saturating_sub(height);
        lines.drain(..start);
        lines.truncate(end - start);
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
        "austringer (i typed / esc blur / enter send)"
    } else {
        "austringer (i to type, q quits)"
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
