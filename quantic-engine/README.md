# Quantic Engine 0.4

Quantic Engine is the experimental **non-Chromium** web engine core for Quantic Glide.

The production Glide browser is still kept intact while the independent engine is validated. Quantic Engine does not fall back to Chromium when a feature is missing.

## Q0.5 engine alpha

Q0.5 keeps the Q0.4e compatibility core and turns it into an engine-alpha browser slice: session/navigation ownership, resource planning and caching, broader DOM/CSS behavior, and native ECMAScript modules without Chromium.

### Web platform

- replaceable `JsRuntime` interface with Boa ECMAScript as the current implementation;
- mutable DOM bindings and dynamic node creation;
- DOM events, bubbling and `preventDefault()`;
- `classList`, `dataset`, `matches()`, `closest()` and DOM traversal helpers;
- richer selectors shared by CSS and DOM queries:
  - descendant and direct-child combinators;
  - attribute selectors;
  - `:first-child`, `:last-child`, `:checked`, `:disabled`, `:root`, `:empty`;
- `URL`, live-bound `URLSearchParams`, `location` and same-origin `history`;
- form-urlencoded `URLSearchParams` serialization, tolerant percent decoding and code-unit ordering covered by upstream WPT;
- live `URLSearchParams` entries/keys/values/forEach during list mutations;
- existing parameter objects remain bound after `URL.search` or `URL.href` replacement;
- opaque data URLs retain their protocol/path when query parameters change;
- `TextEncoder` and UTF-8 `TextDecoder`;
- timers, animation-frame, microtask and idle-callback shims;
- `fetch()` routed through the host Privacy Gate;
- browser-facing `WebSocket` API bridged through the Rust host rather than opening sockets from page JavaScript.
- native Boa module execution for `<script type="module">`;
- recursive static relative ECMAScript imports (`./` and `../`) fetched through the same Privacy Gate and bounded to 16 levels / 128 modules;
- JavaScript `location.replace()` intent preserved through the host navigation request instead of being downgraded to a history push.

### Real font pipeline

Q0.4c removed the fixed-width bitmap assumption from normal text layout; Q0.4d makes that path cheaper to start and more resilient when a selected face lacks characters.

- system font discovery and CSS-like family matching through `fontdb`;
- inherited CSS `font-family`, `font-weight` and `font-style`;
- text shaping through Rustybuzz;
- glyph metrics and software rasterization through Fontdue;
- layout and paint share the same font system, so wrapping uses shaped advances rather than character-count estimates;
- proportional-font scorecard verifies that `iiii` and `WWWW` no longer receive the same width;
- the legacy `font8x8` path remains only as an explicit fallback when no usable real font is available;
- system-font directory discovery is cached once per process and reused by new Engine instances;
- when the selected face lacks an extended Unicode grapheme, Quantic selects one face covering the whole cluster and shapes contiguous fallback runs;
- combining/spacing marks, flags, emoji modifiers and ZWJ sequences stay with their bases;
- default-ignorable controls stay in the shaping buffer without requiring visible cmap glyphs;
- CSS family order is preserved between installed names and page-local aliases;
- when no complete face exists, the whole cluster remains in the primary face rather than being split;
- original minimal TTF fixtures verify exact advances, real rasterization, alias isolation and WOFF1/2 registration without relying on installed fonts;
- measurement and paint use the same fallback plan and align run baselines.

### Web fonts and privacy

`@font-face` is resolved by the Rust host rather than by page code.

- font URLs are resolved relative to the stylesheet that declared them;
- requests use `ResourceKind::Font` and pass through the same Privacy Gate as other subresources;
- third-party font requests are denied by default;
- accepted font bytes are registered under the declared CSS family alias;
- raw TTF/OTF continue directly into the font database;
- WOFF1 and WOFF2 are decoded to SFNT before registration;
- unsupported or malformed font binaries are reported and the loader can try the next `src` URL;
- the Q0.4c/Q0.4d scorecard executes a real third-party `@font-face` declaration and verifies rejection before transport.

Current limits are explicit: fallback preserves extended graphemes and checks coverage before shaping. Shaping-result-driven fallback, full bidi/complex-script layout, emoji ligature/color rendering, the complete CSS Fonts model and variable-font controls are not claimed yet.

### WebSocket isolation

WebSocket networking is owned by the host, not by Boa page code.

- `ws://` and `wss://` requests are evaluated as `ResourceKind::WebSocket` by the Privacy Gate before any worker is created;
- third-party sockets are denied by default;
- accepted sockets run in dedicated Rust worker threads so a slow socket does not block JavaScript evaluation or rendering;
- the host controls `Origin`, first-party cookies and requested subprotocol headers;
- text/binary send, receive, close and ping/pong handling are implemented;
- page JavaScript receives `open`, `message`, `error` and `close` events through an explicit host bridge;
- asynchronous network events are consumed through `InteractivePage::poll_async_events()`;
- the test suite includes a real local loopback handshake, inbound message, Quantic `send()` and server echo;
- the privacy suite verifies a third-party socket is rejected before network access.

