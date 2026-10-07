(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  if (root) root.GekkoSiteCache = api;
})(typeof window !== 'undefined' ? window : null, function () {
  'use strict';

  const SITES = Object.freeze([
    { name: 'Google', url: 'https://www.google.com/', aliases: ['google','goo'], tag: 'Recherche', rank: 100 },
    { name: 'YouTube', url: 'https://www.youtube.com/', aliases: ['youtube','you','yt'], tag: 'Vidéo', rank: 99 },
    { name: 'Facebook', url: 'https://www.facebook.com/', aliases: ['facebook','face','fb'], tag: 'Social', rank: 97 },
    { name: 'Instagram', url: 'https://www.instagram.com/', aliases: ['instagram','insta','ins'], tag: 'Social', rank: 96 },
    { name: 'X · Twitter', url: 'https://x.com/', aliases: ['twitter','twi','xcom'], tag: 'Social', rank: 94 },
    { name: 'WhatsApp Web', url: 'https://web.whatsapp.com/', aliases: ['whatsapp','what','wha'], tag: 'Messages', rank: 93 },
    { name: 'TikTok', url: 'https://www.tiktok.com/', aliases: ['tiktok','tik'], tag: 'Vidéo', rank: 92 },
    { name: 'Wikipedia', url: 'https://www.wikipedia.org/', aliases: ['wikipedia','wiki','wik'], tag: 'Encyclopédie', rank: 91 },
    { name: 'Amazon', url: 'https://www.amazon.com/', aliases: ['amazon','ama'], tag: 'Shopping', rank: 90 },
    { name: 'Reddit', url: 'https://www.reddit.com/', aliases: ['reddit','red'], tag: 'Communautés', rank: 89 },
    { name: 'LinkedIn', url: 'https://www.linkedin.com/', aliases: ['linkedin','link','lin'], tag: 'Professionnel', rank: 88 },
    { name: 'Netflix', url: 'https://www.netflix.com/', aliases: ['netflix','net'], tag: 'Streaming', rank: 87 },
    { name: 'Twitch', url: 'https://www.twitch.tv/', aliases: ['twitch','twt','twitchtv'], tag: 'Live', rank: 86 },
    { name: 'Spotify', url: 'https://open.spotify.com/', aliases: ['spotify','spo'], tag: 'Musique', rank: 85 },
    { name: 'Discord', url: 'https://discord.com/app', aliases: ['discord','dis'], tag: 'Communautés', rank: 84 },
    { name: 'GitHub', url: 'https://github.com/', aliases: ['github','git'], tag: 'Code', rank: 83 },
    { name: 'ChatGPT', url: 'https://chatgpt.com/', aliases: ['chatgpt','chat','gpt'], tag: 'IA', rank: 82 },
    { name: 'Bing', url: 'https://www.bing.com/', aliases: ['bing','bin'], tag: 'Recherche', rank: 81 },
    { name: 'Yahoo', url: 'https://www.yahoo.com/', aliases: ['yahoo','yah'], tag: 'Portail', rank: 80 },
    { name: 'Microsoft', url: 'https://www.microsoft.com/', aliases: ['microsoft','mic'], tag: 'Tech', rank: 79 },
    { name: 'Apple', url: 'https://www.apple.com/', aliases: ['apple','app'], tag: 'Tech', rank: 78 },
    { name: 'Pinterest', url: 'https://www.pinterest.com/', aliases: ['pinterest','pin'], tag: 'Images', rank: 77 },
    { name: 'eBay', url: 'https://www.ebay.com/', aliases: ['ebay','eba'], tag: 'Shopping', rank: 76 },
    { name: 'Booking.com', url: 'https://www.booking.com/', aliases: ['booking','boo'], tag: 'Voyage', rank: 75 },
    { name: 'Zoom', url: 'https://zoom.us/', aliases: ['zoom','zoo'], tag: 'Visio', rank: 74 },
    { name: 'Canva', url: 'https://www.canva.com/', aliases: ['canva','can'], tag: 'Création', rank: 73 },
    { name: 'PayPal', url: 'https://www.paypal.com/', aliases: ['paypal','pay'], tag: 'Paiement', rank: 72 },
    { name: 'Stack Overflow', url: 'https://stackoverflow.com/', aliases: ['stackoverflow','stack','sta'], tag: 'Développement', rank: 71 },
    { name: 'Steam', url: 'https://store.steampowered.com/', aliases: ['steam','ste'], tag: 'Jeux', rank: 70 },
    { name: 'Roblox', url: 'https://www.roblox.com/', aliases: ['roblox','rob'], tag: 'Jeux', rank: 69 },
    { name: 'IMDb', url: 'https://www.imdb.com/', aliases: ['imdb','imd'], tag: 'Cinéma', rank: 68 },
    { name: 'BBC', url: 'https://www.bbc.com/', aliases: ['bbc'], tag: 'Actualités', rank: 67 },
    { name: 'CNN', url: 'https://www.cnn.com/', aliases: ['cnn'], tag: 'Actualités', rank: 66 },
    { name: 'Gmail', url: 'https://mail.google.com/', aliases: ['gmail','gma'], tag: 'Mail', rank: 65 },
    { name: 'Google Drive', url: 'https://drive.google.com/', aliases: ['drive','dri','gdrive'], tag: 'Cloud', rank: 64 },
    { name: 'Google Maps', url: 'https://maps.google.com/', aliases: ['maps','map','gmaps'], tag: 'Cartes', rank: 63 },
    { name: 'Dropbox', url: 'https://www.dropbox.com/', aliases: ['dropbox','drop','dro'], tag: 'Cloud', rank: 62 },
    { name: 'Telegram Web', url: 'https://web.telegram.org/', aliases: ['telegram','tele','tel'], tag: 'Messages', rank: 61 },
    { name: 'Office', url: 'https://www.office.com/', aliases: ['office','off'], tag: 'Bureautique', rank: 60 },
    { name: 'OpenAI', url: 'https://openai.com/', aliases: ['openai','ope'], tag: 'IA', rank: 59 }
  ]);

  const normalize = (value) => String(value || '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .trim()
    .replace(/^www\./, '');

  function scoreSite(site, query) {
    const q = normalize(query);
    if (q.length < 3 || /[\s/:?&=#]/.test(q)) return -1;

    const name = normalize(site.name).replace(/[^a-z0-9]/g, '');
    const host = normalize(new URL(site.url).hostname).replace(/^www\./, '').replace(/[^a-z0-9]/g, '');
    let best = -1;

    for (const aliasRaw of site.aliases || []) {
      const alias = normalize(aliasRaw).replace(/[^a-z0-9]/g, '');
      if (!alias) continue;
      if (alias === q) best = Math.max(best, 1500);
      else if (alias.startsWith(q)) best = Math.max(best, 1200 - Math.min(120, alias.length - q.length));
      else if (q.length >= 4 && alias.includes(q)) best = Math.max(best, 760);
    }

    if (name === q) best = Math.max(best, 1450);
    else if (name.startsWith(q)) best = Math.max(best, 1120 - Math.min(100, name.length - q.length));

    if (host.startsWith(q)) best = Math.max(best, 1060 - Math.min(100, host.length - q.length));
    else if (q.length >= 4 && host.includes(q)) best = Math.max(best, 720);

    return best < 0 ? -1 : best + Number(site.rank || 0) / 100;
  }

  function matchSites(query, limit = 5) {
    const q = normalize(query);
    if (q.length < 3) return [];
    return SITES
      .map((site) => ({ ...site, score: scoreSite(site, q) }))
      .filter((site) => site.score >= 0)
      .sort((a, b) => b.score - a.score || b.rank - a.rank || a.name.localeCompare(b.name))
      .slice(0, Math.max(1, Math.min(8, Number(limit) || 5)));
  }

  const INSTANT_ALIASES = Object.freeze({
    goo: 'Google',
    you: 'YouTube',
    ins: 'Instagram',
    twi: 'X · Twitter',
    wha: 'WhatsApp Web',
    tik: 'TikTok',
    wiki: 'Wikipedia',
    ama: 'Amazon',
    red: 'Reddit',
    lin: 'LinkedIn',
    net: 'Netflix',
    spo: 'Spotify',
    dis: 'Discord',
    git: 'GitHub',
    gpt: 'ChatGPT',
    bin: 'Bing',
    yah: 'Yahoo',
    pin: 'Pinterest',
    eba: 'eBay',
    boo: 'Booking.com',
    zoo: 'Zoom',
    can: 'Canva',
    pay: 'PayPal',
    ste: 'Steam',
    rob: 'Roblox',
    imd: 'IMDb',
    gma: 'Gmail',
    dri: 'Google Drive',
    map: 'Google Maps',
    dro: 'Dropbox',
    tel: 'Telegram Web',
    off: 'Office',
    ope: 'OpenAI'
  });

  const siteByName = new Map(SITES.map((site) => [site.name, site]));

  function instantSite(query) {
    const q = normalize(query).replace(/[^a-z0-9]/g, '');
    if (!q || /[\s/:?&=#]/.test(normalize(query))) return null;
    const name = INSTANT_ALIASES[q];
    return name ? siteByName.get(name) || null : null;
  }

  function prewarmSite(query) {
    const q = normalize(query).replace(/[^a-z0-9]/g, '');
    if (q.length !== 2 || /[\s/:?&=#]/.test(normalize(query))) return null;
    const names = new Set(
      Object.entries(INSTANT_ALIASES)
        .filter(([alias]) => alias.startsWith(q))
        .map(([, name]) => name)
    );
    if (names.size !== 1) return null;
    return siteByName.get([...names][0]) || null;
  }

  function firstSite(query) {
    return matchSites(query, 1)[0] || null;
  }

  return { SITES, INSTANT_ALIASES, matchSites, firstSite, instantSite, prewarmSite };
});
