// Bootstrap + wiring for the austringer web SPA (todo 37). Zero-build:
// this module is the entry served by the daemon; it parses the URL
// (deep-linked session, token hand-off, share fragments), connects
// the transport, and re-renders on store changes.
import { connect } from "spa/proto";
import { routeCommand } from "spa/grammar";
import { createStore } from "spa/store";
import { mintShare, parseShare } from "spa/share";
import { render } from "spa/ui";

const TOKEN_KEY = "austringer-web-token";
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
    packs: [],
    mcpServers: [],
    replayIdx: null,
    history: [],
    historyLoaded: false,
    expanded: new Set(),
    session: sessionFromPath(location.pathname),
    shareHash: location.hash.startsWith("#share=") ? location.hash : null,
    token: initialToken(),
    feedback: null,
  };

  const props = () => ({
    token: app.token,
    session: app.session,
    status: app.status,
    transport: app.transport,
    sessions: app.sessions,
    packs: app.packs,
    mcpServers: app.mcpServers,
    replayIdx: app.replayIdx ?? (app.store.getState().frames.length - 1),
    history: app.history,
    historyLoaded: app.historyLoaded,
    expanded: app.expanded,
    share: app.shareParsed,
    feedback: app.feedback,
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
    // Composer entry point (todo 23): the shared grammar routes the
    // line; feedback renders under the composer. Protocol v1 submit
    // carries text only — local images stay in the composer preview
    // (disclosed limitation, see composer hint).
    input(text) {
      const { sends, feedback } = routeCommand(text, Object.keys(store.getState().asks));
      for (const msg of sends) {
        send(msg);
      }
      app.feedback = feedback;
      repaint();
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
    // FallbackAnswer keys on the SERVER-assigned connection identity at
    // the transport pump — the identity object is echoed verbatim from
    // the ask (garbage resolves deny-side server-side).
    answerFallback(askId, chosen) {
      send({ type: "fallback_answer", ask: askId, chosen });
    },
    answerPlan(askId, decision) {
      // PlanDecision is externally tagged on the wire: "approve" |
      // "reject" ride as bare strings.
      send({ type: "plan_answer", ask: askId, decision });
    },
    toggleFocus(seq) {
      if (app.expanded.has(seq)) {
        app.expanded.delete(seq);
      } else {
        app.expanded.add(seq);
      }
      repaint();
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
    async loadPacks() {
      if (!app.token) return;
      try {
        const response = await fetch("/packs", {
          headers: { authorization: `Bearer ${app.token}` },
        });
        if (response.status === 401) {
          store.dispatch(JSON.stringify({ type: "error", code: "auth", message: "unauthorized" }));
          return;
        }
        const body = await response.json();
        app.packs = Array.isArray(body.packs) ? body.packs : [];
        app.mcpServers = Array.isArray(body.mcp_servers) ? body.mcp_servers : [];
        repaint();
      } catch {
        app.status = "error";
        repaint();
      }
    },
    // GET /s/{sid}/history (task-2 route). A typed 404 (unknown
    // session, e.g. raced ahead of the WS attach) renders the same
    // empty-state as an empty run list — never a stuck spinner.
    async loadHistory() {
      if (!app.token || !app.session) return;
      try {
        const response = await fetch(`/s/${encodeURIComponent(app.session)}/history`, {
          headers: { authorization: `Bearer ${app.token}` },
        });
        if (response.status === 401) {
          store.dispatch(JSON.stringify({ type: "error", code: "auth", message: "unauthorized" }));
          return;
        }
        const body = await response.json();
        app.history = Array.isArray(body) ? body : [];
        app.historyLoaded = true;
        repaint();
      } catch {
        app.history = [];
        app.historyLoaded = true;
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
        // The history route needs the session to exist server-side; a
        // fetch raced ahead of the attach gets a typed 404, so retry
        // once the connection is open.
        if (state === "open" && !app.historyLoaded) {
          actions.loadHistory();
        }
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
    actions.loadPacks();
  }

  window.addEventListener("hashchange", () => {
    app.shareHash = location.hash.startsWith("#share=") ? location.hash : null;
    app.shareParsed = app.shareHash ? parseShare(app.shareHash) : undefined;
    repaint();
  });

  repaint();
}

boot();
