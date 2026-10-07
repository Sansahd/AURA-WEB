'use strict';

const fs = require('node:fs');
const path = require('node:path');

class GekkoExtensionManager {
  constructor({ session, store }) {
    this.session = session;
    this.store = store;
    this.loaded = new Map();
    this.errors = new Map();
  }

  configuredPaths() {
    const settings = this.store?.settings?.() || {};
    return Array.isArray(settings.extensionPaths)
      ? settings.extensionPaths.filter((item) => typeof item === 'string' && item.trim())
      : [];
  }

  persistPaths(paths) {
    const clean = [...new Set((paths || []).map((item) => path.resolve(String(item))).filter(Boolean))];
    this.store?.setSetting?.('extensionPaths', clean);
    return clean;
  }

  api() {
    return this.session?.extensions || null;
  }

  async loadPath(extensionPath, { persist = true } = {}) {
    const absolute = path.resolve(String(extensionPath || ''));
    if (!absolute || !fs.existsSync(absolute) || !fs.statSync(absolute).isDirectory()) {
      throw new Error('Dossier d’extension introuvable');
    }
    const manifest = path.join(absolute, 'manifest.json');
    if (!fs.existsSync(manifest)) throw new Error('manifest.json manquant');

    const api = this.api();
    if (!api?.loadExtension) throw new Error('API extensions Electron indisponible');

    const extension = await api.loadExtension(absolute, { allowFileAccess: false });
    this.loaded.set(extension.id, { id: extension.id, name: extension.name || extension.id, path: absolute, version: extension.version || '' });
    this.errors.delete(absolute);

    if (persist) {
      this.persistPaths([...this.configuredPaths(), absolute]);
    }
    return this.snapshot();
  }

  async restore() {
    for (const extensionPath of this.configuredPaths()) {
      try {
        await this.loadPath(extensionPath, { persist: false });
      } catch (error) {
        this.errors.set(extensionPath, error?.message || String(error));
      }
    }
    return this.snapshot();
  }

  async remove(extensionId) {
    const id = String(extensionId || '');
    const current = this.loaded.get(id);
    const api = this.api();
    if (api?.removeExtension && id) {
      try { api.removeExtension(id); } catch {}
    }
    this.loaded.delete(id);

    if (current?.path) {
      this.persistPaths(this.configuredPaths().filter((item) => path.resolve(item) !== path.resolve(current.path)));
      this.errors.delete(current.path);
    }
    return this.snapshot();
  }

  snapshot() {
    const api = this.api();
    const live = typeof api?.getAllExtensions === 'function' ? api.getAllExtensions() : [];
    const byId = new Map();
    for (const extension of live || []) {
      const known = this.loaded.get(extension.id);
      byId.set(extension.id, {
        id: extension.id,
        name: extension.name || known?.name || extension.id,
        version: extension.version || known?.version || '',
        path: known?.path || '',
        loaded: true
      });
    }
    for (const [id, known] of this.loaded) {
      if (!byId.has(id)) byId.set(id, { ...known, loaded: false });
    }

    return {
      supported: Boolean(api?.loadExtension),
      compatibility: 'electron-subset',
      items: [...byId.values()],
      errors: [...this.errors.entries()].map(([extensionPath, error]) => ({ path: extensionPath, error }))
    };
  }
}

module.exports = { GekkoExtensionManager };
