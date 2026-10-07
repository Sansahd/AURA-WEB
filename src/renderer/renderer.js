let state = { tabs: [], activeId: null, aiOpen: false, chromeVisible: true, immersive: false, internal: { kind: 'home' } };
let pendingState = null;
let framePending = false;
let aiBusy = false;
let lastActiveId = null;
let lastInternalKey = '';
const tabNodes = new Map();

const $ = (selector) => document.querySelector(selector);
const tabsEl = $('#tabs');
const address = $('#address');
const home = $('#home');
const internalPage = $('#internal-page');
const internalContent = $('#internal-content');
const aiPanel = $('#ai-panel');
const aiOutput = $('#ai-output');
const aiPrompt = $('#ai-prompt');
const aiEngine = $('#ai-engine');
const back = $('#back');
const forward = $('#forward');
const fav = $('#fav');
const reload = $('#reload');
const navigationTransition = $('#navigation-transition');
const appsPanel = $('#apps-panel');
const sideStageRail = $('#sidestage-rail');
const searchEngineButton = $('#search-engine-button');
const searchEngineMenu = $('#search-engine-menu');
const searchEngineMark = $('#search-engine-mark');
const searchEngineMarkText = $('#search-engine-mark-text');
const searchEngineLabel = $('#search-engine-label');
const siteSuggestions = $('#site-suggestions');
const cinemaTarget = $('#cinema-target');
const cinemaStatus = $('#cinema-status');
let siteMatches = [];
let siteSelection = 0;
let lastWallpaperVersion = -1;
let lastSideStageKey = '';


