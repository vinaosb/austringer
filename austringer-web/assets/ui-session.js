// The session surface (route "/s/{sid}"): HUD, ask gates (permission,
// fallback, plan), hunk review, transcript, composer, replay.
import { el, fmt } from "spa/ui-dom";
import { historyPanel, timelinePanel, trendsPanel } from "spa/ui-history";
import { teamPanel } from "spa/ui-team";
import { shareBox } from "spa/ui-share";

export function sessionView(state, props, actions) {
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
