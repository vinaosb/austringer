//! Terminal user interface frontend for the agent (plan todo 36).
//!
//! Thin client over the todo-35 daemon protocol: a synchronous,
//! event-driven UI loop (zero timers, renders only on state change)
//! fed by a terminal-input thread and the daemon WebSocket client
//! task. Everything renderable is a pure function of [`app::App`]
//! state, which is what the headless test surface exercises — no TTY
//! is required to drive or verify the interface.

pub mod app;
mod app_input;
mod app_server;
pub mod client;
pub mod complete;
pub mod diffs;
pub mod hud;
pub mod keys;
pub mod markdown;
pub mod omnibar;
mod panels;
pub mod render;
pub mod transcript;
