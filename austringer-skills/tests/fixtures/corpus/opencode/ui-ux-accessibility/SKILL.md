---
name: ui-ux-accessibility
description: "Comprehensive UI/UX accessibility skill based on WCAG 2.2. Implement, review, audit, and fix accessibility issues across web, mobile, and desktop projects. Covers all 87 WCAG 2.2 success criteria (A/AA/AAA), ARIA patterns, focus management, screen reader testing, automated tooling (axe-core, pa11y, Lighthouse, WAVE), platform-specific guidance (React, Next.js, Android, iOS, Electron), cognitive/motor/visual/auditory accessibility, and legal compliance (ADA, Section 508, EN 301 549, AODA, EAA). USE FOR: accessibility audit, a11y review, WCAG compliance, accessible components, keyboard navigation, screen reader support, ARIA implementation, color contrast, focus management, form accessibility, modal dialogs, accessible forms, skip navigation, live regions, accessible authentication, reduced motion, text alternatives, captions, semantic HTML, accessible UI patterns, fix accessibility issues, accessibility testing, inclusive design. DO NOT USE FOR: visual design implementation without accessibility context (use frontend-ui-ux), general CSS styling (use frontend-ui-ux), backend logic."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  standard: "WCAG 2.2"
  source: "https://www.w3.org/WAI/WCAG22/quickref/?versions=2.2"
---

# UI/UX Accessibility Skill

WCAG 2\.2 accessibility: implement, review, audit, fix. 87 success criteria + real-world patterns + testing + platform guidance.

## When to Use This Skill

| Trigger | Action |
|---------|--------|
| "accessibility audit", "a11y review" | Run full review checklist |
| "make this accessible", "add accessibility" | Implement from patterns section |
| "fix accessibility issue", "a11y bug" | Use remediation workflow |
| "WCAG compliance", "Section 508" | Check conformance levels |
| "keyboard navigation", "screen reader" | Apply interaction patterns |
| "color contrast", "focus management" | Apply visual/interaction checks |
| Building new UI component | Apply component patterns proactively |

## Operating Modes

This skill supports three operating modes. Identify which mode the user needs and follow the corresponding workflow.

### Mode 1: Audit / Review
Full audit of existing code/page. Use Review Checklist. Report by WCAG principle + severity.

### Mode 2: Implement
Build accessible UI or enhance existing. Use Implementation Patterns. Validate against WCAG criteria.

### Mode 3: Fix / Remediate
Fix known a11y issue. Use Remediation Workflow: diagnose ? fix ? verify.

---

## WCAG 2.2 Quick Reference

87 criteria by principle/guideline/level (A/AA/AAA). Each: check, implement, common failures.

> **Target**: AA standard. A = minimum. AAA = aspiratory. Clarify target before auditing.

### Principle 1 — Perceivable

Information and UI components must be presentable to users in ways they can perceive.

#### Guideline 1.1 — Text Alternatives

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 1.1.1 | Non-text Content | A | All meaningful images have alt text; decorative images are ignored by AT | `alt="description"`, `aria-label`, `aria-labelledby`, empty `alt=""` for decorative, `role="presentation"` | Missing alt, filename as alt ("img001.jpg"), stale alt after UI updates, decorative images announced |

#### Guideline 1.2 — Time-based Media

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 1.2.1 | Audio-only / Video-only (Prerecorded) | A | Audio has transcript; video-only has transcript or audio track | Adjacent transcript, described video version | No transcript, transcript omits visual info |
| 1.2.2 | Captions (Prerecorded) | A | Prerecorded video with audio has synchronized captions | WebVTT/TTML track, `<track kind="captions">`, open captions | Missing sound cues, auto-captions unreviewed |
| 1.2.3 | Audio Description or Media Alternative (Prerecorded) | A | Video has audio description or full text alternative | AD track, full transcript with visual notes, linked alt page | Silent visual actions omitted, transcript is dialogue-only |
| 1.2.4 | Captions (Live) | AA | Live audio has real-time captions | CART service, live captioning API | Captions added only after event, excessive delay |
| 1.2.5 | Audio Description (Prerecorded) | AA | Prerecorded video has audio description for visual content | Separate AD track, narration in pauses | Missing visual-only actions |
| 1.2.6 | Sign Language (Prerecorded) | AAA | Prerecorded media has sign language interpretation | Inset interpreter video, sign-language track | No signed alternative |
| 1.2.7 | Extended Audio Description (Prerecorded) | AAA | Extended AD when pauses insufficient | Paused playback with inserted narration | Overstuffed standard AD, missing fast scenes |
| 1.2.8 | Media Alternative (Prerecorded) | AAA | Complete text alternative for all prerecorded media | Detailed transcript with timestamps | Captions-only, summary-only |
| 1.2.9 | Audio-only (Live) | AAA | Live audio has equivalent text alternative | Live captioning, posted script | Audio stream only, transcript posted later |

#### Guideline 1.3 — Adaptable

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 1.3.1 | Info and Relationships | A | Headings, lists, tables, labels, landmarks are semantic | Native HTML (`<h1>`-`<h6>`, `<ul>`, `<th>`, `<label>`), `<fieldset>/<legend>`, ARIA landmarks | div soup, visual-only headings, unassociated labels, layout tables |
| 1.3.2 | Meaningful Sequence | A | DOM order matches intended reading/focus order | Source order mirrors visual layout, CSS-only visual changes | Two-column content read out of order, absolutely positioned controls |
| 1.3.3 | Sensory Characteristics | A | Instructions don't rely solely on shape/size/color/sound/location | "Select the Save button", "Choose Next" | "Click the green button on the right", "press the round icon" |
| 1.3.4 | Orientation | AA | Content works in both portrait and landscape | Responsive layout, no forced orientation locks | Rotation lock, content hidden in one orientation |
| 1.3.5 | Identify Input Purpose | AA | Common inputs expose purpose via autocomplete | Correct `autocomplete` tokens (`name`, `email`, `tel`, `street-address`) | Missing autocomplete, custom inputs with no purpose |
| 1.3.6 | Identify Purpose | AAA | UI components/regions have determinable purpose | Clear roles, names, states, semantic landmarks | Anonymous custom components, unlabeled regions |

#### Guideline 1.4 — Distinguishable

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 1.4.1 | Use of Color | A | Color isn't the only way to convey information | Icons, patterns, underlines, text labels alongside color | Red-only errors, color-only chart legend |
| 1.4.2 | Audio Control | A | Auto-playing audio >3s has pause/stop/volume control | Mute/stop controls, avoid autoplay | Hidden background audio, no stop button |
| 1.4.3 | Contrast (Minimum) | AA | Text: 4.5:1 normal, 3:1 large text | Tokenized color system, contrast testing in design/code | Light gray body text, low-contrast links |
| 1.4.4 | Resize Text | AA | Text resizable to 200% without loss | Relative units (`rem`, `em`), flexible containers | Clipped text, overlapping controls, fixed-height containers |
| 1.4.5 | Images of Text | AA | Use real text, not text-in-images | Styled HTML text, SVG text | Hero text in images, screenshot buttons |
| 1.4.6 | Contrast (Enhanced) | AAA | Text: 7:1 normal, 4.5:1 large | High-contrast themes, stronger palette | Barely-passes color pairs |
| 1.4.7 | Low / No Background Audio | AAA | Speech not drowned by background audio | Ducked audio, no music under narration | Music masking speech |
| 1.4.8 | Visual Presentation | AAA | Text blocks support customization | Fluid layouts, user-selectable styles | Hard-coded line lengths, forced justification |
| 1.4.9 | Images of Text (No Exception) | AAA | No images of text except logos | Live text, SVG with text alternatives | Screenshot-based headings |
| 1.4.10 | Reflow | AA | Content reflows at 320px wide / 400% zoom without horizontal scroll | Responsive breakpoints, flexible grids | Horizontal page scroll, fixed-width tables breaking layout |
| 1.4.11 | Non-text Contrast | AA | UI components and graphics: 3:1 contrast | Clear borders, strong icon contrast, visible states | Faint icons, low-contrast input borders, indistinct disabled state |
| 1.4.12 | Text Spacing | AA | Increased spacing (line-height, paragraph, letter, word) doesn't break UI | Relative sizing, flexible boxes | Clipped lines, overlapping buttons when spacing grows |
| 1.4.13 | Content on Hover or Focus | AA | Hover/focus content is dismissible, hoverable, persistent | ESC-to-close, hoverable popovers, delayed dismissal | Tooltip vanishes on pointer move, cannot dismiss |

### Principle 2 — Operable

UI components and navigation must be operable.

#### Guideline 2.1 — Keyboard Accessible

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 2.1.1 | Keyboard | A | All functionality operable by keyboard | Native controls, keyboard handlers, focus management | Click-only divs, drag-only UI, canvas without keyboard fallback |
| 2.1.2 | No Keyboard Trap | A | Focus can move away from every component | Sane tab order, escape/close controls, focus return | Stuck focus in widgets, modal loops with no exit |
| 2.1.3 | Keyboard (No Exception) | AAA | All functionality keyboard accessible, no exceptions | Full keyboard support for custom widgets, games, drag/reorder | Mouse-only maps, gesture-only interactions |
| 2.1.4 | Character Key Shortcuts | A | Single-char shortcuts can be turned off/remapped/focus-scoped | Modifier-based shortcuts, shortcut settings, focus-scoped bindings | "s" submits page, accidental shortcuts during text entry |

#### Guideline 2.2 — Enough Time

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 2.2.1 | Timing Adjustable | A | Time limits can be turned off/adjusted/extended | Extend button, preferences, save progress | Silent timeout, no extension, lost work |
| 2.2.2 | Pause, Stop, Hide | A | Moving/blinking/scrolling content has controls | Pause buttons, reduced motion, stop auto-rotation | Marquee text, auto-rotating banners, blinking with no control |
| 2.2.3 | No Timing | AAA | Timing not part of activity unless essential | Untimed flows, save-and-return | Arbitrary countdowns, timed forms without need |
| 2.2.4 | Interruptions | AAA | Interruptions can be postponed/suppressed | Quiet mode, defer notification, non-blocking alerts | Unsolicited modal spam, disruptive chat popups |
| 2.2.5 | Re-authenticating | AAA | After timeout, users continue without losing data | Restore draft, resume flow after login | Forced restart after login, lost form state |
| 2.2.6 | Timeouts | AAA | Users warned about inactivity timeouts | Countdown warning modal, timeout settings | Surprise logout, warning after data loss |

