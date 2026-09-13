---
name: frontend
description: "Frontend UI guidance: React component patterns, hooks discipline, theming and dark mode via CSS variables, responsive layout, WCAG accessibility, and Core Web Vitals budgets."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["TypeScript", "JavaScript", "HTML", "CSS"]
allowed-tools: ["view_window", "edit_lines", "execute_command", "tool_search"]
triggers:
  - react component
  - react hook
  - css layout
  - flexbox
  - dark mode
  - wcag
  - aria attribute
  - focus management
  - screen reader
  - color contrast
  - responsive layout
  - core web vitals
  - lighthouse
  - modo escuro
  - accesibilidad web
  - 无障碍
do-not-trigger-on:
  - entity framework
  - ef core
  - docker compose
  - kubernetes manifest
  - lease agreement
  - trial balance
  - journal entries
  - differential equation
  - electrical panel
---

# Frontend pack

{{shared:pack-conventions}}

## Workflow

1. **Component boundaries first**: colocate state with the narrowest
   component that needs it; lift only when two siblings consume it.
   Props are a typed contract (`interface Props`, no `any`).
2. **Hooks discipline**: effects synchronize with external systems
   only; derive during render instead of mirroring state. Every effect
   has a cleanup path; every async effect handles cancellation
   (AbortSignal).
3. **Theming**: design tokens as CSS custom properties on a root scope;
   dark mode flips tokens, never hard-codes colors. One source of truth
   for spacing/typography scale.
4. **Responsive layout**: layout at the container level (flex/grid);
   test at 360px, 768px, 1280px breakpoints; no fixed pixel widths on
   text containers.
5. **Accessibility (WCAG 2.2 AA)**: semantic HTML before ARIA; labels on
   every control; visible focus that is never removed; contrast ≥ 4.5:1
   for body text; motion respects `prefers-reduced-motion`; modals trap
   focus and restore it on close.
6. **Performance budget**: Core Web Vitals as the gate — LCP < 2.5s,
   INP < 200ms, CLS < 0.1. Images sized/resolved responsively; lists
   over ~100 rows virtualized; third-party scripts deferred and audited.

## Worked example

Request: *"Make this dialog usable with a keyboard."*

```tsx
// before: <div class="overlay">…</div> — unreachable by Tab
// after:
function Dialog({ title, onClose }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();          // native focus trap
    return () => ref.current?.close(); // cleanup
  }, []);
  return (
    <dialog ref={ref} aria-labelledby="t" onClose={onClose}>
      <h2 id="t">{title}</h2>
      …
    </dialog>
  );
}
```

Keyboard: Tab cycles inside the dialog, Esc closes it (native), focus
returns to the invoking button. Semantics come from `<dialog>`, not
ARIA re-creation.

## Eval scenario

Prompt: `"Make this React component accessible and fix the layout"`.
Expected routing: `frontend` (trigger: `react component`). First
actions: read the component and its usage, list semantic/focus/contrast
gaps against the checklist above, then fix layout and a11y in one
minimal diff with before/after notes.
