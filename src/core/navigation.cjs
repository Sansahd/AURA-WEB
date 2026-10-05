const ENGINES = {
  brave: (q) => `https://search.brave.com/search?q=${encodeURIComponent(q)}`,
  duckduckgo: (q) => `https://duckduckgo.com/?q=${encodeURIComponent(q)}`,
  qwant: (q) => `https://www.qwant.com/?q=${encodeURIComponent(q)}&t=web`,
  startpage: (q) => `https://www.startpage.com/sp/search?query=${encodeURIComponent(q)}`,
  mojeek: (q) => `https://www.mojeek.com/search?q=${encodeURIComponent(q)}`
};

const COMMON_SITES = {
  google: 'https://www.google.com/',
  youtube: 'https://www.youtube.com/',
  github: 'https://github.com/',
  twitch: 'https://www.twitch.tv/',
  reddit: 'https://www.reddit.com/',
  wikipedia: 'https://fr.wikipedia.org/',
  chatgpt: 'https://chatgpt.com/',
  openai: 'https://openai.com/',
  amazon: 'https://www.amazon.fr/',
  spotify: 'https://open.spotify.com/',
  netflix: 'https://www.netflix.com/',
  mail: 'https://quanticmail.onrender.com/',
  quanticmail: 'https://quanticmail.onrender.com/',
  'quantic mail': 'https://quanticmail.onrender.com/',
  drive: 'https://drive.google.com/'
};

const VALID_ENGINES = new Set(['quantic', ...Object.keys(ENGINES)]);

function normalizeEngine(value) {
  return VALID_ENGINES.has(String(value || '').toLowerCase()) ? String(value).toLowerCase() : 'quantic';
}

function resolveInput(raw, searchEngine = 'quantic') {
  const value = String(raw || '').trim();
  if (!value) return { type: 'home', value: 'quantic://newtab' };

  const lower = value.toLowerCase();
  if (COMMON_SITES[lower]) return { type: 'url', value: COMMON_SITES[lower] };
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(value)) return { type: 'url', value };
  if (/^(localhost|127\.0\.0\.1)(:\d+)?(\/.*)?$/i.test(value)) return { type: 'url', value: `http://${value}` };
  if (/^[^\s]+\.[a-z]{2,}(?::\d+)?(?:\/.*)?$/i.test(value)) return { type: 'url', value: `https://${value}` };

  const selected = normalizeEngine(searchEngine);
  if (selected === 'quantic') return { type: 'search', value: `quantic://search?q=${encodeURIComponent(value)}` };
  return { type: 'search', value: ENGINES[selected](value) };
}

module.exports = { resolveInput, normalizeEngine, ENGINES, COMMON_SITES, VALID_ENGINES };