#### Guideline 2.3 — Seizures and Physical Reactions

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 2.3.1 | Three Flashes or Below Threshold | A | No content flashes above seizure thresholds | Limit flash frequency, avoid strobe effects | Rapid flashing banners, strobe transitions |
| 2.3.2 | Three Flashes | AAA | No content flashes more than 3 times/second | Smooth transitions, no flash effects | Fast flash loops, UI blink effects |
| 2.3.3 | Animation from Interactions | AAA | Motion from interaction can be disabled | `prefers-reduced-motion`, simplified transitions, disable setting | Motion sickness from click animation, no reduce option |

#### Guideline 2.4 — Navigable

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 2.4.1 | Bypass Blocks | A | Skip links or landmark navigation to main content | "Skip to content" link, ARIA landmarks | Repeated header/nav with no bypass |
| 2.4.2 | Page Titled | A | Each page has descriptive title | Page-specific `<title>` with site suffix, SPA route updates | "Home" everywhere, stale SPA titles |
| 2.4.3 | Focus Order | A | Focus follows meaningful sequence | Logical DOM order, sensible focus management in widgets | Focus jumps to hidden elements, broken modal order |
| 2.4.4 | Link Purpose (In Context) | A | Link purpose clear from text or context | Descriptive link labels, headings clarifying grouped links | "click here", "read more", ambiguous repeated text |
| 2.4.5 | Multiple Ways | AA | Multiple ways to find pages | Search + nav, sitemap, category pages | One linear path only, no search on large sites |
| 2.4.6 | Headings and Labels | AA | Headings/labels describe topic or purpose | Descriptive section headings, meaningful form labels | "Section 1", "Information", repeated vague labels |
| 2.4.7 | Focus Visible | AA | Keyboard focus always visible | Strong outline/ring styles, visible custom focus states | `outline: none`, focus only on hover, hidden by styling |
| 2.4.8 | Location | AAA | Users know where they are in site | Breadcrumbs, active nav states, progress markers | No active-state nav, unclear hierarchy |
| 2.4.9 | Link Purpose (Link Only) | AAA | Link purpose clear from text alone | "Download 2026 report (PDF)" | Links needing surrounding sentence for context |
| 2.4.10 | Section Headings | AAA | Content organized with headings | Proper heading hierarchy, subheads for subsections | Wall-of-text, skipped headings |
| 2.4.11 | Focus Not Obscured (Minimum) | AA | Focused element not entirely hidden | Scroll focused element into view, reserve space for fixed UI | Focused input under sticky bar, CTA behind overlay |
| 2.4.12 | Focus Not Obscured (Enhanced) | AAA | Focused element not hidden at all | No overlays covering focus | Even partial occlusion by sticky UI |
| 2.4.13 | Focus Appearance | AAA | Focus indicator sufficiently large and contrasting | Thick outline, high-contrast ring, dual-ring styles | Tiny faint shadow, low-contrast ring |

#### Guideline 2.5 — Input Modalities

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 2.5.1 | Pointer Gestures | A | Path/multipoint gestures have single-pointer alternative | Buttons, discrete controls, keyboard equivalents | Swipe-only carousel, pinch-only zoom |
| 2.5.2 | Pointer Cancellation | A | Pointer actions can be aborted | Activate on up/click, drag-away-cancels, confirm destructive | `mousedown` activation, accidental touch triggers |
| 2.5.3 | Label in Name | A | Accessible name contains visible label text | `aria-labelledby` tied to visible label | Visible "Search" but aria-label says "Submit query" |
| 2.5.4 | Motion Actuation | A | Motion-triggered actions have alternatives + disable | Settings toggle, button alternative | Only device motion works, no disable |
| 2.5.5 | Target Size (Enhanced) | AAA | Pointer targets sufficiently large | Padded buttons, generous hit areas | Tiny icon buttons, cramped inline links |
| 2.5.6 | Concurrent Input Mechanisms | AAA | Interface doesn't force single input modality | Modality-agnostic interactions | Disabling keyboard when touch active |
| 2.5.7 | Dragging Movements | AA | Drag-only actions have non-drag alternative | Move up/down buttons, "Move to..." commands, keyboard reorder | Drag-only sortable lists, slider handles requiring drag |
| 2.5.8 | Target Size (Minimum) | AA | Targets at least 24×24 CSS px (with exceptions) | Padding, larger clickable areas | Tiny close buttons, dense nav links |

### Principle 3 — Understandable

Information and operation of UI must be understandable.

#### Guideline 3.1 — Readable

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 3.1.1 | Language of Page | A | Default page language set programmatically | `<html lang="en">` | Missing/wrong lang, SPA route changes not updating |
| 3.1.2 | Language of Parts | AA | Language changes in content identified | `lang` on inline spans/blocks | Mixed-language text unmarked |
| 3.1.3 | Unusual Words | AAA | Technical/jargon terms explained | Linked glossary, tooltips, plain-language notes | Unexplained jargon, domain acronyms |
| 3.1.4 | Abbreviations | AAA | Abbreviations expanded or explained | Spelled-out first mention, glossary | Unexplained acronyms |
| 3.1.5 | Reading Level | AAA | Complex text has simpler alternative | Plain-language summaries, layered content | Dense legal text with no plain version |
| 3.1.6 | Pronunciation | AAA | Pronunciation available when meaning depends on it | Pronunciation guides, speech markup | Names mispronounced changing meaning |

#### Guideline 3.2 — Predictable

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 3.2.1 | On Focus | A | No unexpected context changes on focus | Focus is passive; actions on activation, not focus | Dropdown opens page on focus, content changes while tabbing |
| 3.2.2 | On Input | A | Input changes don't unexpectedly change context | Explicit submit buttons, preview without navigation | Select causes page load, typing auto-submits |
| 3.2.3 | Consistent Navigation | AA | Repeated navigation in consistent order | Shared templates, stable nav order | Menus reorder by page |
| 3.2.4 | Consistent Identification | AA | Same-function components identified consistently | Reusable component labels, consistent iconography | Same action named differently across pages |
| 3.2.5 | Change on Request | AAA | Context changes only on user request | Explicit apply buttons, opt-in updates | Dropdown changes page immediately |
| 3.2.6 | Consistent Help | A | Help mechanisms in same relative order across pages | Persistent help area/footer placement | Help link moves around or disappears |

#### Guideline 3.3 — Input Assistance

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 3.3.1 | Error Identification | A | Errors identified and described in text | Inline validation, `aria-invalid`, error summary | Red outline only, generic "invalid", silent validation |
| 3.3.2 | Labels or Instructions | A | Inputs have labels and instructions | Visible labels, helper text, required indicators | Unlabeled fields, format requirements in placeholder only |
| 3.3.3 | Error Suggestion | AA | Errors include fix suggestions | Format hints, spelling suggestions, valid range | Vague "error occurred", no correction guidance |
| 3.3.4 | Error Prevention (Legal/Financial/Data) | AA | Critical submissions have review/undo/confirmation | Confirmation pages, undo, transaction review | Irreversible delete/pay/submit with no review |
| 3.3.5 | Help | AAA | Contextual help available for input | Inline help, linked docs, support near forms | Complex forms with no guidance |
| 3.3.6 | Error Prevention (All) | AAA | All submissions provide review/correction/reversal | Review step, undo, editable confirmation | Any irreversible form submission |
| 3.3.7 | Redundant Entry | A | Don't re-ask for already-provided info | Auto-fill, "same as billing", prefilled summaries | Retyping address/name multiple times |
| 3.3.8 | Accessible Authentication (Minimum) | AA | Auth doesn't rely solely on cognitive tests | Passkeys, biometrics, magic links, copyable OTPs, password managers | CAPTCHA-only, "remember this secret", image puzzles |
| 3.3.9 | Accessible Authentication (Enhanced) | AAA | No cognitive function test in auth at all | Passwordless/passkey flows, accessible 2FA | Any auth step requiring recall/puzzle |

### Principle 4 — Robust

Content must be robust enough for reliable interpretation by user agents including AT.

#### Guideline 4.1 — Compatible

| # | Criterion | Level | What to Check | Implementation | Common Failures |
|---|-----------|-------|---------------|----------------|-----------------|
| 4.1.1 | Parsing ⚠️ DEPRECATED in WCAG 2.2 | A | ~~Markup valid enough for reliable parsing~~ — Still best practice but not required for WCAG 2.2 conformance | Valid HTML, linting, unique IDs, proper nesting | Broken nesting, duplicate IDs, unclosed tags |
| 4.1.2 | Name, Role, Value | A | Custom widgets expose name/role/value/state | Native controls first, ARIA when needed, state updates reflected | Div-button with no name/role, slider value not exposed |
| 4.1.3 | Status Messages | AA | Status messages announced without moving focus | `role="status"`, `role="alert"`, `aria-live` regions | Toast notifications silent to screen readers |

---

## Review Checklist (Audit Mode)

Use this checklist when performing an accessibility audit. Check each item and report findings as Pass / Fail / N/A with the WCAG criterion number.

### Level A (Minimum — 32 criteria)

