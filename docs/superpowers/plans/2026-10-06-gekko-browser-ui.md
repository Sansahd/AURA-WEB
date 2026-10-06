# GEKKO Browser UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the approved GEKKO visual identity, bottom omnibox and cinematic navigation transition without changing navigation semantics.

**Architecture:** Keep Electron/main-process navigation intact. Split visible chrome into a top tab strip and bottom navigation dock; reserve matching WebContentsView bounds in main.cjs. Drive a renderer transition overlay from per-tab navigation state set on main-frame navigation lifecycle events.

**Tech Stack:** Electron 44, CommonJS, DOM/CSS, SVG, Node built-in test runner.

**Spec:** docs/gekko-ui-spec.md

## Global Constraints
- No artificial navigation delay.
- Main omnibox is at the bottom.
- Gecko must visibly grip a recognizable Earth in every primary icon.
- `prefers-reduced-motion` disables decorative motion.
- Existing privacy/navigation behavior must remain intact.

## Review Focus
- External WebContentsView never overlaps the bottom dock.
- Keyboard focus (Ctrl/Cmd+L path) still selects the omnibox.
- Same-document/hash navigation does not produce a disruptive transition.
- Failed page loads exit the transition cleanly.
- Immersive chrome reveal works at top and bottom edges.

---

### Task 1: Lock the GEKKO UI contract
**Files:**
- Create: `test/gekko-ui.test.cjs`

**Interfaces:**
- Consumes: repository source files.
- Produces: static regression checks for branding, layout and transition hooks.

- [ ] Write tests for GEKKO branding, bottom dock, transition overlay, WebContentsView top/bottom reservation, and reduced-motion support.
- [ ] Run tests and verify they fail on the current Glide UI.
- [ ] Commit tests.

### Task 2: Implement real GEKKO shell and assets
**Files:**
- Modify: `src/renderer/index.html`
- Modify: `src/renderer/styles.css`
- Modify: `src/renderer/quantic-glide-brand.css`
- Modify: `src/assets/quantic-glide-icon.svg`
- Modify: `src/assets/quantic-glide-mark.svg`
- Modify: `src/assets/quantic-glide-logo.svg`

**Interfaces:**
- Consumes: existing renderer IDs used by renderer.js.
- Produces: top tab strip, bottom dock, GEKKO brand surface and transition overlay.

- [ ] Implement the approved glossy GEKKO shell while retaining existing DOM IDs.
- [ ] Replace Glide SVG artwork with gecko-gripping-Earth SVG assets.
- [ ] Run contract tests.

### Task 3: Wire cinematic navigation and layout
**Files:**
- Modify: `src/main.cjs`
- Modify: `src/renderer/renderer.js`

**Interfaces:**
- Consumes: existing tab state and WebContentsView lifecycle.
- Produces: `transitioning` state and class-driven renderer animation.

- [ ] Mark main-frame navigation as transitioning and hide the outgoing view.
- [ ] Clear transition when DOM is ready or loading fails.
- [ ] Reserve top strip and bottom dock in WebContentsView bounds.
- [ ] Reveal immersive chrome from either top or bottom edge.
- [ ] Run contract and syntax tests.

### Task 4: Product/CI consistency
**Files:**
- Modify: `package.json`
- Modify: `.github/workflows/build-windows.yml`

**Interfaces:**
- Consumes: GEKKO source tree.
- Produces: GEKKO-named installer and PR validation.

- [ ] Update visible package/build product naming to GEKKO without changing the persisted app id.
- [ ] Run UI tests in CI and build on pull requests.
- [ ] Run full checks.
