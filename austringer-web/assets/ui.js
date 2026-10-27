// DOM rendering for the austringer web SPA (todo 37). Security rule:
// untrusted text (events, tool output, share payloads) reaches the
// document ONLY through textContent — never innerHTML. The module is
// dumb: it renders (state, props, actions) and calls actions back.
// Panels live in the spa/ui-* modules (todo-23 size split); this is
// the root: token gate, status banner, and the view switch.
import { el } from "spa/ui-dom";
import { dashboardView } from "spa/ui-dashboard";
import { sessionView } from "spa/ui-session";
import { shareView } from "spa/ui-share";

export { el } from "spa/ui-dom";

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