- [ ] **1.1.1** All meaningful images have alt text; decorative images hidden from AT
- [ ] **1.2.1** Prerecorded audio-only/video-only has transcript or equivalent
- [ ] **1.2.2** Prerecorded video captions are present and synchronized
- [ ] **1.2.3** Prerecorded video has audio description or media alternative
- [ ] **1.3.1** Semantic HTML used for all structure (headings, lists, tables, labels, landmarks)
- [ ] **1.3.2** DOM order matches visual/intended reading order
- [ ] **1.3.3** Instructions don't rely solely on sensory characteristics
- [ ] **1.4.1** Color is not the only means of conveying information
- [ ] **1.4.2** Auto-playing audio has pause/stop/volume controls
- [ ] **2.1.1** All functionality keyboard accessible
- [ ] **2.1.2** No keyboard traps
- [ ] **2.1.4** Single-character shortcuts can be disabled/remapped
- [ ] **2.2.1** Time limits are adjustable/extendable
- [ ] **2.2.2** Moving/blinking content can be paused/stopped/hidden
- [ ] **2.3.1** No content flashes above seizure thresholds
- [ ] **2.4.1** Skip link or landmarks to bypass repeated blocks
- [ ] **2.4.2** Page has descriptive title
- [ ] **2.4.3** Focus order is meaningful
- [ ] **2.4.4** Link purpose clear from context
- [ ] **2.5.1** Complex gestures have single-pointer alternative
- [ ] **2.5.2** Pointer actions can be aborted
- [ ] **2.5.3** Accessible name contains visible label text
- [ ] **2.5.4** Motion-triggered actions have alternatives + disable
- [ ] **3.1.1** Page language declared
- [ ] **3.2.1** No context change on focus
- [ ] **3.2.2** No unexpected context change on input
- [ ] **3.2.6** Help in consistent location
- [ ] **3.3.1** Errors identified and described in text
- [ ] **3.3.2** Inputs have labels/instructions
- [ ] **3.3.7** No redundant data entry
- [ ] **4.1.1** ⚠️ DEPRECATED in WCAG 2.2 — Valid markup (still best practice)
- [ ] **4.1.2** Custom widgets expose name/role/value/state

### Level AA (Standard Target — 24 criteria)

- [ ] **1.2.4** Live captions available
- [ ] **1.2.5** Audio description for prerecorded video
- [ ] **1.3.4** Content works in both orientations
- [ ] **1.3.5** Common inputs have autocomplete purpose
- [ ] **1.4.3** Text contrast: 4.5:1 normal, 3:1 large
- [ ] **1.4.4** Text resizable to 200%
- [ ] **1.4.5** No images of text (except logos)
- [ ] **1.4.10** Content reflows at 320px / 400% zoom
- [ ] **1.4.11** Non-text contrast: 3:1 for UI components/graphics
- [ ] **1.4.12** Text spacing overrides don't break layout
- [ ] **1.4.13** Hover/focus content dismissible, hoverable, persistent
- [ ] **2.4.5** Multiple ways to find pages
- [ ] **2.4.6** Headings and labels are descriptive
- [ ] **2.4.7** Focus always visible
- [ ] **2.4.11** Focus not entirely hidden by other content
- [ ] **2.5.7** Dragging has non-drag alternative
- [ ] **2.5.8** Targets at least 24×24 CSS px
- [ ] **3.1.2** Language changes in content identified
- [ ] **3.2.3** Navigation consistent across pages
- [ ] **3.2.4** Components consistently identified
- [ ] **3.3.3** Error suggestions provided
- [ ] **3.3.4** Error prevention for legal/financial/data
- [ ] **3.3.8** Authentication accessible (no cognitive-only tests)
- [ ] **4.1.3** Status messages announced to AT

### Beyond WCAG — Disability-Specific Checks

**Cognitive**: Clear purpose and labels, consistent layout, no surprise movement, no memory-heavy steps, flexible input formats, chunk long tasks, avoid timeouts
**Motor**: Keyboard-complete, large touch targets, no drag-only/hover-only actions, alternatives for complex gestures
**Visual**: Zoom/reflow works, focus always visible, non-text contrast adequate, color not only signal
**Auditory**: Captions for video, transcripts for audio, visual equivalents for sound cues, volume controls
**Seizure/Motion**: No flashing, respect `prefers-reduced-motion`, allow pause/stop/hide for moving content

---

## Implementation Patterns (Implement Mode)

### Core Rule: No ARIA is Better Than Bad ARIA

Always prefer native semantic HTML over ARIA. Use ARIA only when native elements cannot express the component.

### Semantic HTML Foundation

```text
✅ DO                          ❌ DON'T
<button>Submit</button>        <div onclick="submit()">Submit</div>
<a href="/page">Link</a>       <span onclick="nav()">Link</span>
<h1>Title</h1>                 <div class="heading">Title</div>
<label for="name">Name</label> <span>Name</span>
<input type="text" id="name">  <div contenteditable>
<nav>...</nav>                  <div class="nav">...</div>
<main>...</main>                <div class="main">...</div>
```

### ARIA Roles Quick Reference

| Role | Use For | Key Properties |
|------|---------|----------------|
| `dialog` | Modal/non-modal dialogs | `aria-modal`, `aria-labelledby`, `aria-describedby` |
| `alert` | Urgent announcements | content is the message |
| `status` | Non-urgent status updates | content is the message |
| `alertdialog` | Urgent confirmation dialogs | `aria-labelledby`, focus management |
| `tablist` / `tab` / `tabpanel` | Tab interfaces | `aria-selected`, `aria-controls`, keyboard arrows |
| `menu` / `menuitem` | Menus | `aria-expanded`, arrow key navigation |
| `listbox` / `option` | Custom select dropdowns | `aria-selected`, `aria-activedescendant` |
| `tree` / `treeitem` | Hierarchical lists | `aria-expanded`, `aria-level` |
| `grid` / `row` / `cell` | Data grids | `aria-sort`, arrow key navigation |
| `slider` | Range controls | `aria-valuenow`, `aria-valuemin`, `aria-valuemax` |
| `progressbar` | Progress indicators | `aria-valuenow`, `aria-valuemin`, `aria-valuemax` |
| `tooltip` | Tooltips | triggered by `aria-describedby` |

### Focus Management Patterns

**Modal Dialog Open**:
1. Move focus to first interactive element or dialog heading
2. Trap focus inside modal (Tab/Shift+Tab cycle within)
3. Escape key closes modal
4. On close, return focus to triggering element

**SPA Route Change**:
1. Move focus to `<main>` or new content area
2. Announce page change via live region or route announcer
3. Update `<title>` to match new page

**Dynamic Content Update**:
1. Announce via `aria-live` region
2. Don't move focus unless user initiated the action
3. Use `role="status"` for passive updates, `role="alert"` for urgent

**Form Validation**:
1. On submit with errors, focus first invalid field
2. Set `aria-invalid="true"` on invalid fields
3. Associate error message via `aria-describedby`
4. Announce error summary via live region

### Form Accessibility Template

```html
<form>
  <!-- Visible label with explicit association -->
  <label for="email">Email address</label>
  <span id="email-hint" class="hint">Format: user@example.com</span>
  <input
    type="email"
    id="email"
    autocomplete="email"
    aria-describedby="email-hint"
    aria-invalid="false"
    required
  />
  <!-- Error container - populated on validation failure -->
  <div id="email-error" role="alert" class="error" aria-live="polite"></div>

  <button type="submit">Subscribe</button>
</form>
```

**Validation logic**:
- On error: set `aria-invalid="true"`, update `aria-describedby` to include error ID, focus first invalid
- On correction: reset `aria-invalid="false"`, clear error content
- Never use `placeholder` as the only label

### Skip Navigation Pattern

```html
<!-- First focusable element in body -->
<a href="#main-content" class="skip-link">Skip to main content</a>

<!-- Later in page -->
<main id="main-content" tabindex="-1">
  <!-- Content -->
</main>
```

```css
.skip-link {
  position: absolute;
  top: -100%;
  left: 0;
  z-index: 1000;
}
.skip-link:focus {
  top: 0;
  padding: 0.5rem 1rem;
  background: #000;
  color: #fff;
}
```

### Live Region / Announcer Pattern

```html
<!-- Empty live region created on page load -->
<div id="announcer" aria-live="polite" aria-atomic="true" class="sr-only"></div>
```

```css
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
```

```javascript
function announce(message, priority = 'polite') {
  const el = document.getElementById('announcer');
  el.setAttribute('aria-live', priority);
  el.textContent = '';
  // Small delay ensures AT picks up the change
  requestAnimationFrame(() => { el.textContent = message; });
}
```

### Modal Dialog Pattern

```html
<div role="dialog" aria-modal="true" aria-labelledby="dialog-title" aria-describedby="dialog-desc">
  <h2 id="dialog-title">Confirm Deletion</h2>
  <p id="dialog-desc">This action cannot be undone.</p>
  <button id="confirm-btn">Delete</button>
  <button id="cancel-btn">Cancel</button>
</div>
```

**Behavior**:
- On open: focus `#confirm-btn` or dialog heading
- Tab/Shift+Tab cycle within dialog elements only
- Escape closes dialog
- On close: focus returns to trigger element
- Background content: `aria-hidden="true"` on inert wrapper

### Accessible Component Checklist

For every custom component, verify:

1. **Role**: Does AT know what this is? (button, link, tab, etc.)
2. **Name**: Does AT know what it's called? (visible text, aria-label, aria-labelledby)
3. **State**: Does AT know its current state? (expanded, selected, checked, disabled, invalid)
4. **Value**: Does AT know its value? (slider position, text content, selected option)
5. **Keyboard**: Can all interactions be performed via keyboard?
6. **Focus**: Is focus visible and in logical order?
7. **Changes**: Are state changes announced to AT?

### WCAG AAA Contrast Validation

For projects targeting WCAG AAA (7:1 contrast for normal text):

**Relative Luminance Formula:**
```text
1. Linearize sRGB: R_sRGB / 255 → if <= 0.04045: R_lin = R_sRGB / 12.92 → else: R_lin = ((R_sRGB + 0.055) / 1.055) ^ 2.4
2. L = 0.2126 * R_lin + 0.7152 * G_lin + 0.0722 * B_lin
3. Contrast Ratio = (L_lighter + 0.05) / (L_darker + 0.05)
```

**Palette Validation Checklist:**
- Test EVERY text color against its background in BOTH light and dark modes
- Large text (18pt+ or 14pt bold): minimum 4.5:1 (AAA)
- Normal text: minimum 7:1 (AAA)
- UI components and graphics: minimum 3:1 (AA, applies regardless of target level)
- Never lighten a borderline pair for hover/focus states — it may drop below threshold
- Document all color pairs with their contrast ratios in the theme file

