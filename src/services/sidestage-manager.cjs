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
    wc.on('did-start-loading', () => { this.status.set(appId, 'loading'); this.onChange(); });
    wc.on('did-stop-loading', () => { this.status.set(appId, 'ready'); this.onChange(); });
    wc.on('did-fail-load', (_event, code, _description, _url, isMainFrame) => {
      if (!isMainFrame || code === -3) return;
      this.status.set(appId, 'error');
      this.onChange();
    });
    wc.on('destroyed', () => {
      this.views.delete(appId);
      this.status.set(appId, 'idle');
      this.onChange();
    });

    wc.loadURL(definition.url).catch(() => {
      this.status.set(appId, 'error');
      this.onChange();
    });
    return view;
  }

  action(action, appId = '') {
    const settings = this.settings();
    if (!settings.enabled) return this.state();

    if (action === 'toggle' || action === 'select') {
      const target = appDefinition(appId) ? appId : settings.activeApp || settings.pinnedApps[0];
      if (!target) return this.state();
      const alreadyVisible = settings.open && settings.activeApp === target;
      const open = action === 'toggle' ? !alreadyVisible : true;
      this.setSettings({ activeApp: target, open, collapsed: open ? false : settings.collapsed });
      if (open) this.ensureView(target);
    } else if (action === 'close') {
      this.setSettings({ open: false });
    } else if (action === 'collapse') {
      this.setSettings({ collapsed: !settings.collapsed });
    } else if (action === 'reload') {
      const target = appDefinition(appId) ? appId : settings.activeApp;
      const view = target ? this.ensureView(target) : null;
      view?.webContents.reload();
    }
    this.onChange();
    return this.state();
  }

  setWidth(width) {
    return this.setSettings({ width });
  }

  layout({ x, y, width, height, privateMode = false }) {
    const settings = this.settings();
    const active = settings.enabled && settings.open && !settings.collapsed && settings.activeApp && !privateMode ? settings.activeApp : '';
    for (const [id, view] of this.views) {
      if (view.webContents.isDestroyed()) continue;
      const visible = id === active;
      view.setVisible(visible);
      if (visible) view.setBounds({ x, y, width: Math.max(1, width), height: Math.max(1, height) });
    }
  }

  hideAll() {
    for (const view of this.views.values()) {
      if (!view.webContents.isDestroyed()) view.setVisible(false);
    }
  }

  destroyAll() {
    const win = this.getWindow();
    for (const view of this.views.values()) {
      try { win?.contentView?.removeChildView(view); } catch {}
      try { view.webContents.close(); } catch {}
    }
    this.views.clear();
    this.status.clear();
  }
}

module.exports = { SideStageManager };
