# GEKKO Native 1.0 — Product Readiness Gate

GEKKO Native 1.0 is releasable only when every mandatory gate below is green. The Electron 2.3 branch remains a rollback path during the native transition; it is not part of the native runtime.

## Ladybird runtime integration — target 8.5/10

- Ladybird's tokenizer is compiled from the pinned LibWeb-derived Rust port.
- Every real main-frame `text/html` response loaded by Servo is also tokenized by Ladybird in the network pipeline.
- Ladybird token and invalid-token counters are surfaced in the Gekko diagnostics panel.
- Servo remains the DOM/layout/rendering authority; Ladybird is a live shadow parser and conformance signal rather than a fake replacement parser.
- CI runs the Ladybird contract plus the Quantic Engine integration tests with `fusion-ladybird-html`.

## Gekko product identity — target 9/10

- Product title, runtime labels, installer name, executable name and data directory use Gekko.
- Existing Glide browser data is migrated on first launch.
- The runtime draws a native Earth + attached gekko mark rather than the former abstract circles.
- Windows package is `Quantic-Gekko-Setup.exe`, version 1.0.0.

## Cinematic navigation — target 8.5/10

- Main-frame loads start a native 460 ms eased transition.
- Web content enters with a subtle horizontal cinematic motion.
- A fading dark veil and warm moving highlight bridge page changes.
- Animation is rendered by Gekko itself; it is not injected into websites.

## Web compatibility/testing — target 8.5/10

Release gate must execute:

- Gekko runtime `cargo test`;
- Quantic Engine library and integration tests;
- Q02, Q03, Q04, Q04e and Q05 contracts;
- WPT subset tests;
- Ladybird HTML contracts;
- qcompat and qwpt compilation;
- Servo/Stylo/SpiderMonkey/WebRender dependency contract;
- zero Chromium/Electron/CEF/WebView2 contract;
- Windows release build;
- macOS compile + tests.

## Product-level gate

A release is not called 8.5/10 globally if the current HEAD fails any mandatory quality job. Scores are evidence-based, not branding.

The GitHub quality gate enforces these runtime hooks and rejects legacy Glide branding before release.


## Native 1.0 promotion gate

- Windows desktop runtime is built directly from `fusion/servo-runtime`.
- Runtime dependency tree must reject Chromium, Electron, CEF and WebView2.
- A real Windows smoke run must render the fixture in Servo, execute JavaScript, expose the expected DOM marker and exit by its own smoke hook.
- The Windows installer and portable archive must contain only the native GEKKO executable plus installer metadata.
- Android ARM64 build and x86_64 runtime E2E must remain green.
- Neqo stays wired into the live resource path with Servo fallback.
- `main` remains on GEKKO 2.3 until this gate is green; Native 1.0 is promoted only after validation.
