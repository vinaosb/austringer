// History panels: trends (turn outcomes, cost/cache/model mix from v2
// Usage frames), scheduled-run history with gate verdicts and evidence
// links, and changed-files-by-turn.
import { el, fmt, fmtUsd } from "spa/ui-dom";

// Trends: turn outcomes plus per-model cost/token mix and cache
// efficiency from the v2 Usage frames. Lives in the session view —
// trends only exist where a session is attached.
export function trendsPanel(state) {
  const panel = el("section", { class: "panel", "data-testid": "eval-trends" }, [
    el("h2", { text: "trends" }),
  ]);
  const outcomes = state.hud.outcomes;
  if (outcomes.length === 0) {
    panel.append(
      el("p", { class: "empty", text: "turn outcomes trend appears once a session runs turns" }),
    );
  } else {
    panel.append(
      el("ul", { class: "trend" }, outcomes.map((o, i) => el("li", { text: `#${i + 1} ${o}` }))),
    );
  }
  const usage = state.usage;
  if (usage.frames === 0) {
    panel.append(
      el("p", { class: "empty", text: "cost / cache / model trends appear as turns report usage (v2 frames)" }),
    );
    return panel;
  }
  const cacheTotal = usage.cacheRead + usage.inputTokens;
  const cacheHit = cacheTotal > 0 ? Math.round((usage.cacheRead / cacheTotal) * 100) : 0;
  panel.append(
    el("p", {
      class: "trend-cost",
      text: `${fmtUsd(usage.totalCostMicros)} across ${usage.frames} usage frames · cache hit ${cacheHit}%`,
    }),
  );
  for (const [label, stat] of Object.entries(usage.models)) {
    panel.append(
      el("div", { class: "row" }, [
        el("span", { class: "heat-label", text: label }),
        el("span", {
          class: "heat-bar",
          "data-count": fmt(stat.frames),
          style: `width:${Math.max(6, Math.min(100, Math.round((stat.frames / usage.frames) * 100)))}%`,
        }, [el("span", { text: `${fmt(stat.frames)}f` })]),
        el("span", { class: "hint", text: fmtUsd(stat.costMicros) }),
      ]),
    );
  }
  return panel;
}

// Session history: scheduled-run rows from GET /s/{sid}/history,
// per-session cost/model totals from the v2 usage frames, gate
// verdicts with evidence links into the replay log, and the asks
// answered during this attach.
export function historyPanel(state, props, actions) {
  const box = el("section", { class: "panel", "data-testid": "history" }, [
    el("h2", { text: "history" }),
  ]);
  const usage = state.usage;
  box.append(
    el("p", {
      "data-testid": "history-cost",
      text:
        usage.frames === 0
          ? "cost: no usage frames yet — totals appear as turns run"
          : `${fmtUsd(usage.totalCostMicros)} · ${usage.frames} usage frames · in ${fmt(usage.inputTokens)} out ${fmt(usage.outputTokens)} tokens`,
    }),
  );
  if (usage.frames > 0) {
    box.append(
      el("p", {
        class: "hint",
        text: `models: ${Object.entries(usage.models)
          .map(([label, stat]) => `${label} (${stat.frames}f)`)
          .join(", ")}`,
      }),
    );
  }
  for (const verdict of state.verdicts) {
    const evidence = verdict.evidence.map((id) => {
      const idx = state.frames.findIndex((f) => f.id === id);
      return idx >= 0
        ? el("button", {
            class: "evidence-link",
            text: `#${id}`,
            onclick: () => actions.replaySeek(idx),
          })
        : el("span", { class: "hint", text: `#${id}` });
    });
    box.append(
      el("div", { class: "row", "data-testid": "history-verdict" }, [
        el("span", {
          class: `status verdict-${verdict.verdict}`,
          text: verdict.verdict,
        }),
        el("span", { class: "hint", text: `turn ${verdict.turn}` }),
        el("span", { class: "hint", text: "evidence:" }),
        ...evidence,
      ]),
    );
  }
  if (!props.historyLoaded) {
    box.append(el("p", { class: "hint", text: "loading run history…" }));
  } else if (!props.history || props.history.length === 0) {
    box.append(
      el("p", {
        class: "empty",
        "data-testid": "history-empty",
        text: "no scheduled runs recorded for this session",
      }),
    );
  } else {
    for (const run of props.history) {
      box.append(
        el("div", { class: "row", "data-testid": "history-run" }, [
          el("strong", { text: run.automation_id || "automation" }),
          el("span", { class: "hint", text: run.outcome || "" }),
          run.verdict
            ? el("span", { class: `status verdict-${run.verdict}`, text: run.verdict })
            : null,
          el("span", { class: "hint", text: `fired ${fmt(run.fired_at_minute)}` }),
          el("span", { class: "hint", text: `net ${fmt(run.network || "")}` }),
        ]),
      );
    }
  }
  if (state.resolvedLog.length > 0) {
    const log = el("div", { "data-testid": "resolved-log" }, [
      el("h3", { text: "asks answered" }),
    ]);
    for (const entry of state.resolvedLog) {
      const resolution = entry.resolution || {};
      const kind =
        typeof resolution === "string" ? resolution : resolution.type || Object.keys(resolution)[0] || "?";
      const by = typeof resolution === "object" ? resolution.by || "" : "";
      log.append(el("p", { class: "hint", text: `ask ${entry.askId}: ${kind}${by ? ` by ${by}` : ""}` }));
    }
    box.append(log);
  }
  return box;
}

// Changed-files-by-turn (receipts: OpenCode #55064 Changed-files-by-turn;
// Devin Session Insights): diff events grouped under the turn that
// produced them.
export function timelinePanel(state) {
  const box = el("section", { class: "panel", "data-testid": "timeline" }, [
    el("h2", { text: "changed files by turn" }),
  ]);
  if (state.timeline.length === 0) {
    box.append(
      el("p", { class: "empty", text: "no changed files yet — diffs land here grouped by turn" }),
    );
    return box;
  }
  const turns = new Map();
  for (const entry of state.timeline) {
    const files = turns.get(entry.turn) || [];
    if (!files.includes(entry.path)) files.push(entry.path);
    turns.set(entry.turn, files);
  }
  for (const [turn, files] of turns) {
    box.append(
      el("div", { class: "row", "data-testid": "timeline-turn" }, [
        el("strong", { text: `turn ${turn}` }),
        el("code", { text: files.join(" · ") }),
      ]),
    );
  }
  return box;
}
