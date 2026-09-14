//! The launch-spec format: a strict, bounded, line-oriented dialect that
//! decomposes into objectives, a task graph, and per-task capability +
//! model requirements (draft SS).
//!
//! Grammar (fail closed; unknown keys are typed errors):
//!
//! ```text
//! objective: <text>              # repeatable, required
//! validators: on|off             # mission-validator milestones
//! phases: on|off                 # /boost-style 3-phase decouple
//! budget:
//!   children: <u32>
//!   parallel: <u32>
//!   tokens: <u64>
//! role <name>:
//!   model: <model[effort=high,context=300k]>
//!   tier: explorer|worker|deep
//!   tools: comma,separated
//!   summary: <bytes>
//! task <id>:
//!   role: <name>                 # builtin or declared above
//!   objective: <text>
//!   model: <bracket>             # optional per-task override
//!   writes: comma,globs
//!   needs: comma,ids
//!   class: fact_find|comparison|complex
//! ```
//!
//! Bounded by construction: flat two-level structure (no recursion), line
//! and byte caps, field-length caps, task/role count caps.

use harness_intel::elicitation::TaskClass;

use crate::role::Tier;

mod fields;

use fields::{apply_budget, apply_role, apply_task, apply_top_level, check_field};

/// Hard line cap for one spec.
pub const MAX_SPEC_LINES: usize = 512;
/// Hard byte cap for one spec.
pub const MAX_SPEC_BYTES: usize = 256 * 1024;
/// Hard cap on declared tasks (parallel to the budget guards).
pub const MAX_TASKS: usize = 32;
/// Hard cap on declared roles.
pub const MAX_ROLES: usize = 16;
/// Hard cap on any single field value.
pub const MAX_FIELD_BYTES: usize = 512;

/// A parsed launch spec.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchSpec {
    /// The launch objectives (one per `objective:` line).
    pub objectives: Vec<String>,
    /// Declared budget (any field may default).
    pub budget: SpecBudget,
    /// Custom role declarations.
    pub roles: Vec<RoleDecl>,
    /// The task graph (flat; dependencies via `needs`).
    pub tasks: Vec<TaskDecl>,
    /// Mission-validator milestones on/off.
    pub validators: bool,
    /// /boost-style phase decoupling on/off.
    pub phases: bool,
}

/// Declared budget; `None` fields take the launcher defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpecBudget {
    /// Maximum children.
    pub children: Option<u32>,
    /// Maximum concurrent children.
    pub parallel: Option<u32>,
    /// Total token budget.
    pub tokens: Option<u64>,
}

/// One custom role declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleDecl {
    /// Role name referenced by tasks.
    pub name: String,
    /// Model bracket (optional; tier default applies when absent).
    pub model: Option<String>,
    /// Capability tier.
    pub tier: Option<Tier>,
    /// Execution tool allowlist (fork narrowing).
    pub tools: Vec<String>,
    /// Summary budget in bytes.
    pub summary: Option<usize>,
}

/// One task declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDecl {
    /// Task id (unique; also the resumable child id suffix).
    pub id: String,
    /// Role name.
    pub role: String,
    /// Task objective.
    pub objective: String,
    /// Per-task model bracket override.
    pub model: Option<String>,
    /// Write-set globs (empty = read-only).
    pub writes: Vec<String>,
    /// Dependency task ids.
    pub needs: Vec<String>,
    /// Workload class.
    pub class: TaskClass,
}

/// Typed parse failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecError {
    /// Input exceeded a hard bound.
    #[error("spec too large (lines/bytes over the cap)")]
    TooLarge {
        /// Observed line count.
        lines: usize,
        /// The cap.
        limit: usize,
    },
    /// Task count over the cap.
    #[error("too many tasks")]
    TooManyTasks {
        /// Observed count.
        count: usize,
        /// The cap.
        max: usize,
    },
    /// Role count over the cap.
    #[error("too many roles")]
    TooManyRoles {
        /// Observed count.
        count: usize,
        /// The cap.
        max: usize,
    },
    /// A key not in the grammar.
    #[error("unknown key `{key}` at line {line_no}")]
    UnknownKey {
        /// Line number (1-based).
        line_no: usize,
        /// The offending key.
        key: String,
    },
    /// A malformed header line.
    #[error("bad header at line {line_no}")]
    BadHeader {
        /// Line number (1-based).
        line_no: usize,
    },
    /// A duplicate task or role id.
    #[error("duplicate {kind} `{id}`")]
    Duplicate {
        /// "task" or "role".
        kind: &'static str,
        /// The duplicated id.
        id: String,
    },
    /// A required field is empty or missing.
    #[error("empty field `{key}` at line {line_no}")]
    EmptyField {
        /// Line number (1-based).
        line_no: usize,
        /// The missing key.
        key: &'static str,
    },
    /// No `objective:` line.
    #[error("spec has no objective")]
    MissingObjective,
    /// No tasks.
    #[error("spec has no tasks")]
    NoTasks,
    /// A value failed validation.
    #[error("bad value for `{key}` at line {line_no}: {value}")]
    BadValue {
        /// Line number (1-based).
        line_no: usize,
        /// The key.
        key: String,
        /// The rejected value.
        value: String,
    },
    /// A boolean field was not on/off.
    #[error("bad on/off value `{value}` at line {line_no}")]
    BadBool {
        /// Line number (1-based).
        line_no: usize,
        /// The rejected value.
        value: String,
    },
}

