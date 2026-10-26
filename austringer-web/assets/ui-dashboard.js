// Dashboard panels (route "/"): session list, tool heatmap, packs.
import { el, fmt } from "spa/ui-dom";

export function dashboardView(state, props, actions) {
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
