//! TUI render scenario: frame-budget discipline over the real render
//! path (todo 36 `render` + ratatui TestBackend — buffer reuse via the
//! terminal's diffed front buffer). p99 (min-of-runs) faces the <16 ms
//! budget; frame breaches/hangs face the 10 ms/100 ms Zed thresholds;
//! the debug overlay is proven renderable through the same path.

use std::time::{Duration, Instant};

use austringer_tui::app::{App, DEFAULT_CONTEXT_WINDOW, UiEvent};
use austringer_tui::render::render;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::widgets::Paragraph;

use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::frame::{FrameSample, FrameVerdict, classify, overlay_lines};
use crate::gate::Policy;

pub const FRAMES: u32 = 200;
pub const RUNS: u32 = 3;

fn draw_frame(
    terminal: &mut Terminal<TestBackend>,
    app: &mut App,
) -> Result<Duration, ScenarioError> {
    let started = Instant::now();
    terminal
        .draw(|frame| render(frame, app))
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    app.rendered();
    Ok(started.elapsed())
}

/// One run: FRAMES draws of an attached app; the terminal is reused
/// across frames (buffer-reuse discipline — ratatui diffs the front
/// buffer instead of rebuilding it).
fn run_once() -> Result<Vec<Duration>, ScenarioError> {
    let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
    let _ = app.step(UiEvent::Attached);
    let mut terminal = Terminal::new(TestBackend::new(120, 40))
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    app.rendered();
    let mut frames = Vec::new();
    for _ in 0..FRAMES {
        frames.push(draw_frame(&mut terminal, &mut app)?);
    }
    Ok(frames)
}

/// Prove the debug overlay is displayable through the same render path.
fn overlay_renders(overlay: &[String]) -> Result<(), ScenarioError> {
    let text = overlay.join("\n");
    let height = u16::try_from(overlay.len() + 2).unwrap_or(12);
    let mut terminal = Terminal::new(TestBackend::new(80, height))
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new(text), frame.area());
        })
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let drew_content = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|c| c.symbol() != " ");
    if !drew_content {
        return Err(ScenarioError::Failed("overlay rendered empty".into()));
    }
    Ok(())
}

fn verdict_of(frames: &[Duration], budget: Duration, hang: Duration) -> FrameVerdict {
    let samples: Vec<FrameSample> = frames
        .iter()
        .enumerate()
        .map(|(i, d)| FrameSample {
            frame: i as u32,
            total: *d,
            phases: vec![("draw", *d)],
        })
        .collect();
    classify(&samples, budget, hang).unwrap_or_default()
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let budget = Duration::from_millis(ctx.budgets.frame_budget_ms);
    let hang = Duration::from_millis(ctx.budgets.hang_threshold_ms);
    let mut out = ScenarioOutput::new();
    let mut best: Option<(FrameVerdict, Vec<String>)> = None;
    #[cfg(feature = "dhat")]
    let heap_before = super::heap::snapshot();
    for _ in 0..RUNS {
        let frames = run_once()?;
        let verdict = verdict_of(&frames, budget, hang);
        let overlay = overlay_lines(&verdict, budget, hang);
        overlay_renders(&overlay)?;
        let improves = best.as_ref().is_none_or(|(v, _)| verdict.p99 < v.p99);
        if improves {
            best = Some((verdict, overlay));
        }
    }
    #[cfg(feature = "dhat")]
    super::heap::record(&mut out, "tui", heap_before);
    let (verdict, _overlay) = best.ok_or_else(|| ScenarioError::Failed("no frame runs".into()))?;
    out.push(
        "tui.p99_frame_ms",
        verdict.p99.as_secs_f64() * 1000.0,
        "ms",
        Policy::Budget {
            budget: ctx.budgets.p99_frame_ms as f64,
        },
    );
    out.push(
        "tui.p50_frame_ms",
        verdict.p50.as_secs_f64() * 1000.0,
        "ms",
        Policy::Advisory,
    );
    out.push(
        "tui.frame_breaches",
        verdict.breaches.len() as f64,
        "frames",
        Policy::Budget { budget: 0.0 },
    );
    out.push(
        "tui.hangs",
        verdict.hangs.len() as f64,
        "frames",
        Policy::Budget { budget: 0.0 },
    );
    Ok(out)
}
