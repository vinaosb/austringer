// Protocol codec + transports for the austringer web SPA (todo 37).
// Zero-build ES module: no bundler, no CDN, loopback daemon only.
//
// Wire shapes mirror austringer-daemon/src/protocol.rs exactly:
//   client: {type:"hello",version,token,client_id,session?}
//           {type:"command",command:{type:"submit"|"steer"|"cancel",...}}
//           {type:"permission_answer",ask_id,answer:"allow_once"|"allow_session"|"deny"}
//           {type:"permission_cancel",ask_id}
//           {type:"fallback_answer",ask,chosen:{provider,model}}
//           {type:"plan_answer",ask,decision:"approve"|"reject"|{edit:{artifact}}}
//           {type:"ping"}
//   server: hello_ack | event | permission_ask | permission_resolved |
//           state_resync_required | snapshot | error | pong | bye |
//           usage | fallback_ask | fallback_resolved | verdict |
//           plan_ask | plan_resolved
// The client negotiates protocol v2 (todo 20): the v2 frames above
// flow, and the SSE fallback opts in via ?version=2.
export const PROTOCOL_VERSION = 2;

// Pure WS attach URL, mirroring the TUI contract
// (austringer-tui/src/client.rs ws_attach_url): cursor-less at 0 so the
// first attach keeps the Resync contract, `&after=N` afterwards so a
// reconnect replays the delta.
export function wsAttachUrl(base, session, token, after) {
  return (
    `ws://${base}/s/${encodeURIComponent(session)}/attach?token=${encodeURIComponent(token)}` +
    (after > 0 ? `&after=${after}` : "")
  );
}

// Total decoder: garbage in, {ok:false} out — the store never throws.
export function decode(text) {
  try {
    const value = JSON.parse(text);
    if (value === null || typeof value !== "object" || typeof value.type !== "string") {
      return { ok: false };
    }
    return { ok: true, value };
  } catch {
    return { ok: false };
  }
}

function encode(msg) {
  return JSON.stringify(msg);
}

// Transport controller: WS primary, SSE fallback, POST /cmd for
// commands when only SSE is reachable. Both transports carry the
// `after` replay cursor on reconnect so the daemon replays the delta
// (or forces a resync snapshot).
export function connect({ base, token, session, onMessage, onStatus }) {
  const controller = {
    socket: null,
    source: null,
    mode: "closed", // "ws" | "sse" | "closed"
    lastEventId: 0,
    closedByUser: false,
  };
  const sseURL = (after) =>
    `/s/${encodeURIComponent(session)}/events?token=${encodeURIComponent(token)}` +
    `&version=${PROTOCOL_VERSION}` +
    (after > 0 ? `&after=${after}` : "");

  const status = (state) => onStatus(state, controller);

  function handle(raw) {
    const decoded = decode(raw);
    if (!decoded.ok) {
      onMessage(encode({ type: "error", code: "malformed_frame", message: "undecodable" }));
      return;
    }
    if (decoded.value.type === "event" && decoded.value.frame) {
      controller.lastEventId = Math.max(controller.lastEventId, decoded.value.frame.id);
    }
    onMessage(raw);
  }

  function openWS() {
    if (controller.closedByUser) return;
    status("connecting");
    let socket;
    try {
      socket = new WebSocket(wsAttachUrl(base, session, token, controller.lastEventId));
    } catch {
      openSSE();
      return;
    }
    controller.socket = socket;
    socket.addEventListener("open", () => {
      controller.mode = "ws";
      status("open");
      socket.send(
        encode({
          type: "hello",
          version: PROTOCOL_VERSION,
          token,
          client_id: clientId(),
          session,
        }),
      );
    });
    socket.addEventListener("message", (ev) => handle(String(ev.data)));
    socket.addEventListener("error", () => {
      /* close always follows; handled there */
    });
    socket.addEventListener("close", () => {
      controller.socket = null;
      if (controller.closedByUser) {
        status("closed");
        return;
      }
      if (controller.mode === "ws") {
        // A WS that connected and then died: reconnect after a beat.
        controller.mode = "closed";
        status("reconnecting");
        setTimeout(openWS, 1000);
      } else {
        openSSE();
      }
    });
  }

  function openSSE() {
    if (controller.closedByUser) return;
    status("connecting");
    let source;
    try {
      source = new EventSource(sseURL(controller.lastEventId));
    } catch {
      status("error");
      return;
    }
    controller.source = source;
    source.addEventListener("open", () => {
      controller.mode = "sse";
      status("open");
    });
    source.addEventListener("message", (ev) => handle(String(ev.data)));
    source.addEventListener("error", () => {
      source.close();
      controller.source = null;
      controller.mode = "closed";
      if (controller.closedByUser) {
        status("closed");
        return;
      }
      status("reconnecting");
      setTimeout(() => (controller.mode === "closed" ? openWS() : undefined), 1500);
    });
  }

  controller.start = openWS;
  controller.send = (msg) => {
    const raw = encode(msg);
    if (controller.mode === "ws" && controller.socket && controller.socket.readyState === 1) {
      controller.socket.send(raw);
      return true;
    }
    // SSE is read-only: commands ride the JSON POST endpoint instead.
    postCommand(token, session, raw);
    return false;
  };
  controller.close = () => {
    controller.closedByUser = true;
    if (controller.socket) controller.socket.close();
    if (controller.source) controller.source.close();
    controller.mode = "closed";
    status("closed");
  };
  return controller;
}

async function postCommand(token, session, raw) {
  try {
    await fetch(`/s/${encodeURIComponent(session)}/cmd`, {
      method: "POST",
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      body: raw,
    });
  } catch {
    /* transport-level failure surfaces through status events */
  }
}

// Stable per-tab client id (short, printable — the daemon bounds ids).
export function clientId() {
  const KEY = "austringer-web-client-id";
  let id = sessionStorage.getItem(KEY);
  if (!id) {
    id = `web-${Math.random().toString(36).slice(2, 8)}`;
    sessionStorage.setItem(KEY, id);
  }
  return id;
}
