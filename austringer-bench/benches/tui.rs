//! Criterion bench: todo-36 TUI frame render (ratatui TestBackend over
//! the real `render` path — the p99/<16ms budget surface).

use std::hint::black_box;

use austringer_tui::app::{App, DEFAULT_CONTEXT_WINDOW, UiEvent};
use austringer_tui::render::render;
use criterion::{Criterion, criterion_group, criterion_main};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn frame_render(c: &mut Criterion) {
    let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
    let _ = app.step(UiEvent::Attached);
    app.rendered();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("test terminal");
    c.bench_function("tui/frame_render_120x40", |b| {
        b.iter(|| {
            terminal
                .draw(|frame| render(black_box(frame), black_box(&app)))
                .expect("draw");
            app.rendered();
        })
    });
}

criterion_group!(benches, frame_render);
criterion_main!(benches);
