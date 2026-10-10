'use strict';

const { WebContentsView, shell } = require('electron');
const { appDefinition, publicAppList, normalizeSideStage } = require('./sidestage.cjs');

class SideStageManager {
  constructor({ store, session, getWindow, openInMain, onChange }) {
    this.store = store;
    this.session = session;
    this.getWindow = getWindow;
    this.openInMain = openInMain;
    this.onChange = onChange || (() => {});
    this.views = new Map();
    this.status = new Map();
    this.failures = new Set();
    this.loadTimers = new Map();
  }

  settings() {
    return normalizeSideStage(this.store.settings().sideStage);
  }

  state({ privateMode = false } = {}) {
    const settings = this.settings();
    return {
      ...settings,
      open: Boolean(settings.enabled && settings.open && !privateMode),
      privateDisabled: Boolean(privateMode),
      apps: publicAppList(settings).map((app) => ({
        ...app,
        loaded: this.views.has(app.id),
        status: this.status.get(app.id) || 'idle'
      }))
    };
  }

  setSettings(patch) {
    const next = normalizeSideStage({ ...this.settings(), ...(patch || {}) }, this.settings());
    this.store.setSetting('sideStage', next);
    this.onChange();
    return next;
  }

  clearLoadTimer(id) {
    clearTimeout(this.loadTimers.get(id));
    this.loadTimers.delete(id);
  }

