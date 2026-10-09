'use strict';

const INSTALLED = Symbol.for('quantic.ipcFirewallInstalled');
const SEARCH_ENGINES = new Set(['gekko', 'duckduckgo', 'qwant', 'startpage', 'brave', 'searxng', 'tor']);
const AI_ACTIONS = new Set(['summarize', 'compare', 'plan', 'chat']);
const WINDOW_ACTIONS = new Set(['minimize', 'maximize', 'close', 'sizes']);
const NETWORK_MODES = new Set(['balanced', 'private']);
const SIDE_ACTIONS = new Set(['toggle', 'select', 'close', 'reload', 'collapse']);
const SIDE_APPS = new Set(['youtube', 'twitch', 'spotify', 'netflix', 'proton']);
function isString(value, max = 8192) { return typeof value === 'string' && value.length <= max; }
function isPositiveId(value) { return Number.isSafeInteger(value) && value > 0; }
function isHttpUrl(value) { if (!isString(value, 8192)) return false; try { const url = new URL(value); return url.protocol === 'http:' || url.protocol === 'https:'; } catch { return false; } }
function isAppearancePatch(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const allowed = new Set(['enabled', 'accent', 'glassOpacity', 'radius']);
  for (const key of Object.keys(value)) if (!allowed.has(key)) return false;
  if ('enabled' in value && typeof value.enabled !== 'boolean') return false;
  if ('accent' in value && !/^#[0-9a-f]{6}$/i.test(String(value.accent))) return false;
  if ('glassOpacity' in value && (!Number.isFinite(value.glassOpacity) || value.glassOpacity < 0.25 || value.glassOpacity > 0.95)) return false;
  if ('radius' in value && (!Number.isFinite(value.radius) || value.radius < 6 || value.radius > 28)) return false;
  return true;
}
function isSideAction(value) { if (!isString(value, 64)) return false; const [action, app = ''] = value.split(':', 2); return SIDE_ACTIONS.has(action) && (app === '' || SIDE_APPS.has(app)); }
function isPlainObject(value) { return Boolean(value) && typeof value === 'object' && !Array.isArray(value); }
function isCareerSettingsPatch(value) {
  if (!isPlainObject(value)) return false;
  const allowed = new Set(['autopilot', 'autoSubmit', 'minScore', 'maxOffers', 'maxDaily']);
  for (const key of Object.keys(value)) if (!allowed.has(key)) return false;
  if ('autopilot' in value && typeof value.autopilot !== 'boolean') return false;
  if ('autoSubmit' in value && typeof value.autoSubmit !== 'boolean') return false;
  if ('minScore' in value && (!Number.isFinite(value.minScore) || value.minScore < 0 || value.minScore > 100)) return false;
  if ('maxOffers' in value && (!Number.isFinite(value.maxOffers) || value.maxOffers < 1 || value.maxOffers > 50)) return false;
  if ('maxDaily' in value && (!Number.isFinite(value.maxDaily) || value.maxDaily < 1 || value.maxDaily > 100)) return false;
  return true;
}
function isCareerRunOptions(value) {
  if (!isPlainObject(value)) return false;
  const allowed = new Set(['autoSubmit', 'minScore', 'maxOffers', 'maxDaily']);
  for (const key of Object.keys(value)) if (!allowed.has(key)) return false;
  if ('autoSubmit' in value && typeof value.autoSubmit !== 'boolean') return false;
  if ('minScore' in value && (!Number.isFinite(value.minScore) || value.minScore < 0 || value.minScore > 100)) return false;
  if ('maxOffers' in value && (!Number.isFinite(value.maxOffers) || value.maxOffers < 1 || value.maxOffers > 50)) return false;
  if ('maxDaily' in value && (!Number.isFinite(value.maxDaily) || value.maxDaily < 1 || value.maxDaily > 100)) return false;
  return true;
}
function noArgs(args) { return args.length === 0; }
const CHANNEL_VALIDATORS = Object.freeze({
  'get-state': noArgs,
  navigate: (args) => args.length === 1 && isString(args[0]),
  'prewarm-site': (args) => args.length === 1 && isHttpUrl(args[0]),
  'chrome-overlay-height': (args) => args.length === 1 && Number.isFinite(args[0]) && args[0] >= 0 && args[0] <= 360,
  'new-tab': (args) => args.length <= 1 && (args.length === 0 || isString(args[0])),
  'activate-tab': (args) => args.length === 1 && isPositiveId(args[0]),
  'close-tab': (args) => args.length === 1 && isPositiveId(args[0]),
  'plus-menu': noArgs,
  'main-menu': noArgs,
  'search-engine-menu': (args) => args.length === 2 && args.every((v) => Number.isFinite(v) && v >= 0 && v < 10000),
  'toggle-rail-collapse': noArgs,
  'toggle-rail-pin': noArgs,
  'rail-position-menu': noArgs,
  'window-size-menu': (args) => args.length === 2 && args.every(v => Number.isSafeInteger(v) && v >= 0 && v <= 10000),
  'extension-install': noArgs,
  'extension-remove': (args) => args.length === 1 && isString(args[0], 128) && args[0].trim().length > 0,
  'sync-export': (args) => args.length === 1 && isString(args[0], 1024) && args[0].length >= 8,
  'sync-import': (args) => args.length === 1 && isString(args[0], 1024) && args[0].length >= 8,
  back: noArgs,
  forward: noArgs,
  reload: noArgs,
  stop: noArgs,
  home: noArgs,
  'toggle-favorite': noArgs,
  'toggle-ai': noArgs,
  'career-status': noArgs,
  'career-settings': (args) => args.length === 1 && isCareerSettingsPatch(args[0]),
  'career-run': (args) => args.length <= 1 && (args.length === 0 || isCareerRunOptions(args[0])),
  'career-stop': noArgs,
  'career-folder': noArgs,
  'career-open-folder': noArgs,
  'career-import': noArgs,
  'pick-wallpaper': noArgs,
  'wallpaper-data': noArgs,
  'ai-action': (args) => args.length === 2 && AI_ACTIONS.has(args[0]) && isString(args[1], 16000),
  'window-control': (args) => args.length === 1 && WINDOW_ACTIONS.has(args[0]),
  'set-chrome-lock': (args) => args.length === 1 && typeof args[0] === 'boolean',
  'set-setting': (args) => {
    if (args.length !== 2 || typeof args[0] !== 'string') return false;
    const [key, value] = args;
    if (key === 'searchEngine') return SEARCH_ENGINES.has(value);
    if (key === 'networkMode') return NETWORK_MODES.has(value);
    if (key === 'immersiveMode') return typeof value === 'boolean';
    if (key === 'railPosition') return ['top', 'left', 'right'].includes(value);
    if (key === 'appearance') return isAppearancePatch(value);
    if (key === 'generateWallpaper') return isString(value, 512) && value.trim().length > 0;
    if (key === 'resetWallpaper') return value === true;
    if (key === 'sideStageAction') return isSideAction(value);
    if (key === 'sideStageWidth') return Number.isFinite(value) && value >= 320 && value <= 620;
    if (key === 'sideStageEnabled') return typeof value === 'boolean';
    return false;
  },
  'remove-favorite': (args) => args.length === 1 && isHttpUrl(args[0]),
  'rename-favorite': (args) => args.length === 2 && isHttpUrl(args[0]) && isString(args[1], 240),
  'open-downloads': noArgs,
  'retry-veil': noArgs,
  'retry-current': noArgs
});
function normalizeUrl(raw) { try { const url = new URL(raw); url.hash = ''; return url.href; } catch { return ''; } }
function isTrustedShellEvent(event, shellUrl) {
  if (!event?.senderFrame || !event?.sender || !shellUrl) return false;
  const expected = normalizeUrl(shellUrl); const frameUrl = normalizeUrl(event.senderFrame.url); const contentsUrl = normalizeUrl(event.sender.getURL?.());
  if (!expected || frameUrl !== expected || contentsUrl !== expected) return false;
  return !event.senderFrame.parent;
}
function installIpcFirewall(ipcMain, { shellUrl }) {
  if (!ipcMain || typeof ipcMain.handle !== 'function') throw new TypeError('ipcMain invalide');
  if (ipcMain[INSTALLED]) return;
  const rawHandle = ipcMain.handle.bind(ipcMain);
  Object.defineProperty(ipcMain, INSTALLED, { value: true, configurable: false });
  ipcMain.handle = (channel, handler) => {
    const validateArgs = CHANNEL_VALIDATORS[channel];
    if (!validateArgs) throw new Error(`Canal IPC sans contrat de sécurité: ${channel}`);
    if (typeof handler !== 'function') throw new TypeError(`Handler IPC invalide: ${channel}`);
    return rawHandle(channel, (event, ...args) => {
      if (!isTrustedShellEvent(event, shellUrl)) throw new Error(`IPC refusé: sender non autorisé (${channel})`);
      if (!validateArgs(args)) throw new TypeError(`IPC refusé: arguments invalides (${channel})`);
      return handler(event, ...args);
    });
  };
}
module.exports = { CHANNEL_VALIDATORS, installIpcFirewall, isTrustedShellEvent };