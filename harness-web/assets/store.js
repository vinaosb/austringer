// State store for the harness web SPA (todo 37): a total,
// non-throwing reducer over daemon server messages. Every ServerMsg
// variant of protocol v1 has a case; unknown or malformed input
// counts as a visible anomaly instead of throwing.
const MAX_FRAMES = 4096; // mirrors protocol MAX_SNAPSHOT_FRAMES

export function createStore() {
  let state = emptyState();
  const listeners = new Set();

  function set(next) {
    state = next;
    for (const fn of listeners) fn(state);
  }

  return {
    dispatch(raw) {
      let msg;
      try {
        msg = JSON.parse(raw);
      } catch {
        return set(bumpAnomaly(state));
      }
      if (msg === null || typeof msg !== "object" || typeof msg.type !== "string") {
        return set(bumpAnomaly(state));
      }
      set(reduce(state, msg));
    },
    getState: () => state,
    subscribe(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    resetUnauthorized() {
      if (state.unauthorized) {
        set({ ...state, unauthorized: false });
      }
    },
  };
}

function emptyState() {
  return {
    hello: null,
    unauthorized: false,
    closedBy: null,
    lastEventId: 0,
    frames: [],
    transcript: [],
    hud: {
      turns: 0,
      events: 0,
      llmVisible: 0,
      results: 0,
      toolCalls: {},
      outcomes: [],
      anomalies: 0,
      resyncs: 0,
    },
    asks: {},
    resolvedLog: [],
    diffs: [],
    notifications: [],
    workers: [],
    budget: null,
    budgetExhausted: false,
    serverErrors: [],
  };
}

function bumpAnomaly(state) {
  return {
    ...state,
    hud: { ...state.hud, anomalies: state.hud.anomalies + 1 },
  };
}

function reduce(state, msg) {
  switch (msg.type) {
    case "hello_ack":
      return { ...state, hello: { version: msg.version, lastEventId: msg.last_event_id } };
    case "pong":
      return state;
    case "event":
      return applyFrame(state, msg.frame);
    case "snapshot":
      return applySnapshot(state, msg.frames || []);
    case "state_resync_required":
      return {
        ...state,
        hud: { ...state.hud, resyncs: state.hud.resyncs + 1 },
      };
    case "permission_ask":
      return { ...state, asks: { ...state.asks, [msg.ask.ask_id]: msg.ask } };
    case "permission_resolved": {
      const asks = { ...state.asks };
      delete asks[msg.ask_id];
      return {
        ...state,
        asks,
        resolvedLog: [
          ...state.resolvedLog,
          { askId: msg.ask_id, resolution: msg.resolution },
        ],
      };
    }
    // diff/notification/worker events ride frames; applyFrame reuses
    // these cases for the worker variants.
    case "worker_claimed":
      return {
        ...state,
        workers: [...state.workers, { queue: msg.queue, worker: msg.worker }],
      };
    case "worker_healed":
      return { ...state, workers: [...state.workers, { queue: msg.queue, healed: true }] };
    case "error":
      return applyError(state, msg);
    case "bye":
      return { ...state, closedBy: msg.reason || "" };
    default:
      return bumpAnomaly(state);
  }
}

function diffEntry(msg) {
  return {
    path: msg.path,
    beforeHash: msg.before_hash,
    afterHash: msg.after_hash,
    hunks: Array.isArray(msg.hunks) ? msg.hunks : [],
  };
}

function applyError(state, msg) {
  if (msg.code === "auth") {
    return { ...state, unauthorized: true };
  }
  return {
    ...state,
    serverErrors: [...state.serverErrors, { code: msg.code, message: msg.message }],
  };
}

function applyFrame(state, frame) {
  if (!frame || typeof frame.id !== "number") {
    return bumpAnomaly(state);
  }
  const frames = state.frames.length >= MAX_FRAMES ? state.frames.slice(1) : state.frames;
  const next = {
    ...state,
    frames: [...frames, frame],
    lastEventId: Math.max(state.lastEventId, frame.id),
    hud: { ...state.hud, events: state.hud.events + 1 },
  };
  const ev = frame.event;
  if (!ev || typeof ev.type !== "string") {
    return bumpAnomaly(next);
  }
  switch (ev.type) {
    case "recorded":
      return applyEngineEvent(next, ev.seq, ev.event);
    case "turn_started":
      return { ...next, hud: { ...next.hud, turns: next.hud.turns + 1 } };
    case "turn_finished":
      return {
        ...next,
        hud: {
          ...next.hud,
          outcomes: [...next.hud.outcomes, ev.outcome || ""],
        },
      };
    case "diff":
      return { ...next, diffs: [...next.diffs, diffEntry(ev)] };
    case "notification":
      return {
        ...next,
        notifications: [...next.notifications, { kind: ev.kind, message: ev.message }],
      };
    // Team frames (todo 15): Child* events build the workers panel,
    // TeamBudget carries the delegation aggregate, and the typed
    // exhaustion halt mirrors the v1 downgrade's notification.
    case "child_started":
      return {
        ...next,
        workers: [
          ...next.workers.filter((w) => w.child !== ev.child),
          { child: ev.child, task: ev.task || "", state: "running", detail: "" },
        ],
      };
    case "child_progress":
      return {
        ...next,
        workers: next.workers.some((w) => w.child === ev.child)
          ? next.workers.map((w) =>
              w.child === ev.child ? { ...w, state: ev.phase, detail: ev.detail || "" } : w,
            )
          : [
              ...next.workers,
              { child: ev.child, task: "", state: ev.phase, detail: ev.detail || "" },
            ],
      };
    case "child_finished":
      return {
        ...next,
        workers: next.workers.map((w) =>
          w.child === ev.child ? { ...w, state: ev.state, detail: ev.detail || "" } : w,
        ),
      };
    case "team_budget":
      return {
        ...next,
        budget: {
          spawned: ev.spawned,
          tokensSpent: ev.tokens_spent,
          fanOutCap: ev.fan_out_cap,
          tokenBudget: ev.token_budget,
        },
      };
    case "team_budget_exhausted":
      return {
        ...next,
        budgetExhausted: true,
        notifications: [
          ...next.notifications,
          {
            kind: "team_budget_exhausted",
            message: `delegation budget exhausted: ${ev.spent} of ${ev.budget} tokens; further task spawns refused`,
          },
        ],
      };
    case "worker_claimed":
    case "worker_healed":
      return reduce(next, ev);
    default:
      return bumpAnomaly(next);
  }
}

// Engine conversation events (harness-core event taxonomy) become
// transcript entries; images render as their typed text marker.
function applyEngineEvent(state, seq, event) {
  if (!event || typeof event.type !== "string") {
    return bumpAnomaly(state);
  }
  const hud = { ...state.hud, llmVisible: state.hud.llmVisible + 1 };
  switch (event.type) {
    case "user":
      return push(state, hud, { role: "user", seq, text: blocksText(event.blocks) });
    case "assistant":
      return push(state, hud, { role: "assistant", seq, text: blocksText(event.blocks) });
    case "system":
      return push(state, hud, { role: "system", seq, text: event.text || "" });
    case "tool_call": {
      const withTool = {
        ...hud,
        toolCalls: {
          ...hud.toolCalls,
          [event.tool]: (hud.toolCalls[event.tool] || 0) + 1,
        },
      };
      return push(state, withTool, {
        role: "tool_call",
        seq,
        tool: event.tool,
        text: argumentsPreview(event.arguments),
      });
    }
    case "tool_result":
      return push(state, { ...hud, results: hud.results + 1 }, {
        role: "tool_result",
        seq,
        text: blocksText(event.blocks),
      });
    case "condensation":
      return push(state, hud, { role: "condensation", seq, text: event.summary || "" });
    case "snapshot":
      return push(state, hud, { role: "snapshot", seq, text: event.label || "" });
    default:
      return bumpAnomaly({ ...state, hud });
  }
}

function push(state, hud, entry) {
  return { ...state, hud, transcript: [...state.transcript, entry] };
}

function blocksText(blocks) {
  if (!Array.isArray(blocks)) return "";
  return blocks
    .map((b) => {
      if (b === null || typeof b !== "object") return "";
      if (b.type === "text") return typeof b.text === "string" ? b.text : "";
      if (b.type === "image") {
        const media = b.media && typeof b.media.hash === "string" ? b.media.hash.slice(0, 8) : "?";
        return `[image: ${b.mime || "?"} ${b.width || "?"}x${b.height || "?"} ${media}]`;
      }
      return "";
    })
    .join("\n");
}

function argumentsPreview(args) {
  if (args === null || typeof args !== "object") return "";
  try {
    const text = JSON.stringify(args);
    return text.length > 300 ? `${text.slice(0, 300)}…` : text;
  } catch {
    return "";
  }
}

// A snapshot replaces history entirely: delta continuity is void.
function applySnapshot(state, frames) {
  let next = {
    ...emptyState(),
    hello: state.hello,
    hud: { ...emptyState().hud, resyncs: state.hud.resyncs, anomalies: state.hud.anomalies },
    serverErrors: state.serverErrors,
  };
  for (const frame of frames) {
    next = applyFrame(next, frame);
  }
  return next;
}