/// One grammar line, split into its key and value.
struct Field<'a> {
    key: &'a str,
    value: &'a str,
}

fn split_field(line: &str) -> Option<Field<'_>> {
    let (key, value) = line.split_once(':')?;
    Some(Field {
        key: key.trim(),
        value: value.trim(),
    })
}

fn has_control(value: &str) -> bool {
    value
        .chars()
        .any(|c| (c.is_control() && c != ' ') || c == '\u{7f}')
}

enum Section {
    None,
    Budget,
    Role(usize),
    Task(usize),
}

/// Parses a launch spec. Bounded: total bytes, line count, field sizes,
/// and structure depth are all capped before any allocation-heavy work.
pub fn parse_spec(text: &str) -> Result<LaunchSpec, SpecError> {
    if text.len() > MAX_SPEC_BYTES {
        return Err(SpecError::TooLarge {
            lines: text.len(),
            limit: MAX_SPEC_BYTES,
        });
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_SPEC_LINES {
        return Err(SpecError::TooLarge {
            lines: lines.len(),
            limit: MAX_SPEC_LINES,
        });
    }

    let mut objectives: Vec<String> = Vec::new();
    let mut budget = SpecBudget::default();
    let mut roles: Vec<RoleDecl> = Vec::new();
    let mut tasks: Vec<TaskDecl> = Vec::new();
    let mut validators = true;
    let mut phases = false;
    let mut section = Section::None;

    for (index, raw) in lines.iter().enumerate() {
        let line_no = index + 1;
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indented = line.starts_with(char::is_whitespace);
        let value = check_field(trimmed, line_no)?;

        if !indented {
            section = Section::None;
            if trimmed == "budget:" {
                section = Section::Budget;
                continue;
            }
            if let Some(header_id) = trimmed.strip_prefix("task ") {
                let id = header_id
                    .strip_suffix(':')
                    .unwrap_or(header_id)
                    .trim()
                    .to_string();
                if id.is_empty() || id.contains(' ') {
                    return Err(SpecError::BadHeader { line_no });
                }
                if tasks.iter().any(|t| t.id == id) {
                    return Err(SpecError::Duplicate {
                        kind: "task",
                        id: id.clone(),
                    });
                }
                if tasks.len() >= MAX_TASKS {
                    return Err(SpecError::TooManyTasks {
                        count: tasks.len() + 1,
                        max: MAX_TASKS,
                    });
                }
                tasks.push(TaskDecl {
                    id,
                    role: String::new(),
                    objective: String::new(),
                    model: None,
                    writes: Vec::new(),
                    needs: Vec::new(),
                    class: TaskClass::Complex,
                });
                section = Section::Task(tasks.len() - 1);
                continue;
            }
            if let Some(header_name) = trimmed.strip_prefix("role ") {
                let name = header_name
                    .strip_suffix(':')
                    .unwrap_or(header_name)
                    .trim()
                    .to_string();
                if name.is_empty() || name.contains(' ') {
                    return Err(SpecError::BadHeader { line_no });
                }
                if roles.iter().any(|r| r.name == name) {
                    return Err(SpecError::Duplicate {
                        kind: "role",
                        id: name.clone(),
                    });
                }
                if roles.len() >= MAX_ROLES {
                    return Err(SpecError::TooManyRoles {
                        count: roles.len() + 1,
                        max: MAX_ROLES,
                    });
                }
                roles.push(RoleDecl {
                    name,
                    model: None,
                    tier: None,
                    tools: Vec::new(),
                    summary: None,
                });
                section = Section::Role(roles.len() - 1);
                continue;
            }
            apply_top_level(
                value,
                line_no,
                &mut objectives,
                &mut validators,
                &mut phases,
            )?;
            continue;
        }

        match &mut section {
            Section::None => Err(SpecError::BadHeader { line_no }),
            Section::Budget => apply_budget(value, line_no, &mut budget),
            Section::Role(i) => apply_role(value, line_no, &mut roles[*i]),
            Section::Task(i) => apply_task(value, line_no, &mut tasks[*i]),
        }?;
    }

    if tasks.is_empty() {
        return Err(SpecError::NoTasks);
    }
    if objectives.is_empty() {
        return Err(SpecError::MissingObjective);
    }
    for task in &tasks {
        if task.role.is_empty() {
            return Err(SpecError::EmptyField {
                line_no: 0,
                key: "role",
            });
        }
        if task.objective.is_empty() {
            return Err(SpecError::EmptyField {
                line_no: 0,
                key: "objective",
            });
        }
    }
    Ok(LaunchSpec {
        objectives,
        budget,
        roles,
        tasks,
        validators,
        phases,
    })
}
