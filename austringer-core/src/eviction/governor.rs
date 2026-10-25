//! Two-tier per-class call governors (plan todo 12, research §33).
//!
//! Budgets key on the task classifier's [`TaskClass`] — NEVER one global
//! cap: correct budgets spread ~10× across classes (light chat vs deep
//! research), so the shipped defaults do exactly that. The SOFT tier
//! surfaces a typed notice injected into context (the model self-ration);
//! the HARD tier DENIES further calls with a structured error the model
//! reads and can act on. The classifier is a seam: the keyword heuristic
//! ships now, the real classifier arrives with model profiles.

use std::collections::BTreeMap;
use std::fmt;

/// The task classes budgets key on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskClass {
    /// Quick questions, small talk: cheapest budgets.
    LightChat,
    /// Local edit/test loops: mid budgets.
    CodeEdit,
    /// Multi-source exploration and synthesis: ~10× LightChat.
    DeepResearch,
}

impl TaskClass {
    /// Every shipped class.
    pub const ALL: [TaskClass; 3] = [
        TaskClass::LightChat,
        TaskClass::CodeEdit,
        TaskClass::DeepResearch,
    ];
}

impl fmt::Display for TaskClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::LightChat => "light_chat",
            Self::CodeEdit => "code_edit",
            Self::DeepResearch => "deep_research",
        };
        formatter.write_str(name)
    }
}

/// One class's two-tier budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassBudget {
    /// Calls before the typed notice appears in context.
    pub soft: u32,
    /// Calls before further calls are denied with a structured error.
    pub hard: u32,
}

/// Per-class budget table. There is deliberately NO global fallback:
/// an unbudgeted class is a lookup miss, surfaced by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernorConfig {
    /// The per-class table (every shipped class budgeted by default).
    pub per_class: BTreeMap<TaskClass, ClassBudget>,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        let mut per_class = BTreeMap::new();
        per_class.insert(TaskClass::LightChat, ClassBudget { soft: 10, hard: 15 });
        per_class.insert(TaskClass::CodeEdit, ClassBudget { soft: 30, hard: 40 });
        per_class.insert(
            TaskClass::DeepResearch,
            ClassBudget {
                soft: 100,
                hard: 120,
            },
        );
        Self { per_class }
    }
}

impl GovernorConfig {
    /// The budget for `class`, when budgeted.
    #[must_use]
    pub fn budget_for(&self, class: TaskClass) -> Option<ClassBudget> {
        self.per_class.get(&class).copied()
    }
}

/// The task-classifier seam (real classifier arrives with profiles).
pub trait TaskClassifier: Send + Sync {
    /// Classifies a prompt.
    fn classify(&self, prompt: &str) -> TaskClass;
}

/// The shipping keyword heuristic: research verbs → DeepResearch, edit
/// verbs → CodeEdit, everything else (including empty) → LightChat.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeywordClassifier;

impl TaskClassifier for KeywordClassifier {
    fn classify(&self, prompt: &str) -> TaskClass {
        let lowered = prompt.to_ascii_lowercase();
        let hits = |needles: &[&str]| needles.iter().any(|needle| lowered.contains(needle));
        if hits(&["research", "compare", "benchmark", "survey", "investigate"]) {
            TaskClass::DeepResearch
        } else if hits(&["edit", "fix", "test", "refactor", "file"]) {
            TaskClass::CodeEdit
        } else {
            TaskClass::LightChat
        }
    }
}

/// The typed soft-budget notice injected into context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernorNotice {
    /// The class whose budget is running out.
    pub class: TaskClass,
    /// Calls used so far.
    pub used: u32,
    /// The class's soft budget.
    pub soft: u32,
    /// The class's hard budget.
    pub hard: u32,
}

impl GovernorNotice {
    /// The deterministic structured notice the model reads.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "<call_budget_notice class=\"{}\" used=\"{}\" soft=\"{}\" hard=\"{}\" strategy=\"conserve-calls\">call budget for this task class is nearly exhausted; prefer finishing with the tools already invoked</call_budget_notice>",
            self.class, self.used, self.soft, self.hard
        )
    }
}

/// The structured hard-budget denial the model reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallDenial {
    /// The tool that was refused.
    pub tool: String,
    /// The class whose hard budget is exhausted.
    pub class: TaskClass,
    /// Calls used (== hard).
    pub used: u32,
    /// The class's hard budget.
    pub hard: u32,
}

impl CallDenial {
    /// The deterministic structured denial: what happened and what to do.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "<call_denied tool=\"{}\" class=\"{}\" used=\"{}\" hard=\"{}\">hard call budget exhausted for this task class; stop calling tools and answer with what you have</call_denied>",
            self.tool, self.class, self.used, self.hard
        )
    }
}

/// One governor verdict for a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernorVerdict {
    /// Within budget.
    Allow,
    /// Soft budget crossed: allow, but inject the notice.
    AllowWithNotice(GovernorNotice),
    /// Hard budget exhausted: deny with the structured error.
    Denied(CallDenial),
}

/// The per-class call governor: counts calls, returns verdicts.
#[derive(Debug, Clone)]
pub struct CallGovernor {
    config: GovernorConfig,
    class: TaskClass,
    used: u32,
}

impl CallGovernor {
    /// A governor for `class` under `config`.
    #[must_use]
    pub fn new(config: GovernorConfig, class: TaskClass) -> Self {
        Self {
            config,
            class,
            used: 0,
        }
    }

    /// Records one executed call against the class budget.
    pub fn record_call(&mut self) {
        self.used += 1;
    }

    /// Calls used so far.
    #[must_use]
    pub const fn used(&self) -> u32 {
        self.used
    }

    /// The verdict for a further call of `tool`.
    #[must_use]
    pub fn check(&self, tool: &str) -> GovernorVerdict {
        let Some(budget) = self.config.budget_for(self.class) else {
            return GovernorVerdict::Allow;
        };
        if self.used >= budget.hard {
            return GovernorVerdict::Denied(CallDenial {
                tool: tool.to_owned(),
                class: self.class,
                used: self.used,
                hard: budget.hard,
            });
        }
        if self.used >= budget.soft {
            return GovernorVerdict::AllowWithNotice(GovernorNotice {
                class: self.class,
                used: self.used,
                soft: budget.soft,
                hard: budget.hard,
            });
        }
        GovernorVerdict::Allow
    }
}
