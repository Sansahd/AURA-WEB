const fs = require('node:fs');
const path = require('node:path');
const { DEFAULT_APPEARANCE, normalizeAppearance } = require('./persona.cjs');
const { DEFAULT_SIDESTAGE, normalizeSideStage } = require('./sidestage.cjs');

class QuanticStore {
  constructor(app) {
    this.file = path.join(app.getPath('userData'), 'quantic-state.json');
    this.data = {
      history: [],
      favorites: [],
      recentlyClosed: [],
      projects: [],
      settings: {
        searchEngine: 'quantic',
        immersiveMode: true,
        clearCacheOnExit: false,
        networkMode: 'balanced',
        compatibilityPolicyVersion: 2,
        appearance: { ...DEFAULT_APPEARANCE },
        sideStage: { ...DEFAULT_SIDESTAGE, pinnedApps: [...DEFAULT_SIDESTAGE.pinnedApps] }
      }
    };
    this.favoriteUrls = new Set();
    this.saveTimer = null;
    this.saveChain = Promise.resolve();
    this.load();
  }

  load() {
    try {
      const existing = JSON.parse(fs.readFileSync(this.file, 'utf8'));
      const existingSettings = existing.settings || {};
      this.data = {
        ...this.data,
        ...existing,
        settings: {
          ...this.data.settings,
          ...existingSettings,
          appearance: normalizeAppearance(existingSettings.appearance, DEFAULT_APPEARANCE),
          sideStage: normalizeSideStage(existingSettings.sideStage, DEFAULT_SIDESTAGE)
        },
        history: Array.isArray(existing.history) ? existing.history : [],
        favorites: Array.isArray(existing.favorites) ? existing.favorites : [],
        recentlyClosed: Array.isArray(existing.recentlyClosed) ? existing.recentlyClosed : [],
        projects: Array.isArray(existing.projects) ? existing.projects : []
      };
      // V1.1 used to clear Chromium's cache on every exit and forced the private
      // route for every page. Those defaults made media-heavy sites cold-start on
      // every launch and caused anti-abuse challenges. Migrate that implicit old
      // policy once; users can still select the private route explicitly.
      if (Number(existing.settings?.compatibilityPolicyVersion || 0) < 2) {
        this.data.settings.clearCacheOnExit = false;
        this.data.settings.networkMode = 'balanced';
        this.data.settings.compatibilityPolicyVersion = 2;
      }
    } catch {}
    this.reindex();
  }

  reindex() {
    this.favoriteUrls = new Set(this.data.favorites.map((x) => x.url).filter(Boolean));
  }

  scheduleSave(delay = 300) {
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      this.flush().catch(() => {});
    }, delay);
    this.saveTimer.unref?.();
  }

  save() {
    this.scheduleSave();
  }

  async flush() {
    clearTimeout(this.saveTimer);
    this.saveTimer = null;
    const snapshot = JSON.stringify(this.data);
    const dir = path.dirname(this.file);
    this.saveChain = this.saveChain.then(async () => {
      try {
        await fs.promises.mkdir(dir, { recursive: true });
        await fs.promises.writeFile(this.file, snapshot, 'utf8');
      } catch {}
    });
    return this.saveChain;
  }

  settings() {
    return {
      ...this.data.settings,
      appearance: normalizeAppearance(this.data.settings.appearance, DEFAULT_APPEARANCE),
      sideStage: normalizeSideStage(this.data.settings.sideStage, DEFAULT_SIDESTAGE)
    };
  }

  setSetting(key, value) {
    if (key === 'appearance') value = normalizeAppearance(value, this.data.settings.appearance);
    if (key === 'sideStage') value = normalizeSideStage(value, this.data.settings.sideStage);
    const current = this.data.settings[key];
    if (JSON.stringify(current) === JSON.stringify(value)) return this.settings();
    this.data.settings[key] = value;
    this.scheduleSave(120);
    return this.settings();
  }

  addHistory({ title, url }) {
    if (!/^https?:\/\//i.test(url || '')) return;
    const cleanTitle = title || url;
    const first = this.data.history[0];
    if (first?.url === url && first?.title === cleanTitle) return;
    this.data.history = [
      { title: cleanTitle, url, at: Date.now() },
      ...this.data.history.filter((x) => x.url !== url)
    ].slice(0, 500);
    this.scheduleSave(500);
  }

  history(limit = 100) { return this.data.history.slice(0, limit); }
  favorites(limit = 500) { return this.data.favorites.slice(0, limit); }
  isFavorite(url) { return this.favoriteUrls.has(url); }

  toggleFavorite({ title, url }) {
    if (!/^https?:\/\//i.test(url || '')) return false;
    const i = this.data.favorites.findIndex((x) => x.url === url);
    if (i >= 0) {
      this.data.favorites.splice(i, 1);
      this.favoriteUrls.delete(url);
    } else {
      this.data.favorites.unshift({ title: title || url, url, at: Date.now() });
      this.favoriteUrls.add(url);
    }
    this.scheduleSave(80);
    return i < 0;
  }

  removeFavorite(url) {
    const before = this.data.favorites.length;
    this.data.favorites = this.data.favorites.filter((x) => x.url !== url);
    if (this.data.favorites.length === before) return;
    this.favoriteUrls.delete(url);
    this.scheduleSave(80);
  }

  renameFavorite(url, title) {
    const item = this.data.favorites.find((x) => x.url === url);
    if (!item) return false;
    const next = String(title || item.title || url).trim().slice(0, 240);
    if (item.title === next) return true;
    item.title = next;
    this.scheduleSave(80);
    return true;
  }

  rememberClosed(tab) {
    if (!tab?.url || tab.url.startsWith('quantic://newtab')) return;
    this.data.recentlyClosed = [
      { title: tab.title || tab.url, url: tab.url, at: Date.now() },
      ...this.data.recentlyClosed.filter((x) => x.url !== tab.url)
    ].slice(0, 30);
    this.scheduleSave(180);
  }

  popClosed() {
    const item = this.data.recentlyClosed.shift() || null;
    if (item) this.scheduleSave(120);
    return item;
  }
}

module.exports = { QuanticStore };
