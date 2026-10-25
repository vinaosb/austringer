// DOM rendering for the austringer web SPA (todo 37). Security rule:
// untrusted text (events, tool output, share payloads) reaches the
// document ONLY through textContent — never innerHTML. The module is
// dumb: it renders (state, props, actions) and calls actions back.
export function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || value === null) continue;
    if (key === "class") node.className = value;
    else if (key === "text") node.textContent = String(value);
    else if (key.startsWith("on") && typeof value === "function") {
      node.addEventListener(key.slice(2), value);
    } else if (key === "disabled") {
      node.disabled = Boolean(value);
    } else {
      node.setAttribute(key, String(value));
    }
  }
  for (const child of children) {
    if (child === null || child === undefined) continue;
    node.append(child);
  }
  return node;
}

const fmt = (n) => String(n);

const fmtUsd = (micros) => `$${(micros / 1e6).toFixed(6)}`;

export function render(root, view, state, props, actions) {
  root.replaceChildren();
  if (view === "share") {
    root.append(shareView(props.share));
    return;
  }
  if (!props.token || state.unauthorized) {
    root.append(tokenGate(state, props, actions));
    return;
  }
  root.append(statusBanner(state, props));
  const main =
    view === "session"
      ? sessionView(state, props, actions)
      : dashboardView(state, props, actions);
  root.append(main);
}

function tokenGate(state, props, actions) {
  return el("section", { class: "gate", "data-testid": "token-gate" }, [
    el("h1", { text: "austringer" }),
    el(
      "p",
      { text: state.unauthorized ? "daemon rejected the token — paste a valid one" : "daemon token required" },
    ),
    el("form", {
      onsubmit: (ev) => {
        ev.preventDefault();
        actions.setToken(ev.target.elements.token.value.trim());
      },
    }, [
      el("input", {
        name: "token",
        autocomplete: "off",
        placeholder: "paste daemon token",
        value: "",
        autofocus: true,
      }),
      el("button", { type: "submit", text: "attach" }),
    ]),
    el("p", { class: "hint", text: "the daemon prints the token on startup and stores it under its data dir" }),
  ]);
}

function statusBanner(state, props) {
  const bits = [
    el("strong", { text: `austringer · ${props.session || "dashboard"}` }),
    el("span", { class: `status status-${props.status}`, text: props.status }),
  ];
  if (state.hello && state.hello.version) {
    bits.push(
      el("span", {
        class: "status",
        "data-testid": "proto-version",
        text: `v${state.hello.version}`,
      }),
    );
  }
  if (props.transport) {
    bits.push(el("span", { class: "transport", text: props.transport }));
  }
  if (state.closedBy !== null) {
    bits.push(el("span", { class: "status status-closed", text: `closed: ${state.closedBy}` }));
  }
  if (state.serverErrors.length > 0) {
    const last = state.serverErrors[state.serverErrors.length - 1];
    bits.push(el("span", { class: "status status-error", text: `${last.code}: ${last.message}` }));
  }
  if (state.hud.anomalies > 0) {
    bits.push(el("span", { class: "anomalies", text: `anomalies: ${state.hud.anomalies}` }));
  }
  return el("header", { class: "banner" }, bits);
}

// ---- Dashboard (route "/") -------------------------------------------

function dashboardView(state, props, actions) {
  const sessions = el("section", { class: "panel", "data-testid": "sessions" }, [
    el("h2", { text: "sessions" }),
  ]);
  if (props.sessions.length === 0) {
    sessions.append(el("p", { class: "empty", text: "no sessions reported by the daemon yet" }));
  }
  for (const id of props.sessions) {
    sessions.append(
      el("div", { class: "row" }, [
        el("a", { href: `/s/${encodeURIComponent(id)}`, text: id }),
        el("button", { text: "open", onclick: () => actions.openSession(id) }),
      ]),
    );
  }
  sessions.append(
    el("form", {
      class: "row",
      onsubmit: (ev) => {
        ev.preventDefault();
        const id = ev.target.elements.sid.value.trim();
        if (id) actions.openSession(id);
      },
    }, [
      el("input", { name: "sid", placeholder: "session id", autocomplete: "off" }),
      el("button", { type: "submit", text: "deep-link" }),
    ]),
  );

  return el("main", { class: "dashboard" }, [
    sessions,
    toolHeatmap(state),
    packsPanel(state, props),
  ]);
}

