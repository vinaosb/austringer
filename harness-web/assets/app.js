// Bootstrap + wiring for the harness web SPA (todo 37). Zero-build:
// this module is the entry served by the daemon; it parses the URL
// (deep-linked session, token hand-off, share fragments), connects
// the transport, and re-renders on store changes.
import { connect } from "spa/proto";
import { createStore } from "spa/store";
import { mintShare, parseShare } from "spa/share";
import { render } from "spa/ui";

const TOKEN_KEY = "harness-web-token";
const SHARE_TTL_MS = 10 * 60 * 1000;

function sessionFromPath(pathname) {
  const match = pathname.match(/^\/s\/([^/]+)\/?$/);
  return match ? decodeURIComponent(match[1]) : null;
}

function initialToken() {
  const params = new URLSearchParams(location.search);
  const fromQuery = params.get("token");
  if (fromQuery) {
    sessionStorage.setItem(TOKEN_KEY, fromQuery);
    params.delete("token");
    const query = params.toString();
    history.replaceState(null, "", location.pathname + (query ? `?${query}` : "") + location.hash);
  }
  return sessionStorage.getItem(TOKEN_KEY);
}

function boot() {
  const root = document.getElementById("app");
  const store = createStore();
  const app = {
    store,
    controller: null,
    status: "idle",
    transport: "",
    sessions: [],
    replayIdx: null,
    session: sessionFromPath(location.pathname),
    shareHash: location.hash.startsWith("#share=") ? location.hash : null,
    token: initialToken(),
  };

  const props = () => ({
    token: app.token,
    session: app.session,
    status: app.status,
    transport: app.transport,
    sessions: app.sessions,
    replayIdx: app.replayIdx ?? (app.store.getState().frames.length - 1),
    share: app.shareParsed,
  });

  const view = () => {
    if (app.shareHash) return "share";
    if (app.session) return "session";
    return "dashboard";
  };

  const repaint = () => render(root, view(), store.getState(), props(), actions);

  const actions = {
    setToken(token) {
      app.token = token;
      sessionStorage.setItem(TOKEN_KEY, token);
      store.resetUnauthorized();
      connectAll();
      repaint();
    },
    openSession(id) {
      location.assign(`/s/${encodeURIComponent(id)}`);
    },
    submit(text, localImages) {
      // Protocol v1 submit carries text only; localImages stay in the
      // composer preview (disclosed limitation, see composer hint).
      send({ type: "command", command: { type: "submit", text } });
    },
    steer(text) {
      if (!text) return;
      send({ type: "command", command: { type: "steer", text } });
    },
    cancel() {
      send({ type: "command", command: { type: "cancel" } });
    },
    answer(askId, answer) {
      send({ type: "permission_answer", ask_id: askId, answer });
    },
    cancelVote(askId) {
      send({ type: "permission_cancel", ask_id: askId });
    },
    replaySeek(index) {
      app.replayIdx = index;
      repaint();
    },
    async mintShare() {
      const link = mintShare(
        { session: app.session, ...store.getState() },
        SHARE_TTL_MS,
      );
      const input = document.querySelector("[data-testid=share-link]");
      if (input) {
        input.value = `${location.origin}${location.pathname}${link}`;
      }
    },
    async loadSessions() {
      if (!app.token) return;
      try {
        const response = await fetch("/sessions", {
          headers: { authorization: `Bearer ${app.token}` },
        });
        if (response.status === 401) {
          store.dispatch(JSON.stringify({ type: "error", code: "auth", message: "unauthorized" }));
          return;
        }
        const body = await response.json();
        app.sessions = Array.isArray(body.sessions) ? body.sessions : [];
        repaint();
      } catch {
        app.status = "error";
        repaint();
      }
    },
    reconnect() {
      connectAll();
    },
  };

  function send(msg) {
    if (app.controller) {
      app.controller.send(msg);
    }
  }

  function connectAll() {
    if (app.controller) {
      app.controller.close();
      app.controller = null;
    }
    if (!app.session || !app.token) return;
    app.controller = connect({
      base: location.host,
      token: app.token,
      session: app.session,
      onMessage: (raw) => store.dispatch(raw),
      onStatus: (state, controller) => {
        app.status = state;
        app.transport = controller.mode === "ws" ? "websocket" : controller.mode === "sse" ? "sse" : "";
        repaint();
      },
    });
    app.controller.start();
  }

  store.subscribe(repaint);

  if (app.shareHash) {
    app.shareParsed = parseShare(app.shareHash);
  } else if (app.session && app.token) {
    connectAll();
  } else if (app.token) {
    actions.loadSessions();
  }

  window.addEventListener("hashchange", () => {
    app.shareHash = location.hash.startsWith("#share=") ? location.hash : null;
    app.shareParsed = app.shareHash ? parseShare(app.shareHash) : undefined;
    repaint();
  });

  repaint();
}

boot();