**CSS Variable Approach for Dual-Theme Validation:**
```css
:root {
  --tp-primary: #1A5276;
  --tp-bg: #FFFFFF;
}
[data-mud-theme="dark"] {
  --tp-primary: #5DADE2;
  --tp-bg: #1B1B1F;
}
```
Validate `var(--tp-primary)` against `var(--tp-bg)` in both `:root` and `[data-mud-theme="dark"]` contexts.

### Design System Token Contrast Validation

When building a design system with CSS custom properties, validate every semantic color token against every background it appears on, in both light and dark themes.

**Token definition with documented ratios:**

```css
:root {
    --tp-primary: #1A5276;       /* 8.2:1 on white ✅ */
    --tp-error: #C0392B;         /* 5.1:1 on white ✅ */
    --tp-success: #1E8449;       /* 5.4:1 on white ✅ */
    --tp-warning: #B7950B;       /* 4.6:1 on white ✅ (large text only for AAA) */
    --tp-text: #1B1B1F;          /* 16.5:1 on white ✅ */
    --tp-text-muted: #5F6368;    /* 5.8:1 on white ✅ */
    --tp-bg: #FFFFFF;
    --tp-surface: #F8F9FA;
}
[data-mud-theme="dark"] {
    --tp-primary: #5DADE2;       /* 7.3:1 on #1B1B1F ✅ */
    --tp-error: #E74C3C;         /* 5.4:1 on #1B1B1F ✅ */
    --tp-success: #58D68D;       /* 6.8:1 on #1B1B1F ✅ */
    --tp-warning: #F4D03F;       /* 10.1:1 on #1B1B1F ✅ */
    --tp-text: #E4E4E8;          /* 13.8:1 on #1B1B1F ✅ */
    --tp-text-muted: #BDC3C7;    /* 9.6:1 on #1B1B1F ✅ */
    --tp-bg: #1B1B1F;
    --tp-surface: #2C2C30;
}
```

**Validation checklist for design system tokens:**

1. **Document the ratio for every text/background pair**: Use the relative luminance formula above or an automated checker. Record the exact ratio as a comment in the token definition file.
2. **Test BOTH themes**: A color that passes in light mode may fail in dark mode. Run the full palette through contrast validation for `:root` and `[data-mud-theme="dark"]` separately.
3. **Gradient backgrounds**: When text appears over a gradient, test against the lightest AND darkest points of the gradient. The weakest contrast point must still meet the threshold.
4. **Disabled state minimum 3:1**: Disabled text and disabled controls must still meet 3:1 contrast against their background (WCAG 1.4.11 Non-text Contrast). Lighten with care — graying out can easily drop below 3:1.
5. **Error states need icon + text, not just color**: Error indicators must pair the red color token with an icon (⚠️, ✗) or text label ("Error: ...") to satisfy WCAG 1.4.1 Use of Color.
6. **Hover/focus states**: Verify that hover and focus color changes don't reduce contrast below the baseline. A hover state that lightens text against the same background may drop from 4.5:1 to below threshold.
7. **Component-specific pairs to validate**:
   - `--tp-primary` on `--tp-bg` (buttons, links)
   - `--tp-primary` on `--tp-surface` (secondary buttons, chips)
   - `--tp-error` on `--tp-bg` (error messages, validation)
   - `--tp-text` on `--tp-bg` (body text)
   - `--tp-text-muted` on `--tp-bg` (helper text, captions)
   - `--tp-text` on `--tp-surface` (cards, panels)
   - `--tp-primary` on `--tp-primary` background (text on primary-colored buttons — use white/near-white text)

---

## Testing Methodology

### Automated Testing Tools

| Tool | Best For | Coverage | Integration |
|------|----------|----------|-------------|
| **axe-core** | Unit/component/E2E tests | ~57% of WCAG issues, zero false positives | Jest, Vitest, Cypress, Playwright, Storybook |
| **pa11y** | CI smoke tests, multi-page regression | axe + htmlcs runners | CLI, pa11y-ci |
| **Lighthouse** | Quick audits, trend tracking | Weighted a11y score (manual audits not scored) | Chrome DevTools, CI, PageSpeed Insights |
| **WAVE** | Visual/manual inspection | Page-wide findings with visual overlays | Browser extension, API |
| **Accessibility Insights** | FastTab/Assessment workflows | Guided manual+automated testing | Browser extension (FastPass for quick, Assessment for full) |

**Automated testing strategy**:
1. Run axe-core in component tests (unit level)
2. Run pa11y-ci in CI pipeline (regression guard)
3. Run Lighthouse periodically (trend tracking)
4. None replace manual testing — automated catches ~30-57% of issues

### Manual Testing Techniques

**Keyboard-only walkthrough** (critical, catches most operability issues):
1. Put mouse away
2. Tab through entire page — every interactive element must be reachable
3. Verify Tab/Shift+Tab order matches visual layout
4. Test Enter/Space activation on buttons/links
5. Test arrow keys in composite widgets (tabs, menus, listboxes)
6. Test Escape closes modals/popups/dropdowns
7. Verify no keyboard traps

**Screen reader testing**:
- **Windows**: NVDA (free) + Firefox/Chrome, JAWS + Chrome/Edge
- **macOS**: VoiceOver (built-in) + Safari (Cmd+F5 to toggle)
- **iOS**: VoiceOver + Safari (Settings > Accessibility)
- **Android**: TalkBack + Chrome (Settings > Accessibility)

**What to test with screen reader**:
1. Reading order (swipe/arrow through content)
2. Headings navigation (H key in NVDA/JAWS, rotor in VoiceOver)
3. Landmarks navigation (D key)
4. Link/button names are descriptive
5. Form labels and error messages announced
6. Modal behavior (focus trap, announcements)
7. Dynamic updates announced (live regions)
8. Image alt text meaningful

**Zoom/reflow testing**:
1. Browser zoom to 200% — text scales, no horizontal scroll, no content loss
2. Zoom to 400% — content reflows to single column at 320px equivalent
3. No fixed-width containers breaking layout

**Color contrast testing**:
- Use WebAIM Contrast Checker for specific color pairs
- Use WAVE for page-wide contrast findings
- Verify 4.5:1 for normal text, 3:1 for large text (AA)
- Verify 3:1 for UI components and graphical objects

---

## Platform-Specific Guidance

### Web (HTML/CSS/JS)

- **Semantic HTML first**: `<button>`, `<a>`, `<input>`, `<select>`, `<nav>`, `<main>`
- **Landmarks**: Use `<header>`, `<nav>`, `<main>`, `<aside>`, `<footer>` for page structure
- **Focus styles**: Never `outline: none` without replacement; use visible custom focus rings
- **Reduced motion**: Wrap animations in `@media (prefers-reduced-motion: no-preference)`
- **Color**: Never use color as sole indicator; add icons, patterns, or text labels
- **Responsive**: Use relative units (rem, em), flexible grids, test at 320px width

### React / Next.js

- Use native elements: `<button>`, `<a href>`, `<input>` — not divs with onClick
- Keep `aria-*` attributes valid and kebab-cased
- Install `eslint-plugin-jsx-a11y` for lint-time accessibility checks
- Manage focus across re-renders and route transitions (use `useRef` + `.focus()`)
- Ensure client-side route changes are announced (Next.js has route announcer built in)
- Use `next/image` with meaningful `alt` props
- Sources: https://react.dev/learn/accessibility, https://nextjs.org/docs/accessibility

### Android

- Set `contentDescription` on all meaningful ImageViews/ImageButtons
- Use `android:importantForAccessibility="no"` for decorative images
- Touch targets: minimum 48×48dp
- Test with TalkBack, Switch Access, Voice Access
- Use Accessibility Scanner app for automated suggestions
- Source: https://developer.android.com/guide/topics/ui/accessibility/apps

### iOS / macOS

- Set `accessibilityLabel` on all meaningful UI elements
- Use `accessibilityHint` for complex interactions
- Set `accessibilityTraits` to describe element type
- Group related elements with `accessibilityElementContainers`
- Test with VoiceOver and Accessibility Inspector
- Sources: https://developer.apple.com/documentation/accessibility

### React Native

- Use `accessibilityLabel` for element names
- Use `accessibilityHint` for action descriptions
- Use `accessibilityRole` for element types
- Use `accessibilityState` for checked/selected/disabled
- Use `accessibilityLiveRegion` for dynamic updates
- Test on both iOS (VoiceOver) and Android (TalkBack)
- Source: https://reactnative.dev/docs/accessibility

### Desktop (Windows / Electron)

- Expose proper UI Automation names/roles/patterns
- Custom controls need automation peers (Windows) or accessibility protocols (macOS)
- Electron: enable `app.accessibilitySupport.enabled`, test like web + verify desktop AT support
- Sources: https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-overview, https://electronjs.org/docs/latest/tutorial/accessibility

### Blazor / MudBlazor

- Use `@bind-IsDarkMode` on `MudThemeProvider` with `ObserveSystemDarkModeChange="false"` to respect user preference over OS setting
- MudBlazor v8+ requires explicit generic types: `MudTextField<string>`, `MudSelect<string>`, `MudChip<string>`
- Use `@ref` + `.FocusAsync()` for programmatic focus management in Blazor components
- Shared component libraries: inject services as `private ILogger<T>?` with nullable access — never use `null!` suppression
- JS interop: use `IJSObjectReference` for ES module imports + `IAsyncDisposable` for cleanup — never call global functions
- Route changes: move focus to `<main>` content area; update `<title>` per route
- MudBlazor components may trigger axe-core violations (link-in-text-block, color-contrast) — document justified rule disables
- `MudDialog` supports `Autofocus` parameter for initial focus inside dialogs
- Use `<MudProgressLinear Indeterminate="true" />` instead of spinning text for loading states

**Tour Overlay Accessibility**:

Tour overlays (guided walkthroughs) must follow the dialog pattern for screen reader and keyboard users.

