'use strict';

const DEFAULT_CLOUD_URL = 'https://antiquewhite-dolphin-780448.hostingersite.com';
const HEARTBEAT_MS = 120_000;

function cleanCloudUrl(value) {
  const raw = String(value || DEFAULT_CLOUD_URL).trim().replace(/\/$/, '');
  const url = new URL(raw);
  const hostname = url.hostname.replace(/^\[|\]$/g, '');
  if (url.protocol !== 'https:' && !['127.0.0.1','localhost','::1'].includes(hostname)) {
    throw new Error('AURA Everywhere exige HTTPS hors boucle locale');
  }
  return url.origin;
}

class AuraEverywherePresence {
  constructor({
    fetchImpl = globalThis.fetch,
    baseUrl = process.env.QUANTIC_AURA_CLOUD_URL || process.env.AURA_CLOUD_URL || DEFAULT_CLOUD_URL,
    token = process.env.QUANTIC_AURA_CLOUD_TOKEN || process.env.AURA_CLOUD_TOKEN || '',
    version = '1.3.0',
    canSend = () => true,
  } = {}) {
    this.fetch = fetchImpl;
    this.token = String(token || '').trim();
    this.baseUrl = this.token ? cleanCloudUrl(baseUrl) : DEFAULT_CLOUD_URL;
    this.version = String(version || '1.3.0').slice(0,120);
    this.canSend = typeof canSend === 'function' ? canSend : () => true;
    this.timer = null;
  }

  get enabled() {
    return typeof this.fetch === 'function' && Boolean(this.token);
  }

  async post(path, body) {
    if (!this.enabled || !this.canSend()) return null;
    const response = await this.fetch(this.baseUrl + path, {
      method: 'POST',
      headers: {
        'content-type':'application/json',
        accept:'application/json',
        authorization:'Bearer ' + this.token,
        'user-agent':'Quantic-Glide/AURA-Everywhere-0.1',
      },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(8_000),
    });
    const text = await response.text();
    if (!response.ok) throw new Error('AURA Everywhere HTTP ' + response.status + ': ' + text.slice(0,240));
    try { return text ? JSON.parse(text) : {}; } catch { return {}; }
  }

  descriptor() {
    return {
      id:'quantic-glide',
      name:'Quantic Glide',
      version:this.version,
      repository:'XDSawyerLoL/Quantic-Browser',
      objective:'Perception Web et interface de navigation AURA.',
      criticality:0.92,
      state:'online',
      capabilities:['browser','web-context','tabs','research-context','downloads','privacy','aura-chat'],
      writable_by_aura:true,
      modification_policy:'branch-test-canary-promote',
      bridge_version:'aura-universal-bridge-v1',
      runtime:{
        privacy:'navigation-content-not-forwarded-by-heartbeat',
        local_first:true,
        platform:process.platform,
        arch:process.arch,
      },
    };
  }

  async register() {
    if (!this.enabled) return false;
    try {
      await this.post('/api/aura/products/register', this.descriptor());
      return true;
    } catch {
      return false;
    }
  }

  async heartbeat(state = 'online', detail = 'Quantic Glide actif.') {
    if (!this.enabled) return false;
    try {
      await this.post('/api/aura/products/quantic-glide/observe', {
        state,
        detail,
        metadata:{
          version:this.version,
          platform:process.platform,
          arch:process.arch,
          privacy:'no-url-or-tab-content-in-heartbeat',
        },
      });
      return true;
    } catch {
      return false;
    }
  }

  start() {
    if (!this.enabled || this.timer) return;
    void this.register();
    this.timer = setInterval(() => { void this.heartbeat(); }, HEARTBEAT_MS);
    this.timer.unref?.();
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    void this.heartbeat('offline','Arrêt propre de Quantic Glide.');
  }
}

module.exports = { AuraEverywherePresence, cleanCloudUrl, DEFAULT_CLOUD_URL, HEARTBEAT_MS };
