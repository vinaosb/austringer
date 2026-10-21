// DOM rendering for the harness web SPA (todo 37). Security rule:
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
    el("h1", { text: "harness" }),
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
    el("strong", { text: `harness · ${props.session || "dashboard"}` }),
    el("span", { class: `status status-${props.status}`, text: props.status }),
  ];
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
    trendsPanel(state),
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
  panel.append(el("p", { class: "na", text: "cost / cache / eval scores: not carried by protocol v1" }));
  return panel;
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
    diffs(state, actions),
    transcript(state),
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
          el("button", { text: "cancel-vote", onclick: () => actions.cancelVote(ask.ask_id) }),
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

function transcript(state) {
  const box = el("section", { class: "transcript", "data-testid": "transcript" });
  for (const entry of state.transcript) {
    box.append(
      el("article", { class: `entry entry-${entry.role}`, "data-role": entry.role }, [
        el("span", { class: "entry-role", text: entry.role }),
        entry.tool ? el("code", { class: "entry-tool", text: entry.tool }) : null,
        el("p", { class: "entry-text", text: entry.text }),
      ]),
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
      actions.submit(text.value, images.slice());
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
    el("p", {
      class: "hint",
      text: "images preview locally; protocol v1 submit carries text only (media upload endpoint disclosed for the daemon)",
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
    el("p", { class: "hint", text: "redacted snapshot link with a 10-minute expiry (client-side; no cloud)" }),
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