```razor
@* Accessible Tour Overlay — Razor/Blazor pattern *@
@implements IAsyncDisposable

@if (_isActive)
{
    <div class="tour-backdrop" aria-hidden="true"></div>
    <div role="dialog"
         aria-modal="true"
         aria-labelledby="tour-step-title"
         aria-describedby="tour-step-desc"
         class="tour-overlay">
        <h2 id="tour-step-title">@_steps[_currentStep].Title</h2>
        <p id="tour-step-desc">@_steps[_currentStep].Description</p>
        <div class="tour-actions">
            <MudButton OnClick="NextStep" Color="Color.Primary">
                @(_currentStep < _steps.Length - 1 ? "Next" : "Finish")
            </MudButton>
            <MudButton OnClick="SkipTour" Color="Color.Default">
                Skip
            </MudButton>
        </div>
        <span class="tour-counter" aria-live="polite">
            Step @_currentStepFormatted of @_steps.Length
        </span>
    </div>
}

@code {
    private bool _isActive;
    private int _currentStep;
    private ElementReference _triggerElement;
    private TourStep[] _steps = [];

    private string _currentStepFormatted => (_currentStep + 1).ToString();

    public async Task StartTour(ElementReference trigger)
    {
        _triggerElement = trigger;
        _currentStep = 0;
        _isActive = true;
        // Focus moves into dialog on open
        await Task.Delay(50); // allow DOM render
    }

    private async Task NextStep()
    {
        if (_currentStep < _steps.Length - 1)
        {
            _currentStep++;
        }
        else
        {
            await CloseTour();
        }
    }

    private async Task SkipTour()
    {
        await CloseTour();
    }

    private async Task CloseTour()
    {
        _isActive = false;
        // Return focus to the element that opened the tour
        await _triggerElement.FocusAsync();
    }

    public async ValueTask DisposeAsync()
    {
        if (_isActive) await CloseTour();
    }
}
```

**Tour a11y requirements**:
- `role="dialog"` + `aria-modal="true"` to trap focus within the tour step
- Each step has a visible title (`aria-labelledby`) and description (`aria-describedby`)
- Focus moves into the dialog on open, returns to trigger element on close
- "Skip" button is always available (never hidden or disabled)
- Step counter uses `aria-live="polite"` to announce progress
- Escape key closes the tour (provided by `MudDialog` or manual `@onkeydown`)
- Dark mode: tour backdrop and overlay must meet contrast requirements in both themes
- Test with keyboard-only: Tab cycles within dialog, Escape exits

**Breadcrumb Accessibility**:

```razor
@* Accessible Breadcrumb — Razor/Blazor pattern *@
<nav aria-label="Breadcrumb">
    <ol class="breadcrumb-list">
        @foreach (var crumb in _crumbs.Take(_crumbs.Length - 1))
        {
            <li class="breadcrumb-item">
                <a href="@crumb.Href">@crumb.Label</a>
                <span aria-hidden="true">/</span>
            </li>
        }
        @{
            var current = _crumbs.Last();
        }
        <li class="breadcrumb-item active" aria-current="page">
            <span>@current.Label</span>
        </li>
    </ol>
</nav>
```

**Breadcrumb a11y requirements**:
- Wrap in `<nav aria-label="Breadcrumb">` so screen readers identify it as a navigation landmark
- Last item is the current page: plain text (not a link) with `aria-current="page"`
- Separators (e.g., `/`, `>`) have `aria-hidden="true"` — they are decorative
- Use an ordered list (`<ol>`) since breadcrumb order is meaningful
- Each link's purpose must be clear from its text alone (2.4.4, 2.4.9)

**Language Switcher Accessibility**:

```razor
@* Accessible Language Switcher — Razor/Blazor pattern *@
<MudSelect<string>"
          T="string"
          Label="@_localizer["Language"]"
          aria-label="Select language"
          @bind-Value="_currentLanguage"
         ValueChanged="OnLanguageChanged">
    <MudSelectItem T="string" Value=""en"">English</MudSelectItem>
    <MudSelectItem T="string" Value=""pt-BR"">Português</MudSelectItem>
    <MudSelectItem T="string" Value=""es"">Español</MudSelectItem>
    <MudSelectItem T="string" Value=""gn"">Avañe'ẽ</MudSelectItem>
</MudSelect<string>>

<div id="lang-announcer" aria-live="polite" class="sr-only"></div>

@code {
    private string _currentLanguage = "pt-BR";

    private async Task OnLanguageChanged(string lang)
    {
        _currentLanguage = lang;
        // Announce the change to screen readers
        var name = lang switch
        {
            "en" => "English",
            "pt-BR" => "Português",
            "es" => "Español",
            "gn" => "Avañe'ẽ",
            _ => lang
        };
        // JS interop to update announcer text
        await Announce($"Language changed to {name}");
    }

    private async Task Announce(string message)
    {
        // Use IJSObjectReference module, not global function
        // await _jsModule.InvokeVoidAsync("announce", "lang-announcer", message);
    }
}
```

**Language switcher a11y requirements**:
- Must have a visible label or `aria-label` (never rely on flag icons alone)
- Announce the change via `aria-live="polite"` region so screen readers confirm the switch
- Use native language names in their own script: "Português" not "Portuguese", "Español" not "Spanish" (3.1.2)
- Never use flag icons as the sole indicator of language — flags represent countries, not languages

**Additional MudBlazor Accessibility Notes**:
- `MudDataGrid<T>` handles `aria-sort` automatically on sortable columns — prefer it over `MudTable` for data tables
- `MudSelect<T>` requires explicit `T="string"` (or the correct type) in v8+ for proper ARIA role and option announcement
- Use `<MudProgressLinear Indeterminate="true" aria-label="Loading content" />` — always add `aria-label` for screen readers
- Loading states: prefer `MudProgressLinear` over text-based spinners like "Loading..." — progress bars communicate status semantically

---

## Remediation Workflow (Fix Mode)

When fixing a known accessibility issue, follow this systematic approach:

### Step 1: Identify the Issue

1. Determine the WCAG criterion violated (use the quick reference tables above)
2. Identify the severity: Level A (critical), AA (standard), AAA (aspirational)
3. Understand the impact: which disabilities are affected?

### Step 2: Analyze the Root Cause

1. Read the current code for the affected component
2. Determine if it's a markup issue (wrong elements), ARIA issue (missing/wrong attributes), CSS issue (contrast, visibility, layout), or logic issue (focus management, keyboard handlers)
3. Check if a native HTML element can solve it before adding ARIA

### Step 3: Apply the Fix

**Markup fixes** (most common):
- Replace div/span with semantic elements (button, a, label, nav)
- Add missing alt text, labels, headings
- Fix heading hierarchy (h1 → h2 → h3, no skips)
- Add lang attribute, autocomplete tokens

**ARIA fixes**:
- Add role, aria-label, aria-labelledby, aria-describedby where native semantics insufficient
- Set aria-invalid, aria-expanded, aria-selected for states
- Add aria-live regions for dynamic updates
- Ensure accessible name contains visible label text (Label in Name)

**CSS fixes**:
- Restore visible focus indicators
- Fix contrast ratios (text 4.5:1, UI components 3:1)
- Use relative units instead of fixed px
- Add prefers-reduced-motion media query
- Remove content clipping at zoom levels

**JavaScript fixes**:
- Add keyboard event handlers (Enter, Space, Escape, arrows)
- Implement focus management (modal trap, focus return, route announcer)
- Update ARIA states on interaction (aria-expanded, aria-selected)
- Announce dynamic changes via live regions

### Step 4: Verify the Fix

1. Run automated tool (axe-core) on the fixed component
2. Test with keyboard only (Tab, Enter, Space, Escape, Arrows)
3. Test with screen reader (NVDA/VoiceOver) if applicable
4. Test zoom/reflow if the fix affects layout
5. Verify no regressions in surrounding components

---

## Legal and Regulatory Reference

| Regulation | Region | Technical Standard | Scope |
|------------|--------|--------------------|-------|
| **ADA** | United States | DOJ points to WCAG/Section 508 | Public accommodations, state/local gov |
| **Section 508** | US Federal | WCAG 2.0 AA (Revised 508) | Federal ICT, web and non-web |
| **EN 301 549** | European Union | WCAG 2.1 AA (v3.2.1) | Websites, apps, software, hardware |
| **European Accessibility Act** | EU | EN 301 549 as benchmark | Products and services (effective 2025-2030) |
| **AODA** | Ontario, Canada | WCAG 2.0 AA | Public websites, with some exceptions |

> **Practical target**: WCAG 2.2 Level AA satisfies or exceeds most regulatory requirements worldwide. Always verify specific obligations with legal counsel.

---

## Accessible Widget Patterns

Reference implementations for common complex UI widgets. Cross-reference: https://www.w3.org/WAI/ARIA/apg/patterns/

### Accordion (Sections)

```html
<div class="accordion">
  <h3>
    <button aria-expanded="false" aria-controls="section-1" id="acc-btn-1">
      Section Title
    </button>
  </h3>
  <div id="section-1" role="region" aria-labelledby="acc-btn-1" hidden>
    <p>Section content...</p>
  </div>
</div>
```
**Keyboard**: Enter/Space toggles section, Tab moves between accordion headers.

### Tabs

```html
<div class="tabs">
  <div role="tablist" aria-label="Settings">
    <button role="tab" id="tab-1" aria-selected="true" aria-controls="panel-1">Account</button>
    <button role="tab" id="tab-2" aria-selected="false" aria-controls="panel-2" tabindex="-1">Security</button>
  </div>
  <div role="tabpanel" id="panel-1" aria-labelledby="tab-1" tabindex="0">Account settings...</div>
  <div role="tabpanel" id="panel-2" aria-labelledby="tab-2" tabindex="0" hidden>Security settings...</div>
</div>
```
**Keyboard**: Arrow keys move between tabs, Home/End for first/last, Enter/Space activates tab.

### Tooltip

```html
<button aria-describedby="tip-1">Help</button>
<div id="tip-1" role="tooltip">Click to learn more about this feature.</div>
```
**Behavior**: Show on focus/hover, dismiss with Escape, remain visible while hovering tooltip itself.

### Combobox (Autocomplete)

