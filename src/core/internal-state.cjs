const { detectIntent } = require('../services/intents.cjs');
const { normalizeEngine } = require('./navigation.cjs');

function parseInternal(url = 'quantic://newtab') {
  try {
    const u = new URL(url);
    if (u.protocol !== 'quantic:') return null;
    return { host: u.hostname, url: u };
  } catch {
    return null;
  }
}

function internalTitle(url = '') {
  const parsed = parseInternal(url);
  if (!parsed) return 'GEKKO';
  if (parsed.host === 'newtab') return 'Accueil';
  if (parsed.host === 'search') return parsed.url.searchParams.get('q') || 'Recherche';
  if (parsed.host === 'favorites') return 'Favoris';
  if (parsed.host === 'history') return 'Historique';
  if (parsed.host === 'settings') return 'Paramètres';
  if (parsed.host === 'career') return 'AURA Career';
  if (parsed.host === 'error') return 'Erreur';
  return 'GEKKO';
}

function buildInternalState(url, store, veilSnapshot = {}, careerSnapshot = null) {
  const parsed = parseInternal(url);
  if (!parsed) return null;

  if (parsed.host === 'newtab') return { kind: 'home' };

  if (parsed.host === 'search') {
    const query = parsed.url.searchParams.get('q') || '';
    const intent = detectIntent(query);
    return {
      kind: 'search',
      query,
      intent: { id: intent.id, label: intent.label, sites: intent.sites.map(([name, siteUrl]) => ({ name, url: siteUrl })) },
      providers: [
        { id: 'duckduckgo', name: 'DuckDuckGo', detail: 'Privé · direct', url: `https://duckduckgo.com/?q=${encodeURIComponent(query)}` },
        { id: 'qwant', name: 'Qwant', detail: 'Européen · direct', url: `https://www.qwant.com/?q=${encodeURIComponent(query)}&t=web` },
        { id: 'startpage', name: 'Startpage', detail: 'Résultats Google sans profilage', url: `https://www.startpage.com/sp/search?query=${encodeURIComponent(query)}` },
        { id: 'brave', name: 'Brave Search', detail: 'Index indépendant', url: `https://search.brave.com/search?q=${encodeURIComponent(query)}` },
        { id: 'searxng', name: 'SearXNG', detail: 'Métamoteur open source', url: `https://searxng.website/search?q=${encodeURIComponent(query)}` }
      ]
    };
  }

  if (parsed.host === 'favorites') {
    return { kind: 'favorites', items: store.favorites(300) };
  }

  if (parsed.host === 'history') {
    return { kind: 'history', items: store.history(250) };
  }

  if (parsed.host === 'settings') {
    const settings = store.settings();
    return {
      kind: 'settings',
      settings: { ...settings, searchEngine: normalizeEngine(settings.searchEngine) },
      veil: veilSnapshot || {}
    };
  }

  if (parsed.host === 'error') {
    return {
      kind: 'error',
      title: parsed.url.searchParams.get('title') || 'Impossible de charger cette page',
      detail: parsed.url.searchParams.get('detail') || ''
    };
  }

  return { kind: 'error', title: 'Page GEKKO introuvable', detail: url };
}

module.exports = { parseInternal, internalTitle, buildInternalState };
