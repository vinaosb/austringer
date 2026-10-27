// Share surfaces: the mint box in the session view and the standalone
// share reader route.
import { el } from "spa/ui-dom";

export function shareBox(state, actions) {
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

export function shareView(share) {
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