```html
<label for="combo">Country</label>
<div class="combobox">
  <input type="text" id="combo" role="combobox"
    aria-autocomplete="list" aria-expanded="false"
    aria-controls="combo-list" aria-activedescendant="">
  <ul id="combo-list" role="listbox" aria-label="Countries">
    <li id="opt-1" role="option">Argentina</li>
    <li id="opt-2" role="option">Brazil</li>
  </ul>
</div>
```
**Keyboard**: Arrow keys navigate options, Enter selects, Escape closes list, typing filters.

### Disclosure Button (Show/Hide)

```html
<button aria-expanded="false" aria-controls="details-panel">
  Show Details
</button>
<div id="details-panel" hidden>
  Detailed content here...
</div>
```

### Data Table with Sortable Headers

```html
<table>
  <caption>Quarterly Revenue</caption>
  <thead>
    <tr>
      <th scope="col" aria-sort="ascending">
        <button>Quarter <span aria-hidden="true">↑</span></button>
      </th>
      <th scope="col" aria-sort="none">
        <button>Revenue</button>
      </th>
    </tr>
  </thead>
  <tbody>
    <tr><td>Q1 2026</td><td>$1.2M</td></tr>
    <tr><td>Q2 2026</td><td>$1.5M</td></tr>
  </tbody>
</table>
```
**Keyboard**: Enter/Space on header button sorts column, toggles ascending/descending.

---

## Testing Code Examples

### axe-core in Vitest/Jest

```javascript
import { axe, toHaveNoViolations } from 'jest-axe';
expect.extend(toHaveNoViolations);

test('button has no accessibility violations', async () => {
  const { container } = render(<MyButton>Click me</MyButton>);
  const results = await axe(container);
  expect(results).toHaveNoViolations();
});
```

### axe-core in Playwright

```javascript
import { injectAxe, checkA11y } from 'axe-playwright';

test('homepage is accessible', async ({ page }) => {
  await page.goto('/');
  await injectAxe(page);
  await checkA11y(page, null, {
    detailedReport: true,
    detailedReportOptions: { html: true },
  });
});
```

### pa11y-ci Config (.pa11yci.json)

```json
{
  "defaults": {
    "timeout": 30000,
    "concurrency": 4,
    "standard": "WCAG2AA",
    "runners": ["axe"],
    "ignore": ["notice", "warning"],
    "paths": [
      "http://localhost:3000/",
      "http://localhost:3000/about",
      "http://localhost:3000/contact"
    ]
  }
}
```

### Cypress with cypress-axe

```javascript
// cypress/support/e2e.js
import 'cypress-axe';

// In test:
cy.visit('/login');
cy.injectAxe();
cy.checkA11y(null, {
  runOnly: ['wcag2a', 'wcag2aa'],
  rules: { 'color-contrast': { enabled: true } },
});
```

### Playwright.Axe in .NET 10

