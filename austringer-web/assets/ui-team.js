// The team grid panel: child cells, budget, and the Warp-precedence
// aggregate.
import { el } from "spa/ui-dom";

// Warp status precedence (todo 15; the Rust source of truth is
// austringer-daemon/src/team.rs — this is its JS port over the same
// child-state vocabulary): InProgress > Blocked > Error > Cancelled >
// Success, plus WaitingForEvents; never terminal while a child runs.
const CHILD_STATUS = {
  success: "success",
  failed: "error",
  quarantined: "cancelled",
  blocked: "blocked",
  collision: "blocked",
};
const STATUS_PRECEDENCE = { in_progress: 5, blocked: 4, error: 3, cancelled: 2, success: 1 };

function teamAggregate(workers) {
  if (workers.length === 0) return "waiting_for_events";
  let best = "success";
  for (const worker of workers) {
    const status = CHILD_STATUS[worker.state] || "in_progress";
    if ((STATUS_PRECEDENCE[status] || 5) > (STATUS_PRECEDENCE[best] || 5)) {
      best = status;
    }
  }
  return best;
}

export function teamPanel(state) {
  const box = el("section", { class: "panel team", "data-testid": "team" });
  if (state.workers.length === 0 && !state.budget && !state.budgetExhausted) {
    return box;
  }
  box.append(el("h2", { text: "agents" }));
  box.append(
    el("p", {
      class: "team-status",
      "data-testid": "team-status",
      text: `status: ${teamAggregate(state.workers)}`,
    }),
  );
  if (state.budget) {
    box.append(
      el("p", {
        class: "hint",
        "data-testid": "team-budget",
        text: `budget: ${state.budget.spawned}/${state.budget.fanOutCap} children | ${state.budget.tokensSpent}/${state.budget.tokenBudget} tokens`,
      }),
    );
  }
  if (state.budgetExhausted) {
    box.append(
      el("span", {
        class: "status status-error",
        text: "budget exhausted — further task spawns refused",
      }),
    );
  }
  for (const worker of state.workers) {
    box.append(
      el("div", { class: "row", "data-child": worker.child }, [
        el("strong", { text: worker.task || worker.child }),
        el("span", { class: "status", text: worker.state }),
        worker.detail ? el("span", { class: "hint", text: worker.detail }) : null,
      ]),
    );
  }
  return box;
}
