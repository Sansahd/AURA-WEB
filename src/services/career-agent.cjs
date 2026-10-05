const { BrowserWindow } = require('electron');
const fs = require('node:fs');
const path = require('node:path');

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function canonicalUrl(raw = '') {
  try {
    const url = new URL(raw);
    for (const key of ['utm_source','utm_medium','utm_campaign','utm_term','utm_content','trk','trackingId']) {
      url.searchParams.delete(key);
    }
    url.hash = '';
    return url.toString().replace(/\/$/, '');
  } catch {
    return String(raw).split('?')[0].replace(/\/$/, '');
  }
}

function readJson(file, fallback = null) {
  try { return JSON.parse(fs.readFileSync(file, 'utf8')); } catch { return fallback; }
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2));
}

function isHumanGate(text = '') {
  const value = String(text).toLowerCase();
  if (/captcha|verify you are human|vérifiez que vous êtes humain|cloudflare|security check/.test(value)) return 'captcha';
  if (/two-factor|2fa|verification code|code de sécurité|authentification à deux facteurs/.test(value)) return '2fa';
  return '';
}

function containsSensitiveQuestion(text = '') {
  const value = String(text).toLowerCase();
  return [
    'salaire actuel',
    'prétentions salariales',
    'salary expectation',
    'état de santé',
    'handicap',
    'religion',
    'origine ethnique',
    'casier judiciaire',
    'work authorization',
    'sponsorship',
    'déclaration sur l\'honneur',
  ].find((needle) => value.includes(needle)) || '';
}

class GlideCareerAgent {
  constructor({
    app,
    browserSession,
    auraClient = null,
    onEvent = () => {},
    profilePath = '',
    searchesPath = '',
    cvPath = '',
  } = {}) {
    this.app = app;
    this.browserSession = browserSession;
    this.auraClient = auraClient;
    this.onEvent = onEvent;
    this.window = null;
    this.running = false;
    this.stopRequested = false;
    this.lastError = '';
    this.lastRunAt = '';
    this.current = null;

    const root = path.join(app.getPath('userData'), 'CareerAgent');
    this.root = root;
    this.profilePath = profilePath || process.env.AURA_CAREER_PROFILE || path.join(root, 'profile.json');
    this.searchesPath = searchesPath || process.env.AURA_CAREER_SEARCHES || path.join(root, 'searches.json');
    this.cvPath = cvPath || process.env.AURA_CAREER_CV || path.join(root, 'cv.pdf');
    this.statePath = path.join(root, 'state.json');

    const bundledSearches = path.join(app.getAppPath(), 'career-agent', 'searches.json');
    if (!fs.existsSync(this.searchesPath) && fs.existsSync(bundledSearches)) {
      fs.mkdirSync(path.dirname(this.searchesPath), { recursive: true });
      fs.copyFileSync(bundledSearches, this.searchesPath);
    }

    this.state = readJson(this.statePath, {
      offers: [],
      applications: [],
      events: [],
      answers: {},
      settings: {},
    });
    this.state.settings = {
      autopilot: true,
      autoSubmit: false,
      minScore: 72,
      maxOffers: 5,
      maxDaily: 12,
      ...(this.state.settings || {}),
    };
    writeJson(this.statePath, this.state);
  }

  emit(type, payload = {}) {
    const event = { type, at: new Date().toISOString(), ...payload };
    this.state.events.unshift(event);
    this.state.events = this.state.events.slice(0, 500);
    writeJson(this.statePath, this.state);
    try { this.onEvent(event); } catch {}
  }

  folder() {
    fs.mkdirSync(this.root, { recursive: true });
    return this.root;
  }

  configStatus() {
    return {
      profile: fs.existsSync(this.profilePath),
      searches: fs.existsSync(this.searchesPath),
      cv: fs.existsSync(this.cvPath),
      profilePath: this.profilePath,
      searchesPath: this.searchesPath,
      cvPath: this.cvPath,
    };
  }

  settings() {
    return { ...this.state.settings };
  }

