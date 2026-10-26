// DOM primitives for the SPA panels (todo-37 split of spa/ui).
// Security rule: untrusted text reaches the document ONLY through
// textContent — never innerHTML.
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

export const fmt = (n) => String(n);

export const fmtUsd = (micros) => `$${(micros / 1e6).toFixed(6)}`;
