'use strict';

const path = require('node:path');
const fs = require('node:fs');

const CACHE_MAX_AGE_MS = 72 * 60 * 60 * 1000;
const FETCH_TIMEOUT_MS = 8000;

const EASYLIST_URL = 'https://easylist.to/easylist/easylist.txt';
const EASYPRIVACY_URL = 'https://easylist.to/easylist/easyprivacy.txt';

const CUSTOM_NETWORK_RULES = Object.freeze([
  '@@||challenges.cloudflare.com^',
  '||doubleclick.net^$third-party',
  '||googlesyndication.com^$third-party',
  '||googleadservices.com^$third-party',
  '||youtube.com/pagead/',
  '||youtube.com/api/stats/ads',
  '||youtube.com/ptracking'
]);

const CUSTOM_COSMETIC_RULES = Object.freeze([
  'youtube.com##.ytp-ad-module',
  'youtube.com##.ytp-ad-player-overlay',
  'youtube.com##.ytp-ad-message-container',
  'youtube.com##.ytp-ad-preview-container',
  'youtube.com##ytd-ad-slot-renderer',
  'youtube.com##ytd-display-ad-renderer',
  'youtube.com##ytd-promoted-sparkles-web-renderer'
]);

function safeMessage(error) {
  return String(error?.message || error || '').slice(0, 240);
}

function cacheFresh(stat) {
  return Boolean(stat && Date.now() - Number(stat.mtimeMs || 0) < CACHE_MAX_AGE_MS);
}

function timeoutFetch(timeoutMs = FETCH_TIMEOUT_MS) {
  return async (url, options = {}) => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    timer.unref?.();
    try {
      return await globalThis.fetch(url, {
        ...options,
        signal: options.signal || controller.signal
      });
    } finally {
      clearTimeout(timer);
    }
  };
}

class GekkoShields {
  constructor(app, { onState = () => {} } = {}) {
    this.app = app;
    this.onState = onState;
    this.blocker = null;
    this.state = {
      status: 'idle',
      engine: 'ghostery-adblocker',
      source: 'fallback',
      cache: false,
      cacheAgeMs: 0,
      customNetworkRules: CUSTOM_NETWORK_RULES.length,
      customCosmeticRules: CUSTOM_COSMETIC_RULES.length,
      error: ''
    };
  }

  snapshot() {
    return { ...this.state };
  }

  setState(patch) {
    this.state = { ...this.state, ...patch };
    try { this.onState(this.snapshot()); } catch {}
  }

  async loadModules() {
    const electronMod = await import('@ghostery/adblocker-electron');
    const coreMod = await import('@ghostery/adblocker');
    return {
      ElectronBlocker: electronMod.ElectronBlocker || electronMod.default?.ElectronBlocker,
      NetworkFilter: coreMod.NetworkFilter,
      CosmeticFilter: coreMod.CosmeticFilter
    };
  }

  async cachedBlocker(ElectronBlocker, cachePath) {
    try {
      const stat = await fs.promises.stat(cachePath);
      if (!cacheFresh(stat)) return null;
      const bytes = await fs.promises.readFile(cachePath);
      const blocker = ElectronBlocker.deserialize(bytes);
      this.setState({
        source: 'cache',
        cache: true,
        cacheAgeMs: Math.max(0, Date.now() - Number(stat.mtimeMs || Date.now()))
      });
      return blocker;
    } catch {
      return null;
    }
  }

  async fetchBlocker(ElectronBlocker, cachePath) {
    const fetcher = timeoutFetch();
    let blocker = null;
    let source = 'prebuilt';

    try {
      blocker = await ElectronBlocker.fromPrebuiltAdsAndTracking(fetcher);
    } catch (prebuiltError) {
      source = 'easylist+easyprivacy';
      blocker = await ElectronBlocker.fromLists(fetcher, [EASYLIST_URL, EASYPRIVACY_URL]).catch((listError) => {
        const error = new Error(`prebuilt: ${safeMessage(prebuiltError)}; lists: ${safeMessage(listError)}`);
        throw error;
      });
    }

    try {
      await fs.promises.mkdir(path.dirname(cachePath), { recursive: true });
      await fs.promises.writeFile(cachePath, Buffer.from(blocker.serialize()));
      this.setState({ source, cache: true, cacheAgeMs: 0 });
    } catch {
      this.setState({ source, cache: false, cacheAgeMs: 0 });
    }

    return blocker;
  }

  applyGekkoRules(blocker, NetworkFilter, CosmeticFilter) {
    const newNetworkFilters = CUSTOM_NETWORK_RULES
      .map((rule) => {
        try { return NetworkFilter.parse(rule); } catch { return null; }
      })
      .filter(Boolean);
    const newCosmeticFilters = CUSTOM_COSMETIC_RULES
      .map((rule) => {
        try { return CosmeticFilter.parse(rule); } catch { return null; }
      })
      .filter(Boolean);

    blocker.update({ newNetworkFilters, newCosmeticFilters });
  }

  async install(targetSession) {
    if (!targetSession) {
      this.setState({ status: 'degraded', error: 'session-unavailable' });
      return false;
    }
    if (this.state.status === 'ready') return true;
    if (this.state.status === 'loading') return false;

    this.setState({ status: 'loading', error: '' });

    try {
      const { ElectronBlocker, NetworkFilter, CosmeticFilter } = await this.loadModules();
      if (!ElectronBlocker || !NetworkFilter || !CosmeticFilter) throw new Error('adblocker-modules-unavailable');

      const cachePath = path.join(this.app.getPath('userData'), 'Privacy', 'gekko-shields.bin');
      let blocker = await this.cachedBlocker(ElectronBlocker, cachePath);

      if (!blocker) {
        try {
          blocker = await this.fetchBlocker(ElectronBlocker, cachePath);
        } catch (freshError) {
          try {
            const bytes = await fs.promises.readFile(cachePath);
            blocker = ElectronBlocker.deserialize(bytes);
            const stat = await fs.promises.stat(cachePath).catch(() => null);
            this.setState({
              source: 'stale-cache',
              cache: true,
              cacheAgeMs: stat ? Math.max(0, Date.now() - Number(stat.mtimeMs || Date.now())) : 0,
              error: safeMessage(freshError)
            });
          } catch {
            throw freshError;
          }
        }
      }

      this.applyGekkoRules(blocker, NetworkFilter, CosmeticFilter);
      blocker.enableBlockingInSession(targetSession);
      this.blocker = blocker;
      this.setState({ status: 'ready', error: '' });
      return true;
    } catch (error) {
      this.blocker = null;
      this.setState({ status: 'degraded', source: 'fallback', cache: false, error: safeMessage(error) });
      return false;
    }
  }
}

module.exports = {
  GekkoShields,
  CUSTOM_NETWORK_RULES,
  CUSTOM_COSMETIC_RULES,
  EASYLIST_URL,
  EASYPRIVACY_URL,
  CACHE_MAX_AGE_MS
};
