//! The named pipeline operations, in their binding order.

/// One stage of the agent-loop pipeline.
///
/// The pipeline runs these in [`Operation::ALL`] order, once per iteration;
/// the order is a stability contract of the loop, not an implementation
/// detail. Later todos implement the currently no-op stages
/// (Compaction = todo 13, ToolApproval inspectors = todo 7, hooks = todo 28)
/// behind the same operation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// Per-iteration entry hook (user lifecycle hooks, todo 28).
    EntryHook,
    /// Handles queued slash commands before they become conversation.
    SlashCommand,
    /// Absorbs queued user input into the conversation (mid-run steering).
    Steer,
    /// Enforces the per-run turn budget.
    MaxTurns,
    /// History condensation (todo 13).
    Compaction,
    /// Folds old tool-call/tool-result pairs (todo 12).
    ToolPairCompaction,
    /// Inspector chain over pending tool calls (todo 7).
    ToolApproval,
    /// Executes approved tool calls.
    ToolExecution,
    /// Decides whether a failed inference is retried.
    Retry,
    /// May block turn exit up to the configured cap (todo 28).
    StopHook,
    /// Decides session fate after an unretried error.
    ExitOnError,
    /// Produces the assistant turn (the todo-9 provider seam).
    Inference,
}

impl Operation {
    /// The pipeline order. Binding: do not reorder.
    pub const ALL: [Self; 12] = [
        Self::EntryHook,
        Self::SlashCommand,
        Self::Steer,
        Self::MaxTurns,
        Self::Compaction,
        Self::ToolPairCompaction,
        Self::ToolApproval,
        Self::ToolExecution,
        Self::Retry,
        Self::StopHook,
        Self::ExitOnError,
        Self::Inference,
    ];

    /// Stable identifier used in cancel marks and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::EntryHook => "EntryHook",
            Self::SlashCommand => "SlashCommand",
            Self::Steer => "Steer",
            Self::MaxTurns => "MaxTurns",
            Self::Compaction => "Compaction",
            Self::ToolPairCompaction => "ToolPairCompaction",
            Self::ToolApproval => "ToolApproval",
            Self::ToolExecution => "ToolExecution",
            Self::Retry => "Retry",
            Self::StopHook => "StopHook",
            Self::ExitOnError => "ExitOnError",
            Self::Inference => "Inference",
        }
    }
}