function active() { return state.tabs.find((tab) => tab.id === state.activeId); }
function fire(promise) { Promise.resolve(promise).catch(() => {}); }
const SEARCH_ENGINES = Object.freeze({
  gekko: { label: 'GEKKO', mark: 'G', detail: 'Recherche interne' },
  duckduckgo: { label: 'DuckDuckGo', mark: 'D', detail: 'Privé · direct' },
  qwant: { label: 'Qwant', mark: 'Q', detail: 'Européen · direct' },
  startpage: { label: 'Startpage', mark: 'S', detail: 'Google sans profilage' },
  brave: { label: 'Brave', mark: 'B', detail: 'Index indépendant' },
  searxng: { label: 'SearXNG', mark: 'Sx', detail: 'Métamoteur open source' },
  tor: { label: 'Tor', mark: 'T', detail: 'DuckDuckGo via Veil' }
});
function currentSearchEngine() {
  return SEARCH_ENGINES[state.settings?.searchEngine] ? state.settings.searchEngine : 'gekko';
}
function renderSearchEngineControl() {
  const key = currentSearchEngine();
  const meta = SEARCH_ENGINES[key];
  if (searchEngineMarkText) searchEngineMarkText.textContent = meta.mark;
  if (searchEngineLabel) searchEngineLabel.textContent = meta.label;
  if (searchEngineButton) {
    searchEngineButton.dataset.engine = key;
    searchEngineButton.title = `Moteur de recherche · ${meta.label} · ${meta.detail}`;
  }
  document.querySelectorAll('[data-search-engine]').forEach((button) => {
    const active = button.dataset.searchEngine === key;
    button.setAttribute('aria-checked', active ? 'true' : 'false');
    button.classList.toggle('selected', active);
  });
}
function el(tag, className = '', text = '') {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

function closeSiteSuggestions() {
  siteMatches = [];
  siteSelection = 0;
  siteSuggestions?.classList.add('hidden');
  siteSuggestions?.replaceChildren();
  address?.setAttribute('aria-expanded', 'false');
}

function refreshSiteSelection() {
  if (!siteSuggestions) return;
  [...siteSuggestions.querySelectorAll('.site-suggestion')].forEach((button, index) => {
    const selected = index === siteSelection;
    button.classList.toggle('selected', selected);
    button.setAttribute('aria-selected', selected ? 'true' : 'false');
  });
}

function renderSiteSuggestions(matches = siteMatches) {
  if (!siteSuggestions) return;
  siteSuggestions.replaceChildren();
  siteMatches = Array.isArray(matches) ? matches : [];
  if (!siteMatches.length || document.activeElement !== address) {
    siteSuggestions.classList.add('hidden');
    address?.setAttribute('aria-expanded', 'false');
    return;
  }

  siteSuggestions.classList.remove('hidden');
  address?.setAttribute('aria-expanded', 'true');
  address?.setAttribute('aria-controls', 'site-suggestions');

  siteMatches.forEach((site, index) => {
    const button = el('button', 'site-suggestion' + (index === siteSelection ? ' selected' : ''));
    button.type = 'button';
    button.role = 'option';
    button.setAttribute('aria-selected', index === siteSelection ? 'true' : 'false');
    button.dataset.url = site.url;

    const icon = el('span', 'site-suggestion-icon', site.name.slice(0, 1).toUpperCase());
    const copy = el('span', 'site-suggestion-copy');
    copy.append(el('strong', '', site.name), el('small', '', new URL(site.url).hostname.replace(/^www\./, '')));
    const meta = el('span', 'site-suggestion-meta', site.tag || 'Site');
    const arrow = el('span', 'site-suggestion-arrow', '↗');

    button.append(icon, copy, meta, arrow);
    button.onpointerdown = (event) => event.preventDefault();
    button.onmouseenter = () => {
      siteSelection = index;
      refreshSiteSelection();
    };
    button.onclick = () => openSiteSuggestion(index);
    siteSuggestions.append(button);
  });
}

function updateSiteSuggestions() {
  const query = address?.value || '';
  const matches = window.GekkoSiteCache?.matchSites?.(query, 5) || [];
  siteSelection = 0;
  renderSiteSuggestions(matches);
}

function openSiteSuggestion(index = siteSelection) {
  const site = siteMatches[index];
  if (!site) return false;
  closeSiteSuggestions();
  address.value = site.url;
  address.blur();
  fire(window.quantic.navigate(site.url));
  return true;
}

function navigateAddressValue() {
  const value = address.value.trim();
  if (!value) return;
  if (siteMatches.length && openSiteSuggestion(siteSelection)) return;
  closeSiteSuggestions();
  address.blur();
  fire(window.quantic.navigate(value));
}

function cinematicTargetFor(tab) {
  const raw = tab?.url || '';
  if (!raw) return 'Ouverture…';
  try {
    const url = new URL(raw);
    if (url.protocol === 'http:' || url.protocol === 'https:') {
      return url.hostname.replace(/^www\./, '');
    }
    if (url.protocol === 'quantic:') return 'GEKKO';
  } catch {}
  return 'Ouverture…';
}

function createTabNode(tab) {
  const button = el('button', 'tab no-drag');
  button.type = 'button';
  const title = el('span', 'tab-title');
  const close = el('span', 'tab-x', '×');
  close.title = 'Fermer';
  close.onclick = (event) => { event.stopPropagation(); fire(window.quantic.closeTab(tab.id)); };
  button.onclick = () => fire(window.quantic.activateTab(tab.id));
  button.append(title, close);
  button._title = title;
  return button;
}

function renderTabs() {
  const seen = new Set();
  let activeNode = null;
  for (const tab of state.tabs) {
    seen.add(tab.id);
    let node = tabNodes.get(tab.id);
    if (!node) {
      node = createTabNode(tab);
      tabNodes.set(tab.id, node);
    }
    node.classList.toggle('active', tab.id === state.activeId);
    node.classList.toggle('loading', Boolean(tab.loading));
    node.title = tab.title || 'Nouvel onglet';
    const title = tab.title || 'Nouvel onglet';
    if (node._title.textContent !== title) node._title.textContent = title;
    tabsEl.append(node);
    if (tab.id === state.activeId) activeNode = node;
  }

  for (const [id, node] of tabNodes) {
    if (!seen.has(id)) {
      node.remove();
      tabNodes.delete(id);
    }
  }

  if (state.activeId !== lastActiveId && activeNode) {
    requestAnimationFrame(() => activeNode.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' }));
  }
  lastActiveId = state.activeId;
}

function displayAddress(tab) {
  if (!tab) return '';
  if (tab.url?.startsWith('quantic://newtab')) return '';
  if (state.internal?.kind === 'search') return state.internal.query || '';
  return tab.url || '';
}

function setInternalVisibility() {
  const kind = state.internal?.kind || null;
  home.classList.toggle('hidden', kind !== 'home');
  internalPage.classList.toggle('hidden', !kind || kind === 'home');
}

function cardButton(title, subtitle, onClick) {
  const button = el('button', 'internal-card');
  button.type = 'button';
  button.append(el('strong', '', title), el('small', '', subtitle || ''));
  button.onclick = onClick;
  return button;
}

function renderSearch(data) {
  const head = el('div', 'internal-head');
  const text = el('div');
  text.append(el('h1', '', data.query || 'Recherche'), el('p', '', `Intention détectée : ${data.intent?.label || 'Rechercher'}`));
  head.append(text);
  internalContent.append(head);

  if (data.intent?.sites?.length) {
    const section = el('section', 'internal-section');
    section.append(el('h2', '', 'Suggestions GEKKO'));
    const grid = el('div', 'internal-grid');
    for (const site of data.intent.sites) grid.append(cardButton(site.name, 'Ouvrir le site', () => fire(window.quantic.navigate(site.url))));
    section.append(grid);
    internalContent.append(section);
  }

  const web = el('section', 'internal-section');
  web.append(el('h2', '', 'Rechercher sur le Web'));
  const grid = el('div', 'internal-grid');
  for (const provider of data.providers || []) grid.append(cardButton(provider.name, provider.detail || 'Recherche privée', () => fire(window.quantic.navigate(provider.url))));
  web.append(grid);
  internalContent.append(web);
}

function renderFavorites(data) {
  const head = el('div', 'internal-head');
  const text = el('div');
  text.append(el('h1', '', 'Favoris'), el('p', '', 'Stockés localement dans Quantic.'));
  head.append(text);
  internalContent.append(head);

  const list = el('div', 'internal-list');
  for (const item of data.items || []) {
    const row = el('div', 'internal-row');
    const grow = el('div', 'grow');
    grow.append(el('strong', '', item.title || item.url), el('small', '', item.url));
    const actions = el('div', 'internal-actions');
    const open = el('button', '', 'Ouvrir');
    open.onclick = () => fire(window.quantic.navigate(item.url));
    const rename = el('button', '', 'Renommer');
    rename.onclick = async () => {
      const next = prompt('Nom du favori', item.title || item.url);
      if (next?.trim()) await window.quantic.renameFavorite(item.url, next.trim());
    };
    const remove = el('button', '', 'Supprimer');
    remove.onclick = () => fire(window.quantic.removeFavorite(item.url));
    actions.append(open, rename, remove);
    row.append(grow, actions);
    list.append(row);
  }
  if (!list.children.length) list.append(el('div', 'error-card muted', 'Aucun favori.'));
  internalContent.append(list);
}

function renderHistory(data) {
  const head = el('div', 'internal-head');
  const text = el('div');
  text.append(el('h1', '', 'Historique'), el('p', '', 'Historique local, sans synchronisation distante.'));
  head.append(text);
  internalContent.append(head);

  const list = el('div', 'internal-list');
  for (const item of data.items || []) {
    const row = el('button', 'internal-row');
    row.type = 'button';
    row.style.textAlign = 'left';
    row.style.color = 'inherit';
    row.style.width = '100%';
    row.style.cursor = 'pointer';
    const grow = el('div', 'grow');
    grow.append(el('strong', '', item.title || item.url), el('small', '', item.url));
    row.append(grow);
    row.onclick = () => fire(window.quantic.navigate(item.url));
    list.append(row);
  }
  if (!list.children.length) list.append(el('div', 'error-card muted', 'Aucun historique.'));
  internalContent.append(list);
}

function wallpaperPalette(prompt) {
  const p=String(prompt||'').toLowerCase();
  const sets=[[['forest','forêt','nature','jungle'],['#071f17','#0d5135','#68d391','#d9f99d']],[['ocean','mer','sea','water','eau'],['#041b2d','#075985','#22d3ee','#bae6fd']],[['space','espace','galaxy','galaxie','cosmos'],['#090b22','#312e81','#7c3aed','#d8b4fe']],[['sunset','coucher','orange','gold','doré'],['#2a1020','#9a3412','#fb923c','#fde68a']],[['cyber','neon','néon','futur','sci-fi'],['#071226','#1d4ed8','#7c3aed','#22d3ee']],[['pink','rose','cherry','sakura'],['#260b1d','#9d174d','#f472b6','#fce7f3']]];
  return sets.find(([words])=>words.some(w=>p.includes(w)))?.[1]||['#07111f','#163b63','#5b8cff','#9ad8ff'];
}
function localHash(s){let a=2166136261>>>0,b=2654435761>>>0;for(const ch of String(s||'Quantic')){const c=ch.codePointAt(0)||0;a=Math.imul(a^c,16777619)>>>0;b=Math.imul(b^(c+a),2246822519)>>>0;}return[a,b];}
function localWallpaperData(prompt){const colors=wallpaperPalette(prompt),[h1,h2]=localHash(prompt);const circles=Array.from({length:12},(_,i)=>{const a=(h1+Math.imul(i+3,2654435761))>>>0,b=(h2+Math.imul(i+7,1597334677))>>>0;return '<circle cx="'+(a%1920)+'" cy="'+(b%1080)+'" r="'+(120+((a^b)%420))+'" fill="'+colors[i%colors.length]+'" opacity="'+(0.09+((a>>>9)%22)/100).toFixed(2)+'"/>';}).join('');const svg='<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080"><defs><linearGradient id="g"><stop stop-color="'+colors[0]+'"/><stop offset=".5" stop-color="'+colors[1]+'"/><stop offset="1" stop-color="'+colors[2]+'"/></linearGradient><filter id="b"><feGaussianBlur stdDeviation="68"/></filter></defs><rect width="1920" height="1080" fill="url(#g)"/><g filter="url(#b)">'+circles+'</g><rect width="1920" height="1080" fill="#020617" opacity=".18"/></svg>';return 'data:image/svg+xml;base64,'+btoa(unescape(encodeURIComponent(svg)));}
function renderPersonaSettings(data){const appearance=data.settings?.appearance||{};const section=el('section','internal-section');section.append(el('h2','','Quantic Persona · 100 % local'),el('div','persona-note','Couleurs, verre et fond sont calculés et stockés sur cet appareil. Aucun compte ni cloud n’est requis.'));const grid=el('div','persona-grid');const accentBox=el('div','persona-control');accentBox.append(el('label','','Couleur d’accent'));const accent=document.createElement('input');accent.type='color';accent.value=appearance.accent||'#7aa2ff';accent.oninput=()=>document.documentElement.style.setProperty('--quantic-accent',accent.value);accent.onchange=()=>fire(window.quantic.setSetting('appearance',{accent:accent.value}));accentBox.append(accent);const glassBox=el('div','persona-control');glassBox.append(el('label','','Intensité du verre'));const glass=document.createElement('input');glass.type='range';glass.min='.25';glass.max='.95';glass.step='.05';glass.value=String(appearance.glassOpacity??.72);glass.onchange=()=>fire(window.quantic.setSetting('appearance',{glassOpacity:Number(glass.value)}));glassBox.append(glass);const radiusBox=el('div','persona-control');radiusBox.append(el('label','','Arrondi de la fenêtre'));const radius=document.createElement('input');radius.type='range';radius.min='6';radius.max='28';radius.value=String(appearance.radius??14);radius.onchange=()=>fire(window.quantic.setSetting('appearance',{radius:Number(radius.value)}));radiusBox.append(radius);grid.append(accentBox,glassBox,radiusBox);section.append(grid);const wallpaper=el('div','persona-control');wallpaper.style.marginTop='12px';wallpaper.append(el('label','','Fond personnalisé'));const input=document.createElement('input');input.type='text';input.maxLength=512;input.value=appearance.wallpaperPrompt||'';input.placeholder='Ex. forêt cyberpunk bleue, néons et pluie';const actions=el('div','persona-actions');const generate=el('button','pill-button','Générer localement');generate.onclick=()=>{const p=input.value.trim();if(p)fire(window.quantic.setSetting('generateWallpaper',p));};const choose=el('button','pill-button','Choisir une image du PC');choose.onclick=()=>fire(window.quantic.pickWallpaper());const reset=el('button','pill-button','Retirer le fond');reset.onclick=()=>fire(window.quantic.setSetting('resetWallpaper',true));actions.append(generate,choose,reset);wallpaper.append(input,actions,el('div','persona-note','Prompt ou image locale : rien ne quitte votre appareil. PNG, JPG, WEBP et SVG, 20 Mo max.'));section.append(wallpaper);internalContent.append(section);}
function renderSideStageSettings(data){const s=data.settings?.sideStage||{};const section=el('section','internal-section');section.append(el('h2','','SideStage · lecteurs persistants'),el('div','persona-note','Les lecteurs restent actifs dans SideStage quand vous changez d’onglet principal.'));const toggle=el('button','pill-button '+(s.enabled!==false?'active':''),s.enabled!==false?'Activé':'Désactivé');toggle.onclick=()=>fire(window.quantic.setSetting('sideStageEnabled',s.enabled===false));section.append(toggle);const box=el('div','persona-control');box.style.marginTop='12px';box.append(el('label','','Largeur du lecteur'));const width=document.createElement('input');width.type='range';width.min='320';width.max='620';width.step='20';width.value=String(s.width||420);width.onchange=()=>fire(window.quantic.setSetting('sideStageWidth',Number(width.value)));box.append(width,el('div','persona-note','YouTube, Twitch et Spotify restent actifs pendant la navigation. Netflix nécessite Widevine pour les contenus DRM.'));section.append(box);internalContent.append(section);}

function renderSettings(data) {
  const head = el('div', 'internal-head');
  const text = el('div');
  text.append(el('h1', '', 'Paramètres'), el('p', '', 'Seulement les réglages utiles.'));
  const privateMode = data.settings?.networkMode === 'private';
  const networkOk = !privateMode || data.veil?.connected;
  const status = el('span', `status ${networkOk ? 'ok' : 'bad'}`);
  status.append(el('span', 'status-dot'), document.createTextNode(privateMode ? (data.veil?.connected ? 'Tor actif' : 'Tor indisponible') : 'Mode compatible'));
  head.append(text, status);
  internalContent.append(head);

  renderPersonaSettings(data);
  renderSideStageSettings(data);

  const engine = el('section', 'internal-section');
  engine.append(el('h2', '', 'Moteur de recherche'));
  const row = el('div', 'engine-row');
  const engines = [
    ['gekko','GEKKO Search'],
    ['duckduckgo','DuckDuckGo'],
    ['qwant','Qwant'],
    ['startpage','Startpage'],
    ['brave','Brave Search'],
    ['searxng','SearXNG'],
    ['tor','Tor · Veil']
  ];
  for (const [id, label] of engines) {
    const button = el('button', `pill-button ${data.settings?.searchEngine === id ? 'active' : ''}`, label);
    button.onclick = () => fire(window.quantic.setSetting('searchEngine', id));
    row.append(button);
  }
  engine.append(row);
  internalContent.append(engine);

  const network = el('section', 'internal-section');
  network.append(el('h2', '', 'Réseau'));
  const networkRow = el('div', 'engine-row');
  const balanced = el('button', `pill-button ${data.settings?.networkMode !== 'private' ? 'active' : ''}`, 'Compatible');
  balanced.title = 'Connexion directe, protections locales Quantic, meilleure compatibilité vidéo et anti-bot.';
  balanced.onclick = () => fire(window.quantic.setSetting('networkMode', 'balanced'));
  const privateButton = el('button', `pill-button ${data.settings?.networkMode === 'private' ? 'active' : ''}`, 'Privé · Tor');
  privateButton.title = 'Masque l’adresse IP mais peut ralentir ou déclencher des contrôles sur certains sites.';
  privateButton.onclick = () => fire(window.quantic.setSetting('networkMode', 'private'));
  networkRow.append(balanced, privateButton);
  network.append(networkRow, el('div', 'muted', data.settings?.networkMode === 'private' ? 'Tor masque votre IP, avec une compatibilité parfois réduite.' : 'Votre IP reste visible aux sites ; les protections locales Quantic restent actives.'));
  internalContent.append(network);

  const immersion = el('section', 'internal-section');
  immersion.append(el('h2', '', 'Immersion'));
  const toggle = el('button', `pill-button ${data.settings?.immersiveMode !== false ? 'active' : ''}`, data.settings?.immersiveMode !== false ? 'Activée' : 'Désactivée');
  toggle.onclick = () => fire(window.quantic.setSetting('immersiveMode', data.settings?.immersiveMode === false));
  immersion.append(toggle);
  internalContent.append(immersion);

  if (data.settings?.networkMode === 'private' && !data.veil?.connected && data.veil?.error) {
    const network = el('section', 'internal-section');
    network.append(el('h2', '', 'Réseau privé'));
    const box = el('div', 'error-card');
    box.append(el('div', 'muted', data.veil.error));
    const retry = el('button', 'pill-button', 'Réessayer');
    retry.style.marginTop = '10px';
    retry.onclick = () => fire(window.quantic.retryVeil());
    box.append(retry);
    network.append(box);
    internalContent.append(network);
  }
}


function renderConnecting(data) {
  const head = el('div', 'internal-head');
  const text = el('div');
  let host = data.url || 'Internet';
  try { host = new URL(data.url).hostname || host; } catch {}
  const privatePhase = data.phase === 'private-network';
  text.append(el('h1', '', host), el('p', '', privatePhase ? 'Connexion privée en cours…' : 'Chargement de la page…'));
  const status = el('span', 'status');
  status.append(el('span', 'status-dot'), document.createTextNode(privatePhase ? (data.veil?.status || 'initialisation') : 'GEKKO'));
  head.append(text, status);
  internalContent.append(head);
  const box = el('div', 'error-card');
  box.append(el('div', 'muted', privatePhase ? 'Quantic prépare le circuit privé avant de laisser la page communiquer avec Internet.' : 'La page se charge derrière cette interface afin d’éviter tout flash blanc.'));
  internalContent.append(box);
}

function renderCareer(data) {
  const career = data.career || {};
  const config = career.config || {};
  const applications = career.applications || [];
  const offers = career.offers || [];
  const events = career.events || [];
  const submitted = applications.filter((item) => item.status === 'submitted').length;
  const prepared = applications.filter((item) => item.status === 'prepared').length;
  const review = applications.filter((item) => ['needs_review', 'paused'].includes(item.status)).length;

  const head = el('div', 'internal-head');
  const text = el('div');
  text.append(
    el('h1', '', 'AURA Career'),
    el('p', '', career.running ? 'Recherche et candidatures en cours dans GEKKO.' : 'Agent de candidature intégré à GEKKO.')
  );
  const status = el('span', 'status');
  status.append(el('span', 'status-dot'), document.createTextNode(career.running ? 'actif' : 'au repos'));
  head.append(text, status);
  internalContent.append(head);

  const controls = el('section', 'internal-section');
  controls.append(el('h2', '', 'Pilotage'));
  const actions = el('div', 'internal-actions');
  const run = el('button', 'pill-button', career.running ? 'En cours…' : 'Lancer maintenant');
  run.disabled = Boolean(career.running);
  run.onclick = () => fire(window.quantic.careerRun());
  const stop = el('button', 'pill-button', 'Arrêter');
  stop.disabled = !career.running;
  stop.onclick = () => fire(window.quantic.careerStop());
  const settings = career.settings || {};
  const autopilot = el('button', 'pill-button', 'Autopilote : ' + (settings.autopilot === false ? 'OFF' : 'ON'));
  autopilot.onclick = () => fire(window.quantic.careerSettings({ autopilot: settings.autopilot === false }));

  const autoSubmit = el('button', 'pill-button', 'Envoi automatique : ' + (settings.autoSubmit ? 'ON' : 'OFF'));
  autoSubmit.onclick = () => {
    const enable = !settings.autoSubmit;
    if (enable && !window.confirm('Activer l’envoi automatique des candidatures suffisamment compatibles ? AURA s’arrêtera toujours sur CAPTCHA, 2FA et questions sensibles.')) return;
    fire(window.quantic.careerSettings({ autoSubmit: enable }));
  };

  const importButton = el('button', 'pill-button', 'Importer profil + CV');
  importButton.onclick = () => fire(window.quantic.careerImport());
  const folder = el('button', 'pill-button', 'Dossier AURA Career');
  folder.onclick = () => fire(window.quantic.careerOpenFolder());
  actions.append(run, stop, autopilot, autoSubmit, importButton, folder);
  controls.append(actions);
  internalContent.append(controls);

  const configSection = el('section', 'internal-section');
  configSection.append(el('h2', '', 'Configuration'));
  const configGrid = el('div', 'internal-grid');
  const cfgCard = (title, ok, detail) => {
    const card = el('div', 'internal-card');
    card.append(el('strong', '', title), el('small', '', ok ? 'Prêt' : detail));
    if (!ok) card.classList.add('warning');
    return card;
  };
  configGrid.append(
    cfgCard('Profil', Boolean(config.profile), 'profile.json manquant'),
    cfgCard('CV', Boolean(config.cv), 'cv.pdf manquant'),
    cfgCard('Sources', Boolean(config.searches), 'searches.json manquant')
  );
  configSection.append(configGrid);
  internalContent.append(configSection);

  const metrics = el('section', 'internal-section');
  metrics.append(el('h2', '', 'Pipeline'));
  const metricGrid = el('div', 'internal-grid');
  for (const [title, value] of [
    ['Offres analysées', offers.length],
    ['Préparées', prepared],
    ['Envoyées', submitted],
    ['À vérifier', review],
  ]) {
    const card = el('div', 'internal-card');
    card.append(el('strong', '', String(value)), el('small', '', title));
    metricGrid.append(card);
  }
  metrics.append(metricGrid);
  internalContent.append(metrics);

  const recent = el('section', 'internal-section');
  recent.append(el('h2', '', 'Candidatures récentes'));
  const list = el('div', 'internal-list');
  for (const item of applications.slice(0, 20)) {
    const row = el('div', 'internal-row');
    const grow = el('div', 'grow');
    grow.append(
      el('strong', '', item.title || item.company || 'Candidature'),
      el('small', '', [item.company, item.status, item.score != null ? ('score ' + item.score) : ''].filter(Boolean).join(' · '))
    );
    const open = el('button', '', 'Ouvrir');
    open.onclick = () => item.url && fire(window.quantic.navigate(item.url));
    row.append(grow, open);
    list.append(row);
  }
  if (!applications.length) list.append(el('div', 'muted', 'Aucune candidature enregistrée pour le moment.'));
  recent.append(list);
  internalContent.append(recent);

  if (career.lastError || events.length) {
    const activity = el('section', 'internal-section');
    activity.append(el('h2', '', 'Activité'));
    if (career.lastError) {
      const box = el('div', 'error-card');
      box.append(el('div', 'muted', career.lastError));
      activity.append(box);
    }
    const log = el('div', 'internal-list');
    for (const event of events.slice(0, 12)) {
      const row = el('div', 'internal-row');
      row.append(
        el('strong', '', event.type || 'événement'),
        el('small', '', event.at || '')
      );
      log.append(row);
    }
    activity.append(log);
    internalContent.append(activity);
  }
}

function renderError(data) {
  const head = el('div', 'internal-head');
  const text = el('div');
  text.append(el('h1', '', data.title || 'Erreur'), el('p', '', 'Quantic a arrêté le chargement proprement.'));
  head.append(text);
  internalContent.append(head);
  const box = el('div', 'error-card');
  box.append(el('div', 'muted', data.detail || 'Erreur inconnue'));
  const retry = el('button', 'pill-button', 'Réessayer');
  retry.style.marginTop = '12px';
  retry.onclick = () => fire(window.quantic.retryCurrent());
  box.append(retry);
  internalContent.append(box);
}

function renderInternal() {
  const data = state.internal;
  const key = JSON.stringify(data || null);
  if (key === lastInternalKey) return;
  lastInternalKey = key;
  internalContent.replaceChildren();
  if (!data || data.kind === 'home') return;
  if (data.kind === 'connecting') renderConnecting(data);
  else if (data.kind === 'search') renderSearch(data);
  else if (data.kind === 'favorites') renderFavorites(data);
  else if (data.kind === 'history') renderHistory(data);
  else if (data.kind === 'settings') renderSettings(data);
  else if (data.kind === 'career') renderCareer(data);
  else renderError(data);
}

function applyAppearance(){const a=state.settings?.appearance||{},root=document.documentElement;root.style.setProperty('--quantic-accent',a.accent||'#7aa2ff');root.style.setProperty('--quantic-glass-opacity',String(a.glassOpacity??.72));root.style.setProperty('--quantic-window-radius',String(Number(a.radius||14))+'px');document.body.classList.toggle('private-mode',state.settings?.networkMode==='private');const hasWallpaper=a.wallpaperMode!=='none';document.body.classList.toggle('has-wallpaper',hasWallpaper);const v=Number(a.wallpaperVersion||0);if(v===lastWallpaperVersion)return;lastWallpaperVersion=v;if(!hasWallpaper){root.style.setProperty('--quantic-wallpaper-image','none');return;}fire(window.quantic.wallpaperData().then((image)=>{if(v!==lastWallpaperVersion)return;const ok=Boolean(image);document.body.classList.toggle('has-wallpaper',ok);root.style.setProperty('--quantic-wallpaper-image',ok?'url('+JSON.stringify(image)+')':'none');}));}
function renderSideStage(){const data=state.sideStage||{enabled:false,apps:[]},key=JSON.stringify(data);if(key===lastSideStageKey)return;lastSideStageKey=key;sideStageRail.classList.toggle('hidden',!data.enabled||data.privateDisabled);sideStageRail.classList.toggle('collapsed',Boolean(data.collapsed));sideStageRail.replaceChildren();if(!data.enabled||data.privateDisabled)return;const collapse=el('button','side-stage-collapse',data.collapsed?'‹':'›');collapse.title=data.collapsed?'Déployer SideStage':'Rétracter SideStage';collapse.onclick=()=>fire(window.quantic.setSetting('sideStageAction','collapse'));sideStageRail.append(collapse);if(data.collapsed)return;sideStageRail.append(el('div','side-stage-brand','SIDE'));for(const app of data.apps||[]){const button=el('button','side-stage-app '+(data.open&&data.activeApp===app.id?'active ':'')+(app.status==='loading'?'loading':''));button.title=app.label||app.id;button.setAttribute('aria-label',app.label||app.id);if(app.icon){const icon=document.createElement('img');icon.className='side-stage-icon';icon.src=app.icon;icon.alt='';button.append(icon);}else button.textContent=app.short||app.label.slice(0,2);button.onclick=()=>fire(window.quantic.setSetting('sideStageAction','toggle:'+app.id));button.oncontextmenu=(e)=>{e.preventDefault();fire(window.quantic.setSetting('sideStageAction','reload:'+app.id));};sideStageRail.append(button);}sideStageRail.append(el('div','side-stage-spacer'));if(data.open){const close=el('button','side-stage-close','×');close.title='Masquer le lecteur';close.onclick=()=>fire(window.quantic.setSetting('sideStageAction','close'));sideStageRail.append(close);}}

function render() {
  applyAppearance();
  renderSearchEngineControl();
  renderSideStage();
  renderTabs();
  const tab = active();
  if (document.activeElement !== address) {
    const nextAddress = displayAddress(tab);
    if (address.value !== nextAddress) address.value = nextAddress;
  }

  back.disabled = !tab?.canGoBack;
  forward.disabled = !tab?.canGoForward;
  fav.classList.toggle('active', Boolean(tab?.favorite));
  fav.title = tab?.favorite ? 'Retirer des favoris' : 'Ajouter aux favoris';
  reload.classList.toggle('loading', Boolean(tab?.loading));
  reload.title = tab?.loading ? 'Arrêter' : 'Actualiser';
  const cinematicActive = Boolean(tab?.transitioning);
  navigationTransition?.classList.toggle('active', cinematicActive);
  navigationTransition?.setAttribute('aria-hidden', cinematicActive ? 'false' : 'true');
  if (cinematicActive) {
    if (cinemaTarget) cinemaTarget.textContent = cinematicTargetFor(tab);
    if (cinemaStatus) cinemaStatus.textContent = tab?.awaitingNetwork
      ? 'Création du circuit privé…'
      : (tab?.loading ? 'Chargement de la page…' : 'Préparation de la page…');
  }
  $('#home-button')?.classList.toggle('active', state.internal?.kind === 'home');

  setInternalVisibility();
  renderInternal();
  aiPanel.classList.toggle('closed', !state.aiOpen);
  if (aiEngine) {
    const runtime = state.aiRuntime || {};
    aiEngine.textContent = runtime.label || 'AURA 2.0';
    aiEngine.classList.toggle('fallback', Boolean(runtime.fallback));
    const details = [runtime.model, runtime.role].filter(Boolean).join(' · ');
    aiEngine.title = details || (runtime.lastError || 'AURA 2.0');
  }
  document.body.classList.toggle('chrome-hidden', state.immersive && !state.chromeVisible);
  document.body.classList.toggle('immersive', state.immersive && !state.chromeVisible);
}

function acceptState(next) {
  pendingState = next;
  if (framePending) return;
  framePending = true;
  requestAnimationFrame(() => {
    framePending = false;
    if (pendingState) state = pendingState;
    pendingState = null;
    render();
  });
}

window.quantic.onState(acceptState);
fire(window.quantic.state().then(acceptState));

$('#logo').onclick = () => fire(window.quantic.home());
$('#home-button').onclick = () => fire(window.quantic.home());
$('#discover').onclick = () => fire(window.quantic.navigate('Découvrir le web'));
$('#bookmarks').onclick = () => fire(window.quantic.newTab('quantic://favorites'));
$('#downloads').onclick = () => fire(window.quantic.navigate('https://mediumorchid-badger-314305.hostingersite.com/downloads/#gekko'));
$('#apps-button').onclick = () => appsPanel?.classList.toggle('hidden');
$('#home-tool-apps').onclick = () => appsPanel?.classList.toggle('hidden');
$('#home-tool-home').onclick = () => fire(window.quantic.home());
$('#home-tool-settings').onclick = () => fire(window.quantic.newTab('quantic://settings'));
$('#apps-close').onclick = () => appsPanel?.classList.add('hidden');
document.querySelectorAll('[data-app-url]').forEach((button) => {
  button.onclick = () => {
    appsPanel?.classList.add('hidden');
    fire(window.quantic.newTab(button.dataset.appUrl));
  };
});
$('#plus').onclick = () => fire(window.quantic.newTab());
$('#plus').oncontextmenu = (event) => { event.preventDefault(); fire(window.quantic.plusMenu()); };
$('#menu').onclick = () => fire(window.quantic.mainMenu());
$('#persona').onclick = () => fire(window.quantic.newTab('quantic://settings'));

searchEngineButton.onclick = () => {
  closeSiteSuggestions();
  const opening = searchEngineMenu.classList.contains('hidden');
  searchEngineMenu.classList.toggle('hidden', !opening);
  searchEngineButton.setAttribute('aria-expanded', opening ? 'true' : 'false');
};
document.querySelectorAll('[data-search-engine]').forEach((button) => {
  button.onclick = async () => {
    const engine = button.dataset.searchEngine;
    if (!SEARCH_ENGINES[engine]) return;
    searchEngineMenu.classList.add('hidden');
    searchEngineButton.setAttribute('aria-expanded', 'false');
    await window.quantic.setSetting('searchEngine', engine);
    address.focus();
  };
});

back.onclick = () => fire(window.quantic.back());
forward.onclick = () => fire(window.quantic.forward());
reload.onclick = () => fire(active()?.loading ? window.quantic.stop() : window.quantic.reload());
fav.onclick = () => fire(window.quantic.toggleFavorite());
$('#ai').onclick = () => fire(window.quantic.toggleAi());
$('#ai-close').onclick = () => fire(window.quantic.toggleAi());

document.querySelectorAll('[data-win]').forEach((button) => {
  button.onclick = () => fire(window.quantic.windowControl(button.dataset.win));
});

address.oninput = () => {
  if (!searchEngineMenu.classList.contains('hidden')) {
    searchEngineMenu.classList.add('hidden');
    searchEngineButton.setAttribute('aria-expanded', 'false');
  }
  updateSiteSuggestions();
};
address.onkeydown = (event) => {
  if (event.key === 'ArrowDown' && siteMatches.length) {
    event.preventDefault();
    siteSelection = (siteSelection + 1) % siteMatches.length;
    refreshSiteSelection();
    return;
  }
  if (event.key === 'ArrowUp' && siteMatches.length) {
    event.preventDefault();
    siteSelection = (siteSelection - 1 + siteMatches.length) % siteMatches.length;
    refreshSiteSelection();
    return;
  }
  if (event.key === 'Escape' && siteMatches.length) {
    event.preventDefault();
    closeSiteSuggestions();
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    navigateAddressValue();
  }
};
$('#address-go').onclick = () => navigateAddressValue();
address.onfocus = () => {
  fire(window.quantic.chromeLock(true));
  updateSiteSuggestions();
};
address.onblur = () => {
  closeSiteSuggestions();
  fire(window.quantic.chromeLock(false));
};

document.querySelectorAll('[data-q]').forEach((button) => {
  button.onclick = () => {
    address.value = button.dataset.q;
    address.focus();
    address.select();
  };
});

document.querySelectorAll('[data-home-action]').forEach((button) => {
  button.onclick = () => {
    const action = button.dataset.homeAction;
    if (action === 'private-search') {
      address.value = '';
      address.focus();
      address.select();
      return;
    }
    if (action === 'secure-tabs') {
      fire(window.quantic.newTab('quantic://settings'));
      return;
    }
    if (action === 'fast-light') {
      fire(window.quantic.navigate('quantic://settings'));
      return;
    }
    if (action === 'explore-more') {
      fire(window.quantic.navigate('Découvrir le web'));
    }
  };
});

window.quantic.onFocusAddress(() => { address.focus(); address.select(); });
window.quantic.onFocusHomeSearch(() => { address.focus(); address.select(); });

async function runAi(action, prompt = '') {
  if (aiBusy) return;
  aiBusy = true;
  aiOutput.textContent = 'Analyse en cours…';
  document.querySelectorAll('[data-ai]').forEach((button) => { button.disabled = true; });
  $('#ai-send').disabled = true;
  try {
    const result = await window.quantic.aiAction(action, prompt);
    aiOutput.textContent = result || 'Aucune réponse.';
  } catch {
    aiOutput.textContent = 'Quantic AI n’a pas pu exécuter cette action.';
  } finally {
    aiBusy = false;
    document.querySelectorAll('[data-ai]').forEach((button) => { button.disabled = false; });
    $('#ai-send').disabled = false;
  }
}

document.querySelectorAll('[data-ai]').forEach((button) => {
  button.onclick = () => runAi(button.dataset.ai);
});
$('#ai-send').onclick = () => {
  const prompt = aiPrompt.value.trim();
  if (!prompt) return;
  aiPrompt.value = '';
  runAi('chat', prompt);
};
aiPrompt.onkeydown = (event) => {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    $('#ai-send').click();
  }
};

document.addEventListener('pointerdown', (event) => {
  if (!searchEngineMenu || searchEngineMenu.classList.contains('hidden')) return;
  if (searchEngineMenu.contains(event.target) || searchEngineButton.contains(event.target)) return;
  searchEngineMenu.classList.add('hidden');
  searchEngineButton.setAttribute('aria-expanded', 'false');
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && appsPanel && !appsPanel.classList.contains('hidden')) {
    appsPanel.classList.add('hidden');
  }
  if (event.key === 'Escape' && searchEngineMenu && !searchEngineMenu.classList.contains('hidden')) {
    searchEngineMenu.classList.add('hidden');
    searchEngineButton.setAttribute('aria-expanded', 'false');
  }
  if (event.key === 'Escape' && siteSuggestions && !siteSuggestions.classList.contains('hidden')) {
    closeSiteSuggestions();
  }
});