function toolHeatmap(state) {
  const panel = el("section", { class: "panel", "data-testid": "tool-heatmap" }, [
    el("h2", { text: "tool heatmap" }),
  ]);
  const names = Object.keys(state.hud.toolCalls);
  if (names.length === 0) {
    panel.append(el("p", { class: "empty", text: "attach to a session to collect tool calls" }));
    return panel;
  }
  const max = Math.max(...Object.values(state.hud.toolCalls));
  for (const name of names) {
    const count = state.hud.toolCalls[name];
    panel.append(
      el("div", { class: "heat-row" }, [
        el("span", { class: "heat-label", text: name }),
        el("span", {
          class: "heat-bar",
          "data-count": fmt(count),
          style: `width:${Math.max(6, Math.round((count / max) * 100))}%`,
        }, [el("span", { text: fmt(count) })]),
      ]),
    );
  }
  return panel;
}

// Trends: turn outcomes plus per-model cost/token mix and cache
// efficiency from the v2 Usage frames. Lives in the session view —
// trends only exist where a session is attached.
function trendsPanel(state) {
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
function historyPanel(state, props, actions) {
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
function timelinePanel(state) {
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

function packsPanel(state, props) {
  const panel = el("section", { class: "panel", "data-testid": "pack-manager" }, [
    el("h2", { text: "packs" }),
  ]);
  const packs = props.packs || [];
  if (packs.length === 0) {
    panel.append(el("p", { class: "empty", text: "no skill packs reported by the daemon yet" }));
    return panel;
  }
  for (const pack of packs) {
    const trusted = pack.trusted
      ? el("span", { class: "status status-open", text: `${pack.tier} · trusted` })
      : el("span", { class: "status status-closed", text: `${pack.tier} · quarantined` });
    panel.append(
      el("div", { class: "row", "data-pack": pack.name }, [
        el("strong", { text: pack.name }),
        trusted,
        el("span", { class: "hint", text: pack.description || "" }),
      ]),
    );
  }
  const servers = props.mcpServers || [];
  for (const server of servers) {
    panel.append(
      el("div", { class: "row", "data-mcp-server": server.server }, [
        el("span", { text: `mcp: ${server.server}` }),
        server.disabled
          ? el("span", { class: "status status-closed", text: "disabled — explicit consent required" })
          : el("span", { class: "status status-open", text: "enabled" }),
      ]),
    );
  }
  return panel;
}

// ---- Session view (route "/s/{sid}") ---------------------------------

function sessionView(state, props, actions) {
  return el("main", { class: "session" }, [
    hud(state),
    gates(state, actions),
    teamPanel(state),
    historyPanel(state, props, actions),
    timelinePanel(state),
    trendsPanel(state),
    diffs(state, actions),
    transcript(state, props, actions),
    composer(state, props, actions),
    replay(state, props, actions),
    shareBox(state, actions),
  ]);
}

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

function teamPanel(state) {
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

function hud(state) {
  const { hud } = state;
  return el("section", { class: "hud", "data-testid": "hud" }, [
    stat("turns", hud.turns),
    stat("events", hud.events),
    stat("context (llm-visible)", hud.llmVisible),
    stat("tool results", hud.results),
    stat("resyncs", hud.resyncs),
    stat("last event id", state.lastEventId),
  ]);
}

function stat(label, value) {
  return el("div", { class: "stat" }, [
    el("span", { class: "stat-value", "data-testid": `stat-${label}`, text: fmt(value) }),
    el("span", { class: "stat-label", text: label }),
  ]);
}

const identityLabel = (identity) =>
  identity && typeof identity === "object"
    ? `${identity.provider || "?"}/${identity.model || "?"}`
    : "?/?";

function gates(state, actions) {
  const box = el("section", { class: "gates", "data-testid": "gates" });
  for (const ask of Object.values(state.asks)) {
    box.append(
      el("div", { class: "gate-ask", "data-ask": fmt(ask.ask_id) }, [
        el("strong", { text: `permission: ${ask.tool}` }),
        ask.target ? el("code", { text: ask.target }) : null,
        el("p", { text: ask.reason }),
        el("div", { class: "gate-actions" }, [
          el("button", { text: "y (once)", onclick: () => actions.answer(ask.ask_id, "allow_once") }),
          el("button", { text: "a (session)", onclick: () => actions.answer(ask.ask_id, "allow_session") }),
          el("button", { text: "n (deny)", onclick: () => actions.answer(ask.ask_id, "deny") }),
          el("button", { text: "vote-cancel", onclick: () => actions.cancelVote(ask.ask_id) }),
        ]),
      ]),
    );
  }
  // Model-fallback asks reuse the gates surface: keep the current
  // model or acknowledge the proposed hop — any other resolution
  // retries the SAME model, never a silent substitution.
  for (const ask of Object.values(state.fallbackAsks)) {
    box.append(
      el("div", { class: "gate-ask fallback", "data-testid": "fallback-ask", "data-ask": fmt(ask.askId) }, [
        el("strong", { text: "model fallback proposed" }),
        el("p", {}, [
          el("code", { text: identityLabel(ask.current) }),
          el("span", { text: " → " }),
          el("code", { text: identityLabel(ask.proposed) }),
        ]),
        el("p", { text: ask.reason }),
        el("div", { class: "gate-actions" }, [
          el("button", { text: "keep current", onclick: () => actions.answerFallback(ask.askId, ask.current) }),
          el("button", { text: "switch to proposed", onclick: () => actions.answerFallback(ask.askId, ask.proposed) }),
        ]),
      ]),
    );
  }
  // Plan-approval asks: approve ends plan mode and re-arms writes per
  // the autonomy matrix; reject keeps plan mode active — nothing ever
  // auto-approves a plan.
  for (const ask of Object.values(state.planAsks)) {
    const pre = el("pre", { class: "hunks plan-artifact" });
    pre.textContent = ask.artifact;
    box.append(
      el("div", { class: "gate-ask plan", "data-testid": "plan-ask", "data-ask": fmt(ask.askId) }, [
        el("strong", { text: `plan approval · revision ${ask.revision}` }),
        pre,
        el("div", { class: "gate-actions" }, [
          el("button", { text: "approve", onclick: () => actions.answerPlan(ask.askId, "approve") }),
          el("button", { text: "reject", onclick: () => actions.answerPlan(ask.askId, "reject") }),
        ]),
      ]),
    );
  }
  return box;
}

// Semantic-hunk review: y/a/n answer the ask that produced the diff
// (first-responder mediation); "e" steers an edit note.
function diffs(state, actions) {
  const box = el("section", { class: "diffs", "data-testid": "diffs" });
  if (state.diffs.length === 0) return box;
  box.append(el("h2", { text: "hunks" }));
  for (const diff of state.diffs.slice(-10)) {
    const pre = el("pre", { class: "hunks" });
    pre.textContent = diff.hunks.join("\n");
    box.append(
      el("div", { class: "diff", "data-path": diff.path }, [
        el("code", { text: diff.path }),
        pre,
        el("div", { class: "gate-actions" }, [
          el("button", { text: "e (edit-note)", onclick: () => actions.steer(`review hunks of ${diff.path}:`) }),
        ]),
      ]),
    );
  }
  return box;
}

const PREVIEW_LIMIT = 140;

// Collapsed-by-default transcript (receipt: Claude /focus; the
// 5-project convergence on one-line-per-entry): each entry renders a
// single-line preview until its focus control expands it.
function transcript(state, props, actions) {
  const box = el("section", { class: "transcript", "data-testid": "transcript" });
  for (const entry of state.transcript) {
    const open = props.expanded.has(entry.seq);
    const text = entry.text || "";
    const singleLine = text.split("\n")[0] || "";
    const preview = open || singleLine.length <= PREVIEW_LIMIT
      ? singleLine
      : `${singleLine.slice(0, PREVIEW_LIMIT)}…`;
    box.append(
      el(
        "article",
        {
          class: `entry entry-${entry.role} ${open ? "entry-open" : "entry-collapsed"}`,
          "data-role": entry.role,
          "data-testid": "transcript-entry",
          "data-seq": fmt(entry.seq),
        },
        [
          el("span", { class: "entry-role", text: entry.tool ? `${entry.role}:${entry.tool}` : entry.role }),
          el("p", { class: "entry-text", text: preview }),
          el("button", {
            class: "focus-toggle",
            text: open ? "collapse" : "focus",
            onclick: () => actions.toggleFocus(entry.seq),
          }),
          open && text !== preview
            ? (() => {
                const pre = el("pre", { class: "entry-full" });
                pre.textContent = text;
                return pre;
              })()
            : null,
        ],
      ),
    );
  }
  if (state.transcript.length === 0) {
    box.append(el("p", { class: "empty", text: "no events yet — submit a turn below" }));
  }
  return box;
}

function composer(state, props, actions) {
  const previews = el("div", { class: "previews", "data-testid": "image-previews" });
  const file = el("input", { type: "file", accept: "image/*", multiple: true, class: "file-input" });
  const text = el("textarea", {
    placeholder: "submit a turn… (paste or drop images to attach)",
    "data-testid": "composer",
  });
  const images = [];
  const addImages = (files) => {
    for (const f of files) {
      if (!f.type.startsWith("image/") || f.size > 2 * 1024 * 1024) continue;
      const reader = new FileReader();
      reader.addEventListener("load", () => {
        images.push({ name: f.name, mime: f.type, data: String(reader.result) });
        previews.append(
          el("figure", { class: "preview" }, [
            el("img", { src: String(reader.result), alt: f.name }),
            el("figcaption", { text: f.name }),
          ]),
        );
      });
      reader.readAsDataURL(f);
    }
  };
  file.addEventListener("change", () => addImages(file.files));
  text.addEventListener("paste", (ev) => {
    const items = Array.from(ev.clipboardData?.items || []);
    addImages(items.filter((i) => i.kind === "file").map((i) => i.getAsFile()).filter(Boolean));
  });
  text.addEventListener("drop", (ev) => {
    ev.preventDefault();
    addImages(ev.dataTransfer?.files || []);
  });
  const form = el("form", {
    class: "composer",
    onsubmit: (ev) => {
      ev.preventDefault();
      actions.input(text.value);
      text.value = "";
      images.length = 0;
      previews.replaceChildren();
    },
  }, [
    previews,
    text,
    el("div", { class: "composer-actions" }, [
      file,
      el("button", { type: "button", text: "steer", onclick: () => actions.steer(text.value) }),
      el("button", { type: "button", text: "cancel turn", onclick: () => actions.cancel() }),
      el("button", { type: "submit", text: "submit" }),
    ]),
    props.feedback
      ? (() => {
          const pre = el("pre", { class: "hint", "data-testid": "command-feedback" });
          pre.textContent = props.feedback;
          return pre;
        })()
      : null,
    el("p", {
      class: "hint",
      text: "images preview locally; submits carry text only (the v2 media upload endpoint is disclosed for the daemon)",
    }),
  ]);
  return form;
}

// Replay debugging: step through the captured frame log.
function replay(state, props, actions) {
  const panel = el("section", { class: "panel replay", "data-testid": "replay" }, [
    el("h2", { text: "replay" }),
  ]);
  const frames = state.frames;
  if (frames.length === 0) {
    panel.append(el("p", { class: "empty", text: "no frames captured yet" }));
    return panel;
  }
  const idx = Math.min(props.replayIdx ?? frames.length - 1, frames.length - 1);
  const pre = el("pre", { class: "frame" });
  pre.textContent = JSON.stringify(frames[idx], null, 1);
  panel.append(
    el("div", { class: "gate-actions" }, [
      el("button", { text: "⏮", onclick: () => actions.replaySeek(0) }),
      el("button", { text: "◀", onclick: () => actions.replaySeek(Math.max(0, idx - 1)) }),
      el("span", { class: "replay-pos", text: `${idx + 1}/${frames.length}` }),
      el("button", { text: "▶", onclick: () => actions.replaySeek(Math.min(frames.length - 1, idx + 1)) }),
      el("button", { text: "⏭", onclick: () => actions.replaySeek(frames.length - 1) }),
    ]),
    pre,
  );
  return panel;
}

function shareBox(state, actions) {
  return el("section", { class: "panel", "data-testid": "share" }, [
    el("h2", { text: "share" }),
    el("p", { class: "hint", text: "redacted share link with a 10-minute expiry (client-side; no cloud)" }),
    el("div", { class: "row" }, [
      el("button", { text: "mint share link", onclick: () => actions.mintShare() }),
      el("input", {
        "data-testid": "share-link",
        readonly: true,
        class: "share-link",
        placeholder: "minted link appears here",
      }),
    ]),
  ]);
}

function shareView(share) {
  if (share.status === "gone") {
    return el("section", { class: "share gone", "data-testid": "share-gone" }, [
      el("h1", { text: "410 Gone" }),
      el("p", { text: "this share link expired" }),
    ]);
  }
  if (share.status === "invalid") {
    return el("section", { class: "share gone", "data-testid": "share-invalid" }, [
      el("h1", { text: "400 Bad Request" }),
      el("p", { text: "this share link could not be decoded" }),
    ]);
  }
  const data = share.data;
  const box = el("section", { class: "share", "data-testid": "share-view" }, [
    el("h1", { text: `shared session: ${data.session}` }),
    el("p", {
      text: `turns ${data.hud.turns} · events ${data.hud.events} · through event ${data.lastEventId}`,
    }),
  ]);
  for (const entry of data.transcript) {
    box.append(
      el("article", { class: `entry entry-${entry.role}` }, [
        el("span", { class: "entry-role", text: entry.tool ? `${entry.role}:${entry.tool}` : entry.role }),
        el("p", { class: "entry-text", text: entry.text }),
      ]),
    );
  }
  return box;
}
