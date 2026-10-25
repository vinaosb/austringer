// Client-side share links (todo 37): a redacted, expiring snapshot of
// the live session encoded URL-safely in the location fragment. No
// server round-trip, no cloud service — the daemon (locked during
// todo 37) exposes no share endpoint, so links are self-contained.
// Opening an expired link renders the 410 Gone state.
//
// Redaction policy: tool arguments, tool targets, and image markers
// never ride the link; free text truncates at 200 chars per entry.
const TEXT_LIMIT = 200;
const MAX_ENTRIES = 50;

export function mintShare(state, ttlMs) {
  const payload = {
    v: 1,
    exp: Date.now() + ttlMs,
    session: state.session || "",
    lastEventId: state.lastEventId || 0,
    hud: {
      turns: state.hud.turns,
      events: state.hud.events,
      toolCalls: state.hud.toolCalls,
    },
    transcript: (state.transcript || [])
      .slice(-MAX_ENTRIES)
      .map((entry) => ({
        role: entry.role,
        tool: entry.role === "tool_call" ? entry.tool : undefined,
        // Tool text embeds stringified arguments/output — always redacted.
        text:
          entry.role === "tool_call" || entry.role === "tool_result"
            ? "(redacted)"
            : typeof entry.text === "string"
              ? entry.text.slice(0, TEXT_LIMIT)
              : "",
      })),
  };
  return `#share=${encodeB64u(JSON.stringify(payload))}`;
}

export function parseShare(fragment) {
  const body = typeof fragment === "string" ? fragment.replace(/^#share=/, "") : "";
  if (!body || !/^[A-Za-z0-9_-]+$/.test(body)) {
    return { status: "invalid" };
  }
  let payload;
  try {
    payload = JSON.parse(decodeB64u(body));
  } catch {
    return { status: "invalid" };
  }
  if (
    payload === null ||
    typeof payload !== "object" ||
    payload.v !== 1 ||
    typeof payload.exp !== "number" ||
    !Array.isArray(payload.transcript)
  ) {
    return { status: "invalid" };
  }
  if (Date.now() > payload.exp) {
    return { status: "gone" };
  }
  return { status: "ok", data: payload };
}

// Unicode-safe unpadded base64url of a UTF-8 string.
export function encodeB64u(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function decodeB64u(body) {
  const padded = body.replace(/-/g, "+").replace(/_/g, "/");
  const binary = atob(padded + "=".repeat((4 - (padded.length % 4)) % 4));
  const bytes = Uint8Array.from(binary, (ch) => ch.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}