  setSettings(patch = {}) {
    const next = { ...this.state.settings };
    if (Object.prototype.hasOwnProperty.call(patch, 'autopilot')) next.autopilot = Boolean(patch.autopilot);
    if (Object.prototype.hasOwnProperty.call(patch, 'autoSubmit')) next.autoSubmit = Boolean(patch.autoSubmit);
    if (Object.prototype.hasOwnProperty.call(patch, 'minScore')) next.minScore = Math.max(0, Math.min(100, Number(patch.minScore) || 72));
    if (Object.prototype.hasOwnProperty.call(patch, 'maxOffers')) next.maxOffers = Math.max(1, Math.min(50, Number(patch.maxOffers) || 5));
    if (Object.prototype.hasOwnProperty.call(patch, 'maxDaily')) next.maxDaily = Math.max(1, Math.min(100, Number(patch.maxDaily) || 12));
    this.state.settings = next;
    writeJson(this.statePath, this.state);
    this.emit('settings_changed', { settings: next });
    return { ...next };
  }

  snapshot() {
    return {
      running: this.running,
      stopRequested: this.stopRequested,
      lastError: this.lastError,
      lastRunAt: this.lastRunAt,
      current: this.current,
      settings: this.settings(),
      config: this.configStatus(),
      offers: this.state.offers.slice(0, 100),
      applications: this.state.applications.slice(0, 100),
      events: this.state.events.slice(0, 100),
    };
  }

  ensureConfig() {
    const config = this.configStatus();
    const missing = Object.entries(config)
      .filter(([key, value]) => ['profile','searches','cv'].includes(key) && !value)
      .map(([key]) => key);
    if (missing.length) {
      throw new Error(`AURA Career configuration missing: ${missing.join(', ')}. Expected under ${this.root}`);
    }
    return {
      profile: readJson(this.profilePath),
      searches: readJson(this.searchesPath),
    };
  }

  ensureWindow() {
    if (this.window && !this.window.isDestroyed()) return this.window;
    this.window = new BrowserWindow({
      width: 1180,
      height: 820,
      show: false,
      backgroundColor: '#0b1220',
      webPreferences: {
        session: this.browserSession,
        nodeIntegration: false,
        contextIsolation: true,
        sandbox: true,
        webSecurity: true,
        javascript: true,
        spellcheck: false,
      },
    });
    this.window.on('closed', () => { this.window = null; });
    return this.window;
  }