Install the NuGet package ([GitHub](https://github.com/IsaacWalker/PlaywrightAxeDotnet)):

```bash
dotnet add package Playwright.Axe
```

**Basic test (xUnit):**

```csharp
using Microsoft.Playwright;
using Playwright.Axe;

public class AccessibilityTests : IAsyncLifetime
{
    private IPlaywright _playwright;
    private IBrowser _browser;
    private IPage _page;

    public async Task InitializeAsync()
    {
        _playwright = await Microsoft.Playwright.Playwright.CreateAsync();
        _browser = await _playwright.Chromium.LaunchAsync(new() { Headless = true });
        _page = await _browser.NewPageAsync();
    }

    [Fact]
    public async Task Homepage_HasNoAccessibilityViolations()
    {
        await _page.GotoAsync("http://localhost:5000");

        var results = await _page.RunAxe(new AxeRunOptions
        {
            // Optionally restrict to specific rules/tags
            RunOnly = new List<string> { "wcag2a", "wcag2aa" }
        });

        // Assert no violations
        Assert.Empty(results.Violations);
    }

    [Fact]
    public async Task LoginForm_HasNoAccessibilityViolations()
    {
        await _page.GotoAsync("http://localhost:5000/login");

        // Test only a specific element
        var form = _page.Locator("form");
        var results = await form.RunAxe();

        Assert.Empty(results.Violations);
    }

    [Fact]
    public async Task Form_AfterValidationError_HasNoViolations()
    {
        await _page.GotoAsync("http://localhost:5000/login");
        await _page.Locator("button[type='submit']").ClickAsync();

        // Test after dynamic content changes
        var results = await _page.RunAxe();
        Assert.Empty(results.Violations);
    }

    public async Task DisposeAsync()
    {
        _page?.Dispose();
        await _browser?.CloseAsync();
        _playwright?.Dispose();
    }
}
```

**NUnit variant:**

```csharp
using NUnit.Framework;
using Microsoft.Playwright;
using Playwright.Axe;

[TestFixture]
public class AccessibilityTests
{
    private IPlaywright _playwright;
    private IBrowser _browser;
    private IPage _page;

    [SetUp]
    public async Task Setup()
    {
        _playwright = await Microsoft.Playwright.Playwright.CreateAsync();
        _browser = await _playwright.Chromium.LaunchAsync(new() { Headless = true });
        _page = await _browser.NewPageAsync();
    }

    [TearDown]
    public async Task Teardown()
    {
        _page?.Dispose();
        await _browser?.CloseAsync();
        _playwright?.Dispose();
    }

    [Test]
    public async Task Homepage_IsAccessible()
    {
        await _page.GotoAsync("http://localhost:5000");
        var results = await _page.RunAxe();
        Assert.That(results.Violations, Is.Empty,
            results.Violations.FirstOrDefault()?.Description ?? "Accessibility violations found");
    }
}
```

**Filtering rules:**

```csharp
var results = await _page.RunAxe(new AxeRunOptions
{
    // Include/exclude specific rules
    Rules = new Dictionary<string, bool>
    {
        { "color-contrast", true },   // enable
        { "document-title", true },   // enable
    },
    // Exclude a section (e.g., third-party widgets)
    Exclude = new List<string> { "#third-party-widget" },
    // Include only a section
    Include = new List<string> { "#main-content" }
});
```

**CI integration** (GitHub Actions):

```yaml
- name: Run accessibility tests
  run: dotnet test --filter "Category=Accessibility"
```

### Playwright + axe-core in Python

Install dependencies:

```bash
pip install playwright axe-playwright-python pytest-playwright
playwright install
```

**Basic test (pytest):**

```python
import pytest
from playwright.sync_api import Page
from axe_playwright_python.sync_playwright import Axe


def test_homepage_has_no_accessibility_violations(page: Page):
    page.goto("http://localhost:5000")

    axe = Axe()
    results = axe.run(page)

    violations = results.get("violations", [])
    assert len(violations) == 0, _format_violations(violations)


def test_login_form_has_no_violations(page: Page):
    page.goto("http://localhost:5000/login")

    # Test only a specific element
    form = page.locator("form")
    axe = Axe()
    results = axe.run(form)

    assert len(results.get("violations", [])) == 0


def test_after_validation_errors_has_no_violations(page: Page):
    page.goto("http://localhost:5000/login")
    page.locator("button[type='submit']").click()

    # Test after dynamic content changes
    axe = Axe()
    results = axe.run(page)
    assert len(results.get("violations", [])) == 0


def _format_violations(violations):
    """Format axe violations for readable error output."""
    messages = []
    for v in violations:
        nodes = [n["html"] for n in v.get("nodes", [])]
        messages.append(f"- {v['id']}: {v['description']} → {nodes}")
    return "\n".join(messages)
```

**Async variant (pytest-asyncio):**

```python
import pytest
from playwright.async_api import Page
from axe_playwright_python.async_playwright import Axe


@pytest.mark.asyncio
async def test_homepage_accessible(page: Page):
    await page.goto("http://localhost:5000")

    axe = Axe()
    results = await axe.run(page)

    violations = results.get("violations", [])
    assert len(violations) == 0, f"Found {len(violations)} accessibility violations"
```

**Run:**

```bash
pytest tests/accessibility/ --browser chromium
```

### Playwright + axe-core in Java

Add Maven dependencies (`pom.xml`):

```xml
<dependencies>
    <dependency>
        <groupId>com.microsoft.playwright</groupId>
        <artifactId>playwright</artifactId>
        <version>1.49.0</version>
        <scope>test</scope>
    </dependency>
    <dependency>
        <groupId>org.junit.jupiter</groupId>
        <artifactId>junit-jupiter</artifactId>
        <version>5.11.0</version>
        <scope>test</scope>
    </dependency>
</dependencies>
```

Create `AxeTestHelper.java`:

```java
import com.microsoft.playwright.ElementHandle;
import com.microsoft.playwright.Page;
import com.microsoft.playwright.Playwright;
import java.util.List;
import java.util.Map;
import static org.junit.jupiter.api.Assertions.*;

public class AxeTestHelper {

    /**
     * Injects axe-core and runs accessibility audit on the page.
     * Uses axe-core from CDN. For offline use, host locally.
     */
    @SuppressWarnings("unchecked")
    public static void assertNoViolations(Page page) {
        // Inject axe-core from CDN
        page.evaluate(
            "async () => { " +
            "  if (!window.axe) { " +
            "    const script = document.createElement('script'); " +
            "    script.src = 'https://cdnjs.cloudflare.com/ajax/libs/axe-core/4.10.2/axe.min.js'; " +
            "    document.head.appendChild(script); " +
            "    await new Promise(r => script.onload = r); " +
            "  } " +
            "}"
        );

        Map<String, Object> results = (Map<String, Object>) page.evaluate(
            "async () => await axe.run(document, { runOnly: ['wcag2a', 'wcag2aa'] })"
        );

        List<Map<String, Object>> violations = (List<Map<String, Object>>) results.get("violations");
        if (!violations.isEmpty()) {
            StringBuilder sb = new StringBuilder("Accessibility violations found:\n");
            for (Map<String, Object> v : violations) {
                sb.append(String.format("- %s: %s\n", v.get("id"), v.get("description")));
                List<Map<String, Object>> nodes = (List<Map<String, Object>>) v.get("nodes");
                for (Map<String, Object> node : nodes) {
                    sb.append(String.format("    → %s\n", node.get("html")));
                }
            }
            fail(sb.toString());
        }
    }

    /** Run axe on a specific element only */
    public static void assertNoViolations(Page page, String selector) {
        page.evaluate(
            "async () => { " +
            "  if (!window.axe) { " +
            "    const script = document.createElement('script'); " +
            "    script.src = 'https://cdnjs.cloudflare.com/ajax/libs/axe-core/4.10.2/axe.min.js'; " +
            "    document.head.appendChild(script); " +
            "    await new Promise(r => script.onload = r); " +
            "  } " +
            "}"
        );

        Map<String, Object> results = (Map<String, Object>) page.evaluate(
            "async (sel) => await axe.run(document.querySelector(sel), { runOnly: ['wcag2a', 'wcag2aa'] })",
            selector
        );

        List<Map<String, Object>> violations = (List<Map<String, Object>>) results.get("violations");
        assertTrue(violations.isEmpty(),
            "Found " + violations.size() + " accessibility violations in " + selector);
    }
}
```

**JUnit 5 test:**

```java
import com.microsoft.playwright.*;
import org.junit.jupiter.api.*;

class AccessibilityTests {
    static Playwright playwright;
    static Browser browser;
    Page page;

    @BeforeAll
    static void launchBrowser() {
        playwright = Playwright.create();
        browser = playwright.chromium().launch(new BrowserType.LaunchOptions().setHeadless(true));
    }

    @BeforeEach
    void createPage() {
        page = browser.newPage();
    }

    @AfterEach
    void closePage() {
        page.close();
    }

    @AfterAll
    static void closeBrowser() {
        browser.close();
        playwright.close();
    }

    @Test
    void homepageHasNoViolations() {
        page.navigate("http://localhost:5000");
        AxeTestHelper.assertNoViolations(page);
    }

    @Test
    void loginFormHasNoViolations() {
        page.navigate("http://localhost:5000/login");
        AxeTestHelper.assertNoViolations(page, "form");
    }

    @Test
    void afterValidationErrors_noViolations() {
        page.navigate("http://localhost:5000/login");
        page.locator("button[type='submit']").click();
        AxeTestHelper.assertNoViolations(page);
    }
}
```

**Run:**

```bash
mvn test -Dtest="AccessibilityTests"
```

### Playwright + axe-core in Go

Install dependencies:

```bash
go get github.com/playwright-community/playwright-go
```

Create `axe_test.go`:

```go
package accessibility

import (
	"encoding/json"
	"fmt"
	"testing"

	playwright "github.com/playwright-community/playwright-go"
)

func TestMain(m *testing.M) {
	playwright.Install()
	m.Run()
}

func runAxe(page playwright.Page, selector ...string) ([]map[string]interface{}, error) {
	// Inject axe-core from CDN
	_, err := page.Evaluate(`async () => {
		if (!window.axe) {
			const script = document.createElement('script');
			script.src = 'https://cdnjs.cloudflare.com/ajax/libs/axe-core/4.10.2/axe.min.js';
			document.head.appendChild(script);
			await new Promise(r => script.onload = r);
		}
	}`)
	if err != nil {
		return nil, fmt.Errorf("failed to inject axe: %w", err)
	}

	target := "document"
	opts := `{ runOnly: ['wcag2a', 'wcag2aa'] }`
	if len(selector) > 0 {
		target = fmt.Sprintf("document.querySelector('%s')", selector[0])
	}

	js := fmt.Sprintf(`async () => {
		const results = await axe.run(%s, %s);
		return JSON.stringify(results);
	}`, target, opts)

	raw, err := page.Evaluate(js)
	if err != nil {
		return nil, fmt.Errorf("axe.run failed: %w", err)
	}

	var results map[string]interface{}
	json.Unmarshal([]byte(raw.(string)), &results)

	violations, _ := results["violations"].([]interface{})
	var typed []map[string]interface{}
	for _, v := range violations {
		typed = append(typed, v.(map[string]interface{}))
	}
	return typed, nil
}

func assertNoViolations(t *testing.T, page playwright.Page, selector ...string) {
	t.Helper()
	violations, err := runAxe(page, selector...)
	if err != nil {
		t.Fatalf("axe execution failed: %v", err)
	}
	if len(violations) > 0 {
		for _, v := range violations {
			t.Errorf("- %s: %s", v["id"], v["description"])
		}
		t.Fatalf("Found %d accessibility violations", len(violations))
	}
}

func TestHomepage_Accessibility(t *testing.T) {
	pw, err := playwright.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer pw.Stop()

	browser, err := pw.Chromium.Launch(playwright.BrowserTypeLaunchOptions{Headless: playwright.Bool(true)})
	if err != nil {
		t.Fatal(err)
	}
	defer browser.Close()

	page, err := browser.NewPage()
	if err != nil {
		t.Fatal(err)
	}
	defer page.Close()

	page.Goto("http://localhost:5000")
	assertNoViolations(t, page)
}

func TestLoginForm_Accessibility(t *testing.T) {
	pw, err := playwright.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer pw.Stop()

	browser, err := pw.Chromium.Launch(playwright.BrowserTypeLaunchOptions{Headless: playwright.Bool(true)})
	if err != nil {
		t.Fatal(err)
	}
	defer browser.Close()

	page, err := browser.NewPage()
	if err != nil {
		t.Fatal(err)
	}
	defer page.Close()

	page.Goto("http://localhost:5000/login")
	assertNoViolations(t, page, "form")
}
```

**Run:**

```bash
go test ./tests/accessibility/ -v
```

---

## Mobile-Specific Accessibility

### Touch Targets
- Minimum 44×44pt (iOS) / 48×48dp (Android)
- For WCAG 2.5.8: minimum 24×24 CSS px with spacing exception (equivalent circle diameter ≥ 24px, measured center-to-center)
- Adjacent targets should have adequate spacing to prevent accidental activation

### Gesture Alternatives
| Gesture | Alternative | WCAG Criterion |
|---------|------------|----------------|
| Pinch to zoom | +/- buttons, zoom control | 2.5.1 |
| Swipe to navigate | Previous/Next buttons | 2.5.1 |
| Drag to reorder | Move up/down buttons, menu | 2.5.7 |
| Long press for options | Context menu button | 2.5.1 |
| Pull to refresh | Refresh button | 2.5.1 |

### Responsive Focus Management
- Ensure focus moves to new content on viewport changes
- Test with software keyboards open — content must not be obscured by keyboard
- Use `touch-action` CSS to control gesture handling without blocking accessibility

### Dark Mode Accessibility

When implementing dark/light mode:

- **Verify contrast in BOTH themes**: Colors passing WCAG in light mode may fail in dark mode. Test all text/background pairs in both states.
- **Flash prevention (FOUC)**: Add inline `<script>` in `<head>` BEFORE CSS `<link>` elements. Script reads `localStorage` and sets `data-mud-theme="dark"` on `<html>` before browser paints.
- **Transition animations**: Wrap theme transitions in `@media (prefers-reduced-motion: no-preference)` — some users are sensitive to theme-switching animations.
- **CSS custom properties**: Define colors as CSS variables at `:root` (light) and `[data-mud-theme="dark"]` (dark). This enables a11y testing across themes by toggling one attribute.
- **Animated counters/statistics**: Use `prefers-reduced-motion` media query — provide instant number display as fallback when animations are disabled.
- **Theme persistence**: Respect user choice — never auto-switch based on OS preference without explicit consent. Always provide a manual toggle.

---

## Additional Resources

- **WCAG 2.2 Quick Reference**: https://www.w3.org/WAI/WCAG22/quickref/?versions=2.2
- **WAI-ARIA Authoring Practices Guide (APG)**: https://www.w3.org/WAI/ARIA/apg/
- **MDN Accessibility Guide**: https://developer.mozilla.org/en-US/docs/Web/Accessibility
- **WebAIM Resources**: https://webaim.org/resources/
- **A11y Project Checklist**: https://www.a11yproject.com/checklist/
- **Inclusive Components**: https://inclusive-components.design/

---

## Anti-Patterns (Blazor / MudBlazor Accessibility)

Common accessibility mistakes in Blazor/MudBlazor applications and how to fix them. Each anti-pattern includes a ❌ WRONG and ✅ RIGHT example.

### 1. Using `aria-label` on Elements That Already Have Visible Text

Adding `aria-label` to an element that already has visible text creates a duplicate or conflicting announcement for screen readers. The accessible name should be derived from the visible text, not overridden.

```razor
@* ❌ WRONG: aria-label overrides the visible text *@
<MudButton aria-label="Submit order form">@_localizer["Submit"]</MudButton>

@* ✅ RIGHT: Let the visible text be the accessible name *@
<MudButton>@_localizer["Submit"]</MudButton>
```

**WCAG**: 2.5.3 Label in Name (Level A) — the accessible name must contain the visible label text.

### 2. Relying Solely on Color to Convey Information

Using only color to indicate status, errors, or categories fails for color-blind users and screen readers. Always pair color with text, icons, or patterns.

```razor
@* ❌ WRONG: status indicated only by color *@
<MudChip Color="Color.Success">Active</MudChip>
<MudChip Color="Color.Error">Suspended</MudChip>

@* ✅ RIGHT: color + icon + text label *@
<MudChip Color="Color.Success">
    <MudIcon Icon="@Icons.Material.Filled.CheckCircle" />
    @(_localizer["Status_Active"])
</MudChip>
<MudChip Color="Color.Error">
    <MudIcon Icon="@Icons.Material.Filled.Cancel" />
    @(_localizer["Status_Suspended"])
</MudChip>
```

**WCAG**: 1.4.1 Use of Color (Level A) — color must not be the only visual means of conveying information.

### 3. Missing `alt` Text on Decorative vs Informative Images

Decorative images must be hidden from assistive technology. Informative images must have meaningful alt text.

```razor
@* ❌ WRONG: decorative image announced as "decorative-line.png" *@
<img src="/images/divider.png" />

@* ❌ WRONG: informative image has no alt text *@
<img src="/images/product-preview.png" />

@* ✅ RIGHT: decorative image hidden from AT *@
<img src="/images/divider.png" alt="" aria-hidden="true" />

@* ✅ RIGHT: informative image with descriptive alt *@
<img src="/images/product-preview.png" alt="Product catalog preview showing 12 items in grid layout" />
```

**WCAG**: 1.1.1 Non-text Content (Level A) — all non-text content must have a text alternative.

### 4. Using `@onclick` on Non-Interactive Elements

Wiring click handlers to `<div>` or `<span>` elements creates inaccessible controls — no keyboard support, no role, no focus indicator.

```razor
@* ❌ WRONG: div with click handler — no keyboard, no role *@
<div @onclick="DeleteOrder" class="delete-btn">Delete</div>

@* ✅ RIGHT: native button with keyboard support and role *@
<button @onclick="DeleteOrder" class="delete-btn" aria-label="Delete order">
    @(_localizer["Delete"])
</button>
```

**WCAG**: 2.1.1 Keyboard (Level A), 4.1.2 Name, Role, Value (Level A).

### 5. Skipping Focus Management After Async Operations

Blazor re-renders can shift focus away from the current element. After async state changes (save, delete, navigation), restore focus to a meaningful location.

```razor
@* ❌ WRONG: focus disappears after save *@
<MudButton OnClick="SaveCustomer">Save</MudButton>
@code {
    private async Task SaveCustomer()
    {
        await _customerService.SaveAsync(_customer);
        _saved = true;
    }
}

@* ✅ RIGHT: focus moves to confirmation or back to form *@
<MudButton OnClick="SaveCustomer" @ref="_saveButton">Save</MudButton>
<MudText id="save-confirmation" style="@(_saved ? "" : "display:none")">
    Customer saved successfully.
</MudText>

@code {
    private MudButton _saveButton;
    private ElementReference _confirmationRef;
    private bool _saved;

    private async Task SaveCustomer()
    {
        await _customerService.SaveAsync(_customer);
        _saved = true;
        await _confirmationRef.FocusAsync();
    }
}
```

**WCAG**: 2.4.3 Focus Order (Level A), 4.1.3 Status Messages (Level AA).

### 6. Using `MudProgressLinear` Without Accessible Value Attributes

Screen readers cannot determine progress without proper ARIA attributes. Indeterminate progress bars must have an `aria-label`.

```razor
@* ❌ WRONG: screen reader announces unknown progress *@
<MudProgressLinear Progress="@_uploadProgress" />

@* ✅ RIGHT: determinate progress with accessible label *@
<MudProgressLinear Progress="@_uploadProgress"
                   aria-label="Order file upload progress"
                   aria-valuenow="@_uploadProgress"
                   aria-valuemin="0"
                   aria-valuemax="100" />

@* ✅ RIGHT: indeterminate progress with label *@
<MudProgressLinear Indeterminate="true"
                   aria-label="Loading customer data" />
```

**WCAG**: 4.1.2 Name, Role, Value (Level A) — value of progress bars must be programmatically determinable.

### 7. Placing `MudDialog` Outside Logical DOM Order

Dialogs must trap focus within their content and return focus to the trigger element on close.

```razor
@* ❌ WRONG: dialog opens but focus stays behind it *@
<MudButton OnClick="OpenConfirmDialog">Delete Customer</MudButton>
<MudDialog @bind-IsOpen="_dialogOpen">
    <DialogContent>Are you sure?</DialogContent>
    <DialogActions>
        <MudButton OnClick="ConfirmDelete">Yes, Delete</MudButton>
        <MudButton OnClick="CloseDialog">Cancel</MudButton>
    </DialogActions>
</MudDialog>

@* ✅ RIGHT: dialog with Autofocus and focus return *@
<MudButton OnClick="OpenConfirmDialog" @ref="_triggerButton">
    Delete Customer
</MudButton>
<MudDialog @bind-IsOpen="_dialogOpen" Autofocus="true">
    <DialogContent>Are you sure you want to delete this customer?</DialogContent>
    <DialogActions>
        <MudButton OnClick="ConfirmDelete" Color="Color.Error">
            Yes, Delete
        </MudButton>
        <MudButton OnClick="CloseDialog">Cancel</MudButton>
    </DialogActions>
</MudDialog>

@code {
    private MudButton _triggerButton;
    private bool _dialogOpen;

    private async Task CloseDialog()
    {
        _dialogOpen = false;
        await _triggerButton.FocusAsync();
    }
}
```

**WCAG**: 2.4.3 Focus Order (Level A), 2.1.2 No Keyboard Trap (Level A).

### 8. Custom Dropdown Components Without ARIA Listbox Pattern

Custom dropdowns built from scratch must implement the full ARIA listbox pattern. Prefer MudBlazor's built-in components.

```razor
@* ❌ WRONG: custom dropdown with no ARIA roles *@
<div class="dropdown" @onclick="ToggleDropdown">
    <span>@_selectedOption</span>
    <div class="dropdown-menu">
        @foreach (var option in _options)
        {
            <div @onclick="() => Select(option)">@option</div>
        }
    </div>
</div>

@* ✅ RIGHT: use MudSelect with proper ARIA support built in *@
<MudSelect T="string"
           Label="@_localizer["SelectCustomer"]"
           @bind-Value="_selectedCustomer"
           autocomplete="organization">
    @foreach (var customer in _customers)
    {
        <MudSelectItem T="string" Value="@customer.Name">@customer.Name</MudSelectItem>
    }
</MudSelect>
```

**WCAG**: 4.1.2 Name, Role, Value (Level A) — custom widgets must expose proper roles and states.

### 9. Missing Skip Navigation Link

The first focusable element on every page should be a skip link that jumps to the main content area, bypassing repeated navigation.

```razor
@* ❌ WRONG: page starts immediately with navigation bar *@
<MudAppBar>...</MudAppBar>
<MudDrawer>...</MudDrawer>
<main>...</main>

@* ✅ RIGHT: skip link as first focusable element *@
<a href="#main-content" class="skip-link">Skip to main content</a>
<MudAppBar>...</MudAppBar>
<MudDrawer>...</MudDrawer>
<main id="main-content" tabindex="-1">...</main>

@* CSS for skip link (add to site stylesheet): *@
@* .skip-link { position: absolute; top: -100%; left: 0; z-index: 1000; }
   .skip-link:focus { top: 0; padding: 0.5rem 1rem; background: #000; color: #fff; } *@
```

**WCAG**: 2.4.1 Bypass Blocks (Level A) — a mechanism must exist to bypass blocks of repeated content.

### 10. Using Placeholder as Label Replacement

Placeholder text disappears when the user starts typing and is not a substitute for a persistent, visible label.

```razor
@* ❌ WRONG: no visible label, only placeholder *@
<MudTextField T="string"
              Placeholder="Enter customer name"
              @bind-Value="_customerName" />

@* ✅ RIGHT: persistent visible label via MudBlazor Label parameter *@
<MudTextField T="string"
              Label="@_localizer["CustomerName"]"
              HelperText="@_localizer["CustomerName_Hint"]"
              @bind-Value="_customerName"
              autocomplete="organization" />
```

**WCAG**: 1.3.1 Info and Relationships (Level A), 3.3.2 Labels or Instructions (Level A).

### 11. Auto-Playing Media or Animations Without `prefers-reduced-motion`

Animations and transitions must respect the user's OS-level reduced motion preference. Wrap non-essential animations in the appropriate media query.

```css
/* ❌ WRONG: animation always plays */
.product-card {
    transition: transform 0.3s ease;
}
.product-card:hover {
    transform: scale(1.05);
}

/* ✅ RIGHT: animation only when user has no motion preference */
@media (prefers-reduced-motion: no-preference) {
    .product-card {
        transition: transform 0.3s ease;
    }
    .product-card:hover {
        transform: scale(1.05);
    }
}

/* ✅ RIGHT: alternative for users who prefer reduced motion */
@media (prefers-reduced-motion: reduce) {
    .product-card:hover {
        outline: 2px solid var(--tp-primary);
        outline-offset: 2px;
    }
}
```

**WCAG**: 2.3.3 Animation from Interactions (Level AAA), 2.2.2 Pause, Stop, Hide (Level A).

### Quick Anti-Pattern Reference

| # | Anti-Pattern | WCAG Criteria | Fix |
|---|-------------|---------------|-----|
| 1 | `aria-label` overriding visible text | 2.5.3 | Remove redundant `aria-label`; let visible text be the name |
| 2 | Color-only status indicators | 1.4.1 | Add icons, text labels, or patterns alongside color |
| 3 | Missing/incorrect `alt` text | 1.1.1 | `alt=""` + `aria-hidden` for decorative; descriptive text for informative |
| 4 | `@onclick` on non-interactive elements | 2.1.1, 4.1.2 | Use `<button>` or `<a>` with native keyboard support |
| 5 | No focus management after async ops | 2.4.3, 4.1.3 | Use `@ref` + `FocusAsync()` after state changes |
| 6 | `MudProgressLinear` without ARIA values | 4.1.2 | Add `aria-valuenow`/`min`/`max` or `aria-label` |
| 7 | `MudDialog` without focus trap/return | 2.4.3, 2.1.2 | Use `Autofocus` + `FocusAsync()` on trigger element |
| 8 | Custom dropdowns without ARIA listbox | 4.1.2 | Use `MudSelect`/`MudAutocomplete` or full ARIA pattern |
| 9 | Missing skip navigation link | 2.4.1 | Add `<a href="#main-content" class="skip-link">` |
| 10 | Placeholder as only label | 1.3.1, 3.3.2 | Use `Label` parameter on `MudTextField` |
| 11 | Animations ignoring reduced motion | 2.3.3, 2.2.2 | Wrap in `@media (prefers-reduced-motion: no-preference)` |

## Related Skills

- **blazor-mudblazor** — Blazor + MudBlazor patterns including dark mode, JS interop, focus management, and accessibility quirks (see Section 4)
- **signalr-realtime** — Real-time UI updates that need accessible announcements via aria-live regions
- **lgpd-data-privacy** — LGPD compliance patterns for consent management, data subject rights, and privacy by design

