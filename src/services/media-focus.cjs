'use strict';

// Native WebContentsView compositing is not affected by CSS in the shell.
// Injecting a reversible style into the *actual main-tab contents* reliably
// darkens the surrounding page when the separately rendered reader is open.
const FOCUS_STYLE = 'html { filter: brightness(.36) saturate(.78) !important; }';

class NativeMediaFocus {
  constructor({ style = FOCUS_STYLE } = {}) {
    this.style = style;
    this.webContents = null;
    this.key = '';
    this.version = 0;
  }

  sync(candidate, enabled = false) {
    const desired = enabled && candidate && !candidate.isDestroyed() ? candidate : null;
    if (desired === this.webContents) return;
    const old = this.webContents;
    const oldKey = this.key;
    this.version += 1;
    const version = this.version;
    this.webContents = desired;
    this.key = '';
    if (old && oldKey && !old.isDestroyed()) old.removeInsertedCSS(oldKey).catch(() => {});
    if (!desired) return;
    desired.insertCSS(this.style, { cssOrigin: 'user' }).then(key => {
      if (version !== this.version || this.webContents !== desired || desired.isDestroyed()) {
        if (!desired.isDestroyed()) desired.removeInsertedCSS(key).catch(() => {});
        return;
      }
      this.key = key;
    }).catch(() => {
      // A failed insert is not a successfully active focus. Retry on the next
      // navigation or state update.
      if (version === this.version) this.webContents = null;
    });
  }

  refresh(wc) {
    if (!wc || this.webContents !== wc) return;
    const key = this.key;
    this.key = '';
    this.webContents = null;
    this.version += 1;
    if (key && !wc.isDestroyed()) wc.removeInsertedCSS(key).catch(() => {});
    this.sync(wc, true);
  }

  clear() { this.sync(null, false); }
}

module.exports = { NativeMediaFocus, FOCUS_STYLE };
