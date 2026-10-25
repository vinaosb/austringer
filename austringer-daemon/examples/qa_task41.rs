//! Task-41 manual QA transcript driver: spawns the REAL daemon binary
//! with a declarative automation seeded on disk, then proves the
//! scheduled-run loop end to end on the wall clock — cron fire, session
//! kickoff (durable journal + live /sessions), completion gate + run
//! history rows, and OSC 777/9 notification bytes on the daemon's own
//! stdout. Run: `cargo run -p austringer-daemon --example qa_task41`.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    port: u16,
    token: String,
    data_dir: std::path::PathBuf,
    out: Arc<Mutex<Vec<u8>>>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

fn spawn_daemon(data_dir: &std::path::Path, port: u16) -> Result<Daemon, String> {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().and_then(|d| d.parent()).map(|d| d.to_path_buf()))
        .map(|dir| {
            #[cfg(windows)]
            let name = "austringer-daemon.exe";
            #[cfg(not(windows))]
            let name = "austringer-daemon";
            dir.join(name)
        })
        .ok_or_else(|| "no exe dir".to_owned())?;
    let mut child = Command::new(exe)
        .args([
            "--data-dir",
            data_dir.to_str().unwrap_or("."),
            "--port",
            &port.to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let out = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&out);
    let pipe = child.stdout.take();
    std::thread::spawn(move || {
        let Some(mut pipe) = pipe else { return };
        let mut buf = [0u8; 512];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut guard) = sink.lock() {
                        guard.extend_from_slice(&buf[..n]);
                    }
                }
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut banner = String::new();
    while Instant::now() < deadline {
        if let Ok(guard) = out.lock() {
            banner = String::from_utf8_lossy(&guard).into_owned();
        }
        if banner.contains("token: ") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let token = banner
        .split("token: ")
        .nth(1)
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect::<String>()
        })
        .filter(|t| !t.is_empty())
        .ok_or_else(|| format!("no token in banner: {banner}"))?;
    Ok(Daemon {
        child,
        port,
        token,
        data_dir: data_dir.to_owned(),
        out,
    })
}

fn seed_automations(data_dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| format!("mkdir: {e}"))?;
    std::fs::write(
        data_dir.join("pack-tiers.json"),
        br#"{"qa-pack": "approved"}"#,
    )
    .map_err(|e| format!("pack tiers: {e}"))?;
    let automation = serde_json::json!([{
        "id": "qa41",
        "name": "task-41 QA nightly",
        "cron": "* * * * *",
        "pack": "qa-pack",
        "goal": "task-41 QA: prove the scheduled loop end to end",
        "criteria": [],
        "budget_turns": null,
        "max_invocations_total": null,
        "max_invocations_per_day": null,
        "network": "online",
        "enabled": true,
    }]);
    std::fs::write(
        data_dir.join("automations.json"),
        serde_json::to_vec_pretty(&automation).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("automations: {e}"))
}

fn stdout_text(daemon: &Daemon) -> String {
    match daemon.out.lock() {
        Ok(guard) => String::from_utf8_lossy(&guard).into_owned(),
        Err(poisoned) => String::from_utf8_lossy(&poisoned.into_inner()).into_owned(),
    }
}

fn read_file(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let data_dir = std::env::temp_dir().join(format!("austringer-qa41-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data_dir);
    seed_automations(&data_dir)?;
    let daemon = spawn_daemon(&data_dir, 18909)?;
    let base = format!("127.0.0.1:{}", daemon.port);
    let token = daemon.token.clone();
    println!("=== TASK-41 QA: real binary, declarative automation, wall clock ===");
    println!(
        "banner: {}",
        stdout_text(&daemon).lines().next().unwrap_or("")
    );
    println!("automation seeded: qa41 `* * * * *` pack=qa-pack (approved tier)");

    let journal = data_dir.join("scheduler-journal.jsonl");
    let runs = data_dir.join("runs.jsonl");
    let mut osc = false;
    let mut fired = false;
    let mut started_row = false;
    let mut done_row = false;
    let mut sessions_live = String::new();
    let deadline = Instant::now() + Duration::from_secs(150);
    while Instant::now() < deadline {
        let out = stdout_text(&daemon);
        osc = osc || (out.contains("\u{1b}]777;notify;austringer;") && out.contains("\u{1b}]9;"));
        fired = fired || read_file(&journal).contains("scheduled.fire");
        let rows = read_file(&runs);
        started_row = started_row || rows.contains("\"started\"");
        done_row = done_row || rows.contains("\"verdict\":");
        if fired && started_row {
            let client = reqwest::Client::new();
            if let Ok(response) = client
                .get(format!("http://{base}/sessions"))
                .bearer_auth(&token)
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                sessions_live = response.text().await.unwrap_or_default();
            }
        }
        if osc && fired && started_row && done_row && sessions_live.contains("auto-qa41-") {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let rows = read_file(&runs);
    let out = stdout_text(&daemon);
    println!("\n=== [HAPPY] scheduled fire on the wall clock ===");
    println!("durable journal has scheduled.fire : {fired}");
    println!("run history started row            : {started_row}");
    println!("\n=== [HAPPY] session kicked off (visible on the live API) ===");
    println!(
        "GET /sessions shows auto-qa41-*    : {}",
        sessions_live.contains("auto-qa41-")
    );
    println!("\n=== run history rows (runs.jsonl) ===");
    for line in rows.lines() {
        println!("  {line}");
    }
    println!("\n=== [HAPPY] notifications: OSC 777/9 on daemon stdout ===");
    let osc_line = out
        .lines()
        .find(|l| l.contains("]777;notify;"))
        .unwrap_or("");
    println!("OSC 777/9 bytes captured           : {osc}");
    println!("frame: {osc_line:?}");
    let verdict = rows
        .lines()
        .any(|l| l.contains("\"verdict\":\"confirmed\""));
    println!("gate verdict row present           : {done_row} (confirmed={verdict})");
    println!(
        "\n(disclosed: the turn itself fails with `no provider configured` — \
         provider wiring is todo 40; the SCHEDULER loop, kickoff, gate and \
         notification channels are what this QA proves)"
    );
    let ok = osc && fired && started_row && done_row && sessions_live.contains("auto-qa41-");
    drop(daemon);
    println!("\nQA transcript {}.", if ok { "PASSED" } else { "FAILED" });
    if !ok {
        Err("task-41 QA assertions failed".into())
    } else {
        Ok(())
    }
}