  ensureView(appId) {
    const definition = appDefinition(appId);
    const win = this.getWindow();
    if (!definition || !win || win.isDestroyed()) return null;
    const existing = this.views.get(appId);
    if (existing && !existing.webContents.isDestroyed()) return existing;

    const view = new WebContentsView({
      webPreferences: {
        partition: 'persist:quantic',
        nodeIntegration: false,
        contextIsolation: true,
        sandbox: true,
        webSecurity: true,
        allowRunningInsecureContent: false,
        javascript: true,
        backgroundThrottling: false,
        spellcheck: false,
        navigateOnDragDrop: false
      }
    });
    try { view.setBackgroundColor('#0b1020'); } catch {}
    win.contentView.addChildView(view);
    view.setVisible(false);
    this.views.set(appId, view);
    this.status.set(appId, 'loading');
    this.failures.delete(appId);

    const wc = view.webContents;
    wc.setWindowOpenHandler(({ url }) => {
      if (/^https?:\/\//i.test(url)) this.openInMain(url);
      else if (/^(mailto|tel):/i.test(url)) shell.openExternal(url).catch(() => {});
      return { action: 'deny' };
    });
    wc.on('will-navigate', (event, url) => {
      if (/^https?:\/\//i.test(url)) return;
      event.preventDefault();
    });
    const showFailure = (reason) => {
      if (wc.isDestroyed() || this.failures.has(appId)) return;
      this.clearLoadTimer(appId);
      this.failures.add(appId);
      this.status.set(appId, 'error');
      this.onChange();
      const label = definition.label.replace(/[&<>"]/g, (s) =>
        ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[s]);
      const cause = reason === 'timeout' ? 'Délai de connexion dépassé après 45 secondes'
        : 'Erreur réseau ' + (Number(reason) || 'inconnue');
      const page = `<!doctype html><html lang="fr"><meta charset="utf-8">
      <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
      <body style="margin:0;min-height:100vh;display:grid;place-items:center;background:#09121c;color:#effff4;font-family:system-ui,sans-serif">
      <main style="max-width:340px;padding:22px;text-align:center">
      <h2 style="font-size:17px;margin:0 0 12px">${label} indisponible</h2>
      <p style="color:#b5c8bd;line-height:1.6;font-size:13px">${cause}. Le service externe ne s'est pas affiché.</p>
      <p style="color:#b5c8bd;font-size:12px">Clique de nouveau sur SOCIAL ou Mail pour réessayer, ou utilise l’onglet normal.</p>
      <a href="${definition.url}" target="_blank" rel="noopener" style="display:inline-block;padding:10px 14px;border:1px solid #9cddb6;border-radius:12px;color:#d8ffe9;text-decoration:none;font-size:12px">Ouvrir dans un onglet</a>
      </main></body></html>`;
      wc.loadURL('data:text/html;charset=utf-8,' + encodeURIComponent(page)).catch(() => {});
    };
    const armTimeout = () => {
      this.clearLoadTimer(appId);
      const timer = setTimeout(() => {
        if (this.status.get(appId) === 'loading') showFailure('timeout');
      }, 45000);
      timer.unref?.();
      this.loadTimers.set(appId, timer);
    };
    wc.on('did-start-loading', () => {
      // Loading the local error page must not turn an error into false READY.
      if (this.failures.has(appId)) return;
      this.status.set(appId, 'loading');
      armTimeout();
      this.onChange();
    });
    wc.on('did-stop-loading', () => {
      if (this.failures.has(appId)) return;
      this.clearLoadTimer(appId);
      this.status.set(appId, 'ready');
      this.onChange();
    });
    wc.on('did-fail-load', (_event, code, _description, _url, isMainFrame) => {
      if (!isMainFrame || code === -3) return;
      showFailure(code);
    });
    wc.on('destroyed', () => {
      this.clearLoadTimer(appId);
      this.failures.delete(appId);
      this.views.delete(appId);
      this.status.set(appId, 'idle');
      this.onChange();
    });

    armTimeout();
    wc.loadURL(definition.url).catch(() => showFailure(-2));
    return view;
  }

  action(action, appId = '') {
    const settings = this.settings();
    if (!settings.enabled) return this.state();

    if (action === 'toggle' || action === 'select') {
      const target = appDefinition(appId) ? appId : settings.activeApp || settings.pinnedApps[0];
      if (!target) return this.state();
      const alreadyVisible = settings.open && settings.activeApp === target;
      const failed = this.status.get(target) === 'error';
      const open = action === 'toggle' ? (!alreadyVisible || failed) : true;
      this.setSettings({ activeApp: target, open, collapsed: open ? false : settings.collapsed });
      if (open) {
        const view = this.ensureView(target);
        if (failed && view) {
          this.failures.delete(target);
          this.status.set(target, 'loading');
          view.webContents.loadURL(appDefinition(target).url).catch(() => {
            this.status.set(target, 'error');
            this.failures.add(target);
            this.onChange();
          });
        }
      }
    } else if (action === 'close') {
      this.setSettings({ open: false });
    } else if (action === 'collapse') {
      this.setSettings({ collapsed: !settings.collapsed });
    } else if (action === 'reload') {
      const target = appDefinition(appId) ? appId : settings.activeApp;
      const view = target ? this.ensureView(target) : null;
      if (target) this.failures.delete(target);
      view?.webContents.loadURL(appDefinition(target).url).catch(() => {
        this.status.set(target, 'error');
        this.onChange();
      });
    }
    this.onChange();
    return this.state();
  }

  setWidth(width) {
    return this.setSettings({ width });
  }

  layout({ x, y, width, height, privateMode = false }) {
    const settings = this.settings();
    const active = settings.enabled && settings.open && !settings.collapsed &&
      settings.activeApp && !privateMode && width > 0 ? settings.activeApp : '';
    for (const [id, view] of this.views) {
      if (view.webContents.isDestroyed()) continue;
      const visible = id === active;
      view.setVisible(visible);
      if (visible) {
        view.setBounds({ x, y, width: Math.max(1, width), height: Math.max(1, height) });
        const win = this.getWindow();
        if (win && !win.isDestroyed()) win.contentView.addChildView(view);
      }
    }
  }

  hideAll() {
    for (const view of this.views.values()) {
      if (!view.webContents.isDestroyed()) view.setVisible(false);
    }
  }

  destroyAll() {
    const win = this.getWindow();
    for (const timeout of this.loadTimers.values()) clearTimeout(timeout);
    this.loadTimers.clear();
    this.failures.clear();
    for (const view of this.views.values()) {
      try { win?.contentView?.removeChildView(view); } catch {}
      try { view.webContents.close(); } catch {}
    }
    this.views.clear();
    this.status.clear();
  }
}

module.exports = { SideStageManager };
