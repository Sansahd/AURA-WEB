'use strict';

const DEFAULT_AURA_URL = 'http://127.0.0.1:8787';
const DEFAULT_OLLAMA_URL = 'http://127.0.0.1:11434';
const DEFAULT_OLLAMA_MODEL = 'gemma3:12b';

function cleanBaseUrl(value, fallback) {
  const raw = String(value || fallback || '').trim().replace(/\/$/, '');
  const parsed = new URL(raw);
  const loopback = new Set(['127.0.0.1', 'localhost', '::1']);
  if (!loopback.has(parsed.hostname)) {
    throw new Error('Quantic Glide refuse un moteur IA distant direct ; passe par AURA locale.');
  }
  if (!['http:', 'https:'].includes(parsed.protocol)) {
    throw new Error('Protocole IA local invalide');
  }
  return parsed.origin;
}

function cleanError(error) {
  return String(error?.message || error || 'erreur inconnue').slice(0, 300);
}

class QuanticAuraClient {
  constructor({
    fetchImpl = globalThis.fetch,
    auraUrl = process.env.QUANTIC_AURA_URL || DEFAULT_AURA_URL,
    ollamaUrl = process.env.QUANTIC_OLLAMA_URL || DEFAULT_OLLAMA_URL,
    ollamaModel = process.env.QUANTIC_OLLAMA_MODEL || DEFAULT_OLLAMA_MODEL,
  } = {}) {
    if (typeof fetchImpl !== 'function') throw new Error('fetch indisponible');
    this.fetch = fetchImpl;
    this.auraUrl = cleanBaseUrl(auraUrl, DEFAULT_AURA_URL);
    this.ollamaUrl = cleanBaseUrl(ollamaUrl, DEFAULT_OLLAMA_URL);
    this.ollamaModel = String(ollamaModel || DEFAULT_OLLAMA_MODEL).slice(0, 180);
  }

  async generate({
    prompt,
    system = '',
    taskRole = 'auto',
    maxTokens = 500,
    distributed = false,
    maxAgents = 3,
  } = {}) {
    const text = String(prompt || '').trim().slice(0, 60_000);
    if (!text) throw new Error('Prompt vide');
    const boundedTokens = Math.max(64, Math.min(Number(maxTokens || 500), 4000));

    let auraError = '';
    try {
      const result = await this.#requestAura({
        prompt: text,
        system,
        taskRole,
        maxTokens: boundedTokens,
        distributed,
        maxAgents,
      });
      return result;
    } catch (error) {
      auraError = cleanError(error);
    }

    try {
      const answer = await this.#requestOllama({
        prompt: text,
        system,
        maxTokens: boundedTokens,
      });
      return {
        answer,
        engine: 'ollama-direct',
        model: this.ollamaModel,
        distributed: false,
        moa: false,
        fallbackFrom: 'aura-2',
        auraError,
      };
    } catch (error) {
      throw new Error(
        `AURA 2.0 indisponible (${auraError || 'inconnue'}) ; Ollama direct indisponible (${cleanError(error)})`
      );
    }
  }

  async #requestAura({ prompt, system, taskRole, maxTokens, distributed, maxAgents }) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), distributed ? 130_000 : 75_000);
    try {
      const response = await this.fetch(`${this.auraUrl}/api/ai/generate`, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          'accept': 'application/json',
          'user-agent': 'Quantic-Glide/AURA-2',
        },
        body: JSON.stringify({
          prompt,
          system_instruction: String(system || '').slice(0, 12_000),
          task_role: String(taskRole || 'auto').slice(0, 80),
          max_tokens: maxTokens,
          distributed: Boolean(distributed),
          max_agents: Math.max(2, Math.min(Number(maxAgents || 3), 3)),
          source: 'quantic-glide',
        }),
        signal: controller.signal,
      });
      const text = await response.text();
      let data = {};
      try { data = text ? JSON.parse(text) : {}; } catch {}
      if (!response.ok) {
        throw new Error(`AURA HTTP ${response.status}: ${data.detail || data.error || text || 'échec'}`);
      }
      const answer = String(data.answer || '').trim();
      if (!answer) throw new Error('AURA a renvoyé une réponse vide');
      return {
        answer,
        engine: String(data.engine || 'aura-local'),
        model: String(data.model || ''),
        role: String(data.role || taskRole || 'auto'),
        distributed: Boolean(data.distributed),
        moa: Boolean(data.moa || data.engine === 'distributed-moa'),
        verification: data.verification || {},
        metrics: data.metrics || {},
        ensemble: data.ensemble || {},
        distributedError: String(data.distributed_error || ''),
      };
    } finally {
      clearTimeout(timer);
    }
  }

  async #requestOllama({ prompt, system, maxTokens }) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 45_000);
    try {
      const response = await this.fetch(`${this.ollamaUrl}/api/generate`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          model: this.ollamaModel,
          prompt,
          system: String(system || '').slice(0, 12_000),
          stream: false,
          options: { num_predict: Math.max(64, Math.min(maxTokens, 1200)) },
        }),
        signal: controller.signal,
      });
      if (!response.ok) throw new Error(`Ollama HTTP ${response.status}`);
      const data = await response.json();
      const answer = String(data.response || '').trim();
      if (!answer) throw new Error('Réponse Ollama vide');
      return answer;
    } finally {
      clearTimeout(timer);
    }
  }
}

module.exports = {
  QuanticAuraClient,
  cleanBaseUrl,
  DEFAULT_AURA_URL,
  DEFAULT_OLLAMA_URL,
};
