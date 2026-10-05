# AURA Career on Quantic Engine Q0.5

This branch integrates AURA Career directly into the non-Chromium Quantic Engine.

## Architecture

```
AURA Career
   |
   v
AutomationBridge (Rust)
   |
   v
BrowserSession
   |
   v
Quantic Engine Q0.5
```

No Playwright and no Chromium fallback are used by `qcareer`.

## Run

1. Start Ollama and install a local chat model:
   `ollama pull qwen3:4b`
2. Copy `career-agent/profile.example.json` to a private local profile file.
3. Build from `quantic-engine`:
   `cargo build --release --bin qcareer`
4. Run:
   `cargo run --release --bin qcareer -- ../career-agent/profile.json ../career-agent/searches.json ../career-agent/state.json`

Environment variables:

- `OLLAMA_URL=http://127.0.0.1:11434`
- `OLLAMA_MODEL=qwen3:4b`
- `MIN_SCORE=72`
- `MAX_APPLICATIONS_PER_RUN=5`
- `AUTO_SUBMIT=false`
- `CV_PATH=C:\\path\\to\\your-cv.pdf`

## Native CV upload

Q0.5 now supports binary request bodies and native `multipart/form-data` submission for normal HTML forms. AURA Career detects a named file input and can attach the PDF from `CV_PATH` without Chromium or Playwright.

The agent still stops rather than guessing when a site requires an unsupported custom upload widget, login step, CAPTCHA/2FA, or sensitive/unknown answer.

There is no Chromium fallback.
