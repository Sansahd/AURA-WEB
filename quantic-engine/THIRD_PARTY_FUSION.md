# Quantic Fusion — upstream provenance

This branch intentionally combines independent non-Chromium technologies behind the Quantic Engine API.

## Servo

Pinned target: **Servo v0.7.0**.

Role:
- primary standards runtime;
- DOM/script/layout integration;
- rendering;
- embedder WebView API.

License: MPL-2.0.

Servo itself uses components with Mozilla/Firefox lineage including Stylo, SpiderMonkey/MozJS and WebRender. Quantic treats those as internal implementation components, not as branding.

## Ladybird / LibWeb

Pinned source snapshot:
`LadybirdBrowser/ladybird@a4140db626af72d0a676075a1ba26be728f4ea39`.

Runtime component currently ported:
- LibWeb Rust HTML tokenizer state machine.

Quantic-specific standalone adaptations:
- Ladybird's AK/C++ fly-string identity is replaced by an owned Rust `String`;
- the named-character-reference lookup keeps Ladybird's tokenizer API but uses a standalone table generated from Ladybird's `Entities.json`.

The tokenizer algorithm/source remains attributed to the Ladybird developers.

License: BSD-2-Clause.

## Mozilla / Gecko ecosystem

Pinned Neqo snapshot:
`mozilla/neqo@28b49f73b06f71aa602180e4dcc3a2b522103b51`.

Role:
- QUIC transport;
- HTTP/3;
- future WebTransport networking path.

Servo v0.7 also supplies the runtime path to Stylo, SpiderMonkey/MozJS and WebRender. These are technologies from the Servo/Mozilla/Gecko ecosystem.

License: MPL-2.0 and the licenses declared by the upstream crates.

## Explicitly excluded

Quantic Fusion must not introduce:
- Chromium;
- Electron;
- CEF;
- WebView2;
- a Chromium fallback.

The CI dependency contract fails when those names appear in the runtime dependency tree.