  async load(url, timeoutMs = 45000) {
    const win = this.ensureWindow();
    const wc = win.webContents;
    const gate = new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        cleanup();
        reject(new Error(`navigation timeout: ${url}`));
      }, timeoutMs);
      const ok = () => { cleanup(); resolve(); };
      const fail = (_event, code, desc, validated, main) => {
        if (!main || code === -3) return;
        cleanup();
        reject(new Error(`load failed ${code} ${desc} ${validated}`));
      };
      const cleanup = () => {
        clearTimeout(timer);
        wc.removeListener('did-finish-load', ok);
        wc.removeListener('did-fail-load', fail);
      };
      wc.once('did-finish-load', ok);
      wc.on('did-fail-load', fail);
    });
    await wc.loadURL(url);
    await gate.catch((error) => {
      if (!wc.isLoading()) return;
      throw error;
    });
    await sleep(500);
    return wc;
  }

  async pageSnapshot() {
    const wc = this.ensureWindow().webContents;
    return wc.executeJavaScript(`(() => ({
      url: location.href,
      title: document.title || '',
      text: (document.body?.innerText || '').slice(0, 24000),
      links: [...document.querySelectorAll('a[href]')].slice(0, 5000).map(a => ({
        href: a.href || '',
        text: (a.innerText || a.getAttribute('aria-label') || '').trim().slice(0, 300)
      })),
      labels: [...document.querySelectorAll('label')].slice(0, 500).map(x => (x.innerText || '').trim()),
      hasPassword: !!document.querySelector('input[type="password"]'),
      hasFile: !!document.querySelector('input[type="file"]'),
    }))()`, true);
  }

  async resolveOllamaModel() {
    if (this.ollamaModel) return this.ollamaModel;
    const ollamaUrl = process.env.OLLAMA_URL || 'http://127.0.0.1:11434';
    let response;
    try {
      response = await fetch(`${ollamaUrl.replace(/\/$/, '')}/api/tags`);
    } catch {
      throw new Error('Ollama est hors ligne. Ouvre Ollama puis relance AURA Career.');
    }
    if (!response.ok) throw new Error(`Ollama /api/tags: ${response.status}`);
    const data = await response.json();
    const names = (data.models || []).map((item) => item.name).filter(Boolean);
    const requested = process.env.OLLAMA_MODEL || 'qwen3:4b';
    const preferred = [requested, 'qwen3:4b', 'qwen3:8b', 'qwen2.5:7b', 'llama3.2:3b', 'mistral:7b', 'gemma3:4b'];
    this.ollamaModel = preferred.find((name) => names.includes(name))
      || names.find((name) => /qwen|llama|mistral|gemma|deepseek/i.test(name) && !/embed/i.test(name));
    if (!this.ollamaModel) {
      throw new Error('Aucun modèle de chat Ollama installé. Installe qwen3:4b ou un autre modèle local de chat.');
    }
    this.emit('ollama_model', { model: this.ollamaModel });
    return this.ollamaModel;
  }

  async scoreOffer(profile, page) {
    const ollamaUrl = process.env.OLLAMA_URL || 'http://127.0.0.1:11434';
    const model = await this.resolveOllamaModel();
    const system = [
      'Tu es AURA Career, agent de candidature strict.',
      'Compare l’offre au profil réel.',
      'N’invente jamais expérience, diplôme, compétence, langue, salaire ou autorisation.',
      'Retourne uniquement un JSON valide avec score,title,company,location,contract,salary,reasons,blockers,coverLetter,fitSummary.',
      'Toute exigence obligatoire non démontrée doit aller dans blockers.'
    ].join(' ');
    const response = await fetch(`${ollamaUrl.replace(/\/$/, '')}/api/chat`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model,
        stream: false,
        format: 'json',
        messages: [
          { role: 'system', content: system },
          { role: 'user', content: JSON.stringify({ profile, url: page.url, offer: page.text }) }
        ],
        options: { temperature: 0.1 },
      }),
    });
    if (!response.ok) throw new Error(`Ollama ${response.status}: ${await response.text()}`);
    const data = await response.json();
    return JSON.parse(data.message.content);
  }

  async fillKnownFields(profile, analysis) {
    const wc = this.ensureWindow().webContents;
    const payload = {
      identity: profile.identity || {},
      coverLetter: analysis.coverLetter || '',
    };
    return wc.executeJavaScript(`(() => {
      const data = ${JSON.stringify(payload)};
      const full = String(data.identity.fullName || '').trim().split(/\\s+/);
      const first = full.shift() || '';
      const last = full.join(' ');
      const values = [
        [/first.?name|given.?name|pr[eé]nom/i, first],
        [/last.?name|family.?name|surname|nom de famille/i, last],
        [/e.?mail|courriel/i, data.identity.email || ''],
        [/phone|tel|t[eé]l[eé]phone/i, data.identity.phone || ''],
        [/location|city|ville|localisation/i, data.identity.location || ''],
      ];
      let filled = 0;
      for (const el of document.querySelectorAll('input, textarea')) {
        if (el.disabled || el.readOnly) continue;
        const type = String(el.type || '').toLowerCase();
        if (['password','file','checkbox','radio','submit','button','hidden'].includes(type)) continue;
        const sig = [el.name, el.id, el.placeholder, el.getAttribute('aria-label')].filter(Boolean).join(' ');
        let next = '';
        for (const [rx, value] of values) if (rx.test(sig)) { next = value; break; }
        if (!next && el.tagName === 'TEXTAREA' && /cover|motivation|lettre|message/i.test(sig)) next = data.coverLetter;
        if (!next || el.value) continue;
        const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value')?.set;
        if (setter) setter.call(el, next); else el.value = next;
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
        filled++;
      }
      return filled;
    })()`, true);
  }

  async attachCv() {
    if (!fs.existsSync(this.cvPath)) return false;
    const wc = this.ensureWindow().webContents;
    let attachedByUs = false;
    try {
      if (!wc.debugger.isAttached()) {
        wc.debugger.attach('1.3');
        attachedByUs = true;
      }
      const { root } = await wc.debugger.sendCommand('DOM.getDocument', { depth: -1, pierce: true });
      const { nodeIds } = await wc.debugger.sendCommand('DOM.querySelectorAll', {
        nodeId: root.nodeId,
        selector: 'input[type="file"]',
      });
      if (!nodeIds?.length) return false;
      for (const nodeId of nodeIds) {
        await wc.debugger.sendCommand('DOM.setFileInputFiles', {
          files: [path.resolve(this.cvPath)],
          nodeId,
        });
      }
      return true;
    } finally {
      if (attachedByUs && wc.debugger.isAttached()) {
        try { wc.debugger.detach(); } catch {}
      }
    }
  }

  async findText(patterns) {
    const wc = this.ensureWindow().webContents;
    const sources = patterns.map((rx) => ({ source: rx.source, flags: rx.flags }));
    return wc.executeJavaScript(`(() => {
      const patterns = ${JSON.stringify(sources)}.map(x => new RegExp(x.source, x.flags));
      const nodes = [...document.querySelectorAll('button, a, [role="button"], input[type="submit"]')];
      for (const el of nodes) {
        const label = (el.innerText || el.value || el.getAttribute('aria-label') || '').trim();
        if (!label || !patterns.some(rx => rx.test(label))) continue;
        const style = getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || el.disabled) continue;
        return label;
      }
      return '';
    })()`, true);
  }

  async clickText(patterns) {
    const wc = this.ensureWindow().webContents;
    const sources = patterns.map((rx) => ({ source: rx.source, flags: rx.flags }));
    return wc.executeJavaScript(`(() => {
      const patterns = ${JSON.stringify(sources)}.map(x => new RegExp(x.source, x.flags));
      const nodes = [...document.querySelectorAll('button, a, [role="button"], input[type="submit"]')];
      for (const el of nodes) {
        const label = (el.innerText || el.value || el.getAttribute('aria-label') || '').trim();
        if (!label || !patterns.some(rx => rx.test(label))) continue;
        const style = getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || el.disabled) continue;
        el.click();
        return label;
      }
      return '';
    })()`, true);
  }

  async handleApplication(profile, analysis, autoSubmit) {
    let page = await this.pageSnapshot();
    const gate = isHumanGate(page.text);
    if (gate) return { status: 'paused', reason: gate };
    if (page.hasPassword) return { status: 'needs_review', reason: 'login_required' };
    const sensitive = containsSensitiveQuestion(page.labels.join('\n') + '\n' + page.text);
    if (sensitive) return { status: 'needs_review', reason: `sensitive_question:${sensitive}` };

    await this.fillKnownFields(profile, analysis);
    const attached = page.hasFile ? await this.attachCv() : false;

    // Site entry buttons / multi-step flows.
    for (let step = 0; step < 8; step++) {
      page = await this.pageSnapshot();
      const postGate = isHumanGate(page.text);
      if (postGate) return { status: 'paused', reason: postGate };
      if (/application submitted|application sent|candidature envoy[eé]e|candidature transmise|merci pour votre candidature/i.test(page.text)) {
        return { status: 'submitted', reason: 'success_detected' };
      }

      await this.fillKnownFields(profile, analysis);
      if (page.hasFile && !attached) await this.attachCv();

      const submitPatterns = [
        /envoyer la candidature/i,
        /submit application/i,
        /send application/i,
        /valider ma candidature/i,
      ];
      const submitLabel = await this.findText(submitPatterns);
      if (submitLabel) {
        if (!autoSubmit) return { status: 'prepared', reason: 'ready_to_submit' };
        await this.clickText(submitPatterns);
        await sleep(1000);
        continue;
      }

      const nextLabel = await this.clickText([
        /candidature simplifi[eé]e/i,
        /easy apply/i,
        /postuler/i,
        /candidater/i,
        /^suivant$/i,
        /^next$/i,
        /continuer/i,
        /review/i,
        /v[eé]rifier/i,
      ]);
      if (!nextLabel) {
        return {
          status: 'prepared',
          reason: page.hasFile ? 'form_filled_cv_attached' : 'form_filled',
        };
      }
      await sleep(800);
    }
    return { status: 'needs_review', reason: 'max_form_steps' };
  }

  upsertOffer(offer) {
    const key = canonicalUrl(offer.url);
    const index = this.state.offers.findIndex((item) => canonicalUrl(item.url) === key);
    const record = { ...offer, updatedAt: new Date().toISOString() };
    if (index >= 0) this.state.offers[index] = { ...this.state.offers[index], ...record };
    else this.state.offers.unshift({ ...record, createdAt: new Date().toISOString() });
  }

  upsertApplication(application) {
    const key = canonicalUrl(application.url);
    const index = this.state.applications.findIndex((item) => canonicalUrl(item.url) === key);
    const now = new Date().toISOString();
    const previous = index >= 0 ? this.state.applications[index] : null;
    const record = {
      ...application,
      updatedAt: now,
      submittedAt: application.status === 'submitted'
        ? (application.submittedAt || previous?.submittedAt || now)
        : (application.submittedAt || previous?.submittedAt || null),
    };
    if (index >= 0) this.state.applications[index] = { ...this.state.applications[index], ...record };
    else this.state.applications.unshift({ ...record, createdAt: new Date().toISOString() });
    writeJson(this.statePath, this.state);
  }

  applicationsSubmittedToday() {
    const today = new Date().toISOString().slice(0, 10);
    return this.state.applications.filter((item) =>
      item.status === 'submitted' && String(item.submittedAt || '').slice(0, 10) === today
    ).length;
  }

  async runOnce(options = {}) {
    if (this.running) return { ok: false, error: 'already_running' };
    this.running = true;
    this.stopRequested = false;
    this.lastError = '';
    this.lastRunAt = new Date().toISOString();

    const settings = this.settings();
    const maxOffers = Number(options.maxOffers ?? settings.maxOffers ?? process.env.MAX_APPLICATIONS_PER_RUN ?? 5);
    const maxDaily = Number(options.maxDaily ?? settings.maxDaily ?? process.env.MAX_APPLICATIONS_PER_DAY ?? 12);
    const minScore = Number(options.minScore ?? settings.minScore ?? process.env.MIN_SCORE ?? 72);
    const autoSubmit = Boolean(options.autoSubmit ?? settings.autoSubmit ?? (String(process.env.AUTO_SUBMIT || 'false') === 'true'));

    try {
      const { profile, searches } = this.ensureConfig();
      const handled = [];
      const seen = new Set();

      for (const source of searches.sources || []) {
        if (this.stopRequested || handled.length >= maxOffers) break;
        this.current = { source: source.name, stage: 'discover' };
        this.emit('source_start', { source: source.name });

        await this.load(source.url);
        const searchPage = await this.pageSnapshot();
        const gate = isHumanGate(searchPage.text);
        if (gate) {
          this.emit('source_paused', { source: source.name, reason: gate });
          continue;
        }

        const urls = [...new Set(
          searchPage.links
            .map((link) => canonicalUrl(link.href))
            .filter((url) => (source.linkContains || []).some((needle) => url.includes(needle)))
        )].slice(0, 40);

        for (const url of urls) {
          if (this.stopRequested || handled.length >= maxOffers) break;
          if (seen.has(url)) continue;
          seen.add(url);
          if (this.state.applications.some((item) => canonicalUrl(item.url) === url && item.status === 'submitted')) {
            continue;
          }

          this.current = { source: source.name, stage: 'offer', url };
          await this.load(url);
          const page = await this.pageSnapshot();
          const humanGate = isHumanGate(page.text);
          if (humanGate) {
            this.upsertApplication({ source: source.name, url, status: 'paused', reason: humanGate });
            continue;
          }

          const analysis = await this.scoreOffer(profile, page);
          const offer = {
            source: source.name,
            url: page.url || url,
            title: analysis.title || page.title || '',
            company: analysis.company || '',
            score: Number(analysis.score || 0),
            blockers: analysis.blockers || [],
            fitSummary: analysis.fitSummary || '',
          };
          this.upsertOffer(offer);

          if (offer.score < minScore || offer.blockers.length) {
            this.upsertApplication({ ...offer, status: 'skipped', reason: offer.score < minScore ? 'score_below_threshold' : 'blockers' });
            continue;
          }

          const dailyLimitReached = this.applicationsSubmittedToday() >= maxDaily;
          const effectiveAutoSubmit = autoSubmit && !dailyLimitReached;
          const result = await this.handleApplication(profile, analysis, effectiveAutoSubmit);
          if (dailyLimitReached && result.status === 'prepared' && result.reason === 'ready_to_submit') {
            result.reason = 'daily_submit_limit';
          }
          this.upsertApplication({
            ...offer,
            status: result.status,
            reason: result.reason,
            coverLetter: analysis.coverLetter || '',
          });
          handled.push({ ...offer, ...result });
          this.emit('application_update', { url, score: offer.score, ...result });
        }
      }

      return { ok: true, handled };
    } catch (error) {
      this.lastError = String(error?.message || error);
      this.emit('fatal', { error: this.lastError });
      return { ok: false, error: this.lastError };
    } finally {
      this.current = null;
      this.running = false;
      writeJson(this.statePath, this.state);
    }
  }

  stop() {
    this.stopRequested = true;
    this.emit('stop_requested');
    return true;
  }

  destroy() {
    this.stopRequested = true;
    if (this.window && !this.window.isDestroyed()) this.window.destroy();
    this.window = null;
  }
}

module.exports = { GlideCareerAgent };
