'use strict';

const QUANTIC_PORTAL_ORIGIN = 'https://mediumorchid-badger-314305.hostingersite.com';
const LEGACY_DEFAULT_PINNED = ['youtube', 'twitch', 'spotify', 'netflix', 'quanticmail'];

const SIDE_APPS = Object.freeze({
  youtube: Object.freeze({ id: 'youtube', label: 'YouTube', short: 'YT', icon: '../assets/brands/youtube.svg', url: 'https://www.youtube.com/', media: true }),
  twitch: Object.freeze({ id: 'twitch', label: 'Twitch', short: 'TW', icon: '../assets/brands/twitch.svg', url: 'https://www.twitch.tv/', media: true }),
  spotify: Object.freeze({ id: 'spotify', label: 'Spotify', short: 'SP', icon: '../assets/brands/spotify.svg', url: 'https://open.spotify.com/', media: true }),
  netflix: Object.freeze({ id: 'netflix', label: 'Netflix', short: 'NF', icon: '../assets/brands/netflix.svg', url: 'https://www.netflix.com/', media: true, drm: true }),
  quanticmail: Object.freeze({ id: 'quanticmail', label: 'Quantic Mail', short: 'QM', icon: '../assets/brands/quantic-mail.svg', url: `${QUANTIC_PORTAL_ORIGIN}/mail/`, media: false, privacy: true, localFirst: true }),
  quanticpulse: Object.freeze({ id: 'quanticpulse', label: 'Quantic Pulse', short: 'QP', icon: '../assets/brands/quantic-pulse.svg', url: `${QUANTIC_PORTAL_ORIGIN}/pulse/`, media: false, social: true, privacy: true })
});

const DEFAULT_SIDESTAGE = Object.freeze({
  enabled: true,
  width: 420,
  activeApp: '',
  open: false,
  collapsed: false,
  pinnedApps: ['youtube', 'twitch', 'spotify', 'netflix', 'quanticmail', 'quanticpulse']
});

function clampWidth(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) return DEFAULT_SIDESTAGE.width;
  return Math.round(Math.min(620, Math.max(320, number)));
}

function migrateAppId(value) {
  const id = String(value || '');
  return id === 'proton' ? 'quanticmail' : id;
}

function isLegacyDefaultPinned(ids) {
  return ids.length === LEGACY_DEFAULT_PINNED.length
    && ids.every((id, index) => id === LEGACY_DEFAULT_PINNED[index]);
}

function normalizePinnedApps(value) {
  const provided = Array.isArray(value);
  const source = provided ? value : DEFAULT_SIDESTAGE.pinnedApps;
  const unique = [];
  for (const rawId of source) {
    const id = migrateAppId(rawId);
    if (!SIDE_APPS[id] || unique.includes(id)) continue;
    unique.push(id);
  }

  // Existing default profiles from pre-Pulse builds should receive Pulse once,
  // while custom SideStage selections remain untouched.
  if (provided && isLegacyDefaultPinned(unique) && !unique.includes('quanticpulse')) {
    unique.push('quanticpulse');
  }

  return unique.length ? unique : [...DEFAULT_SIDESTAGE.pinnedApps];
}

function normalizeSideStage(value = {}, current = DEFAULT_SIDESTAGE) {
  const source = value && typeof value === 'object' ? value : {};
  const base = current && typeof current === 'object' ? current : DEFAULT_SIDESTAGE;
  const pinnedApps = normalizePinnedApps(source.pinnedApps ?? base.pinnedApps);
  const requestedActive = migrateAppId(source.activeApp ?? base.activeApp ?? '');
  return {
    enabled: typeof source.enabled === 'boolean' ? source.enabled : base.enabled !== false,
    width: clampWidth(source.width ?? base.width),
    activeApp: SIDE_APPS[requestedActive] ? requestedActive : '',
    open: typeof source.open === 'boolean' ? source.open : Boolean(base.open),
    collapsed: typeof source.collapsed === 'boolean' ? source.collapsed : Boolean(base.collapsed),
    pinnedApps
  };
}

function publicAppList(settings = DEFAULT_SIDESTAGE) {
  const normalized = normalizeSideStage(settings);
  return normalized.pinnedApps.map((id) => SIDE_APPS[id]).filter(Boolean).map((app) => ({ ...app }));
}

function appDefinition(id) {
  return SIDE_APPS[migrateAppId(id)] || null;
}

module.exports = {
  SIDE_APPS,
  DEFAULT_SIDESTAGE,
  normalizeSideStage,
  publicAppList,
  appDefinition,
  clampWidth,
  migrateAppId
};
