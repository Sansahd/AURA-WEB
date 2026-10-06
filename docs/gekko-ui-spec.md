# GEKKO browser UI spec

## Goal
Turn the current Quantic Glide shell into the real GEKKO visual identity approved in conversation while preserving the existing navigation engine.

## Required behavior
- Public product name: **GEKKO** with tagline **BROWSE FREELY**.
- The primary mark is a gecko visibly gripping planet Earth; the Earth remains recognizable at app-icon size.
- Tabs stay in a compact top strip.
- The primary omnibox/address/search field moves to a fixed glossy bottom dock.
- Back, forward, reload, favorite, privacy/assistant and menu controls remain accessible around the bottom dock.
- External web content reserves room for both the top tabs and bottom dock.
- Immersive mode can hide both chrome zones and reveal them from either screen edge.
- Page-to-page navigation uses a short cinematic transition: hide the outgoing web view when main-frame navigation starts, show a GEKKO transition layer, reveal the destination as soon as DOM is ready. No artificial network delay.
- Respect `prefers-reduced-motion`.
- Keep the existing browser security model and navigation resolution unchanged.

## Visual direction
Dark midnight glass, rounded acrylic surfaces, cyan/teal highlights, restrained glow, premium rather than game-like. The home page centers the GEKKO Earth mark and keeps functional product cards secondary to the browser itself.