### Privacy and storage

- no Chromium/Blink/V8 dependency or fallback;
- no telemetry endpoint, analytics endpoint or mandatory account;
- deny-by-default third-party subresources;
- redirect destinations re-checked before transport;
- origin-partitioned `localStorage`;
- page-local `sessionStorage`;
- first-party partitioned cookie jar;
- third-party cookies are never attached to requests;
- `HttpOnly` cookies are hidden from `document.cookie`;
- `Secure` cookies are valid for HTTPS/WSS transport;
- Domain, Path, Max-Age and the security requirement for `SameSite=None` are enforced by the current cookie subset;
- cookies travel through explicit Quantic host layers rather than an opaque browser cookie store.

### Rendering

- independent HTML/CSS pipeline;
- CSS cascade and inheritance;
- shaped proportional text metrics;
- real system-font software rasterization;
- block/inline layout;
- PNG/JPEG fetch, decode, layout and painting;
- padding and borders;
- software border rasterization;
- initial Flexbox:
  - row/column;
  - gap;
  - fixed widths and initial flex-grow allocation;
  - start/center/end/space-between justification;
  - start/center/end alignment;
- software rasterization to PNG.

### Navigation

- link navigation requests;
- form GET/POST navigation requests;
- `location.assign()`, `location.replace()` and reload intents;
- same-origin `history.pushState()` / `replaceState()`;
- cross-origin history mutation is rejected by the Rust host;
- the future native Glide shell can consume navigation requests without exposing transport privileges to page JavaScript.

## Expanded upstream WPT probe

`qwpt` executes selected JavaScript files taken directly from a pinned upstream Web Platform Tests commit.

Q0.4e gates these complete files:

- `url/urlsearchparams-has.any.js`;
- `url/urlsearchparams-size.any.js`;
- `url/urlsearchparams-get.any.js`;
- `url/urlsearchparams-getall.any.js`;
- `url/urlsearchparams-set.any.js`;
- `url/urlsearchparams-stringifier.any.js`;
- `url/urlsearchparams-sort.any.js`;
- `url/urlsearchparams-append.any.js`;
- `url/urlsearchparams-delete.any.js`;
- `url/urlsearchparams-foreach.any.js`;
- `url/url-tojson.any.js`;
- `encoding/textencoder-utf16-surrogates.any.js`.

The current supported slice contains **71 upstream subtests at a required 100% pass rate** across URL and Encoding.

This remains deliberately named a **WPT probe**, not full browser-oriented `wptrunner` integration. The next standards step is to widen coverage into additional URL, encoding, DOM and CSS behavior while preserving explicit supported-scope gates.

## Objective validation

The Quantic Engine CI now gates every change on:

1. `cargo fmt --check`;
2. all unit and integration tests, including the real WebSocket loopback transport test and font-pipeline tests;
3. checkout of the pinned upstream WPT slice;
4. **71 upstream WPT subtests at 100%** for the selected supported slice;
5. the executable **Q0.5 compatibility scorecard** (`qcompat`) with **28 checks**, including ECMAScript relative module graphs, replace-navigation semantics, deterministic grapheme fallback, live URL parameters, real-font metrics, cached system-font scanning, and the `@font-face` privacy bridge;
6. Clippy with warnings denied;
7. optimized Release build;
8. a real interactive end-to-end PNG render smoke.

Run locally:

~~~bash
cd quantic-engine
cargo test --all-targets
cargo run --bin qcompat
cargo run --bin qwpt -- --min-pass-rate 100 path/to/wpt-test.any.js
cargo run --release --bin qengine -- page.html output.png
cargo run --release --bin qengine -- https://example.com output.png
~~~

For final distribution builds, the `dist` profile retains FatLTO. Normal Release validation uses ThinLTO.

## Not production-ready yet

Quantic Engine 0.4 is a real independent interactive browser core, but it is **not yet a safe replacement for the production Glide engine**.

The largest remaining compatibility and architecture gates are:

- shaping-result-driven fallback, full bidi/complex-script layout, emoji rendering and broader CSS Fonts behavior;
- much broader WPT coverage and eventual full browser-oriented WPT/wptrunner integration;
- broader CSSOM, layout and Flexbox behavior;
- stronger WebSocket connection/resource budgets;
- GPU compositor and process sandboxing;
- accessibility;
- media and WebAssembly gates.

Missing features remain explicit instead of being silently delegated to Chromium.
