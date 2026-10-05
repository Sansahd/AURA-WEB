'use strict';

const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const MAX_WALLPAPER_BYTES = 20 * 1024 * 1024;
const ALLOWED_EXTENSIONS = new Set(['.png', '.jpg', '.jpeg', '.webp', '.svg']);

const DEFAULT_APPEARANCE = Object.freeze({
  enabled: true,
  accent: '#7aa2ff',
  glassOpacity: 0.72,
  radius: 14,
  wallpaperMode: 'none',
  wallpaperFile: '',
  wallpaperPrompt: '',
  wallpaperVersion: 0
});

function clamp(value, min, max, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.min(max, Math.max(min, number)) : fallback;
}

function cleanHex(value, fallback = DEFAULT_APPEARANCE.accent) {
  const text = String(value || '').trim();
  return /^#[0-9a-f]{6}$/i.test(text) ? text.toLowerCase() : fallback;
}

function normalizeAppearance(value = {}, current = DEFAULT_APPEARANCE) {
  const source = value && typeof value === 'object' ? value : {};
  const base = current && typeof current === 'object' ? current : DEFAULT_APPEARANCE;
  const mode = ['none', 'generated', 'local'].includes(source.wallpaperMode) ? source.wallpaperMode : base.wallpaperMode;
  return {
    enabled: typeof source.enabled === 'boolean' ? source.enabled : base.enabled !== false,
    accent: cleanHex(source.accent, cleanHex(base.accent)),
    glassOpacity: clamp(source.glassOpacity, 0.25, 0.95, clamp(base.glassOpacity, 0.25, 0.95, DEFAULT_APPEARANCE.glassOpacity)),
    radius: Math.round(clamp(source.radius, 6, 28, clamp(base.radius, 6, 28, DEFAULT_APPEARANCE.radius))),
    wallpaperMode: mode,
    wallpaperFile: path.basename(String(source.wallpaperFile ?? base.wallpaperFile ?? '')).slice(0, 160),
    wallpaperPrompt: String(source.wallpaperPrompt ?? base.wallpaperPrompt ?? '').trim().slice(0, 512),
    wallpaperVersion: Math.max(0, Math.floor(Number(source.wallpaperVersion ?? base.wallpaperVersion ?? 0) || 0))
  };
}

function appearanceDir(app) {
  return path.join(app.getPath('userData'), 'Appearance');
}

function safeAppearancePath(app, fileName) {
  const base = path.basename(String(fileName || ''));
  if (!base || base !== fileName) return '';
  return path.join(appearanceDir(app), base);
}

function promptPalette(prompt) {
  const p = String(prompt || '').toLowerCase();
  const palettes = [
    { words: ['forest', 'forêt', 'nature', 'jungle'], colors: ['#071f17', '#0d5135', '#68d391', '#d9f99d'] },
    { words: ['ocean', 'mer', 'sea', 'water', 'eau'], colors: ['#041b2d', '#075985', '#22d3ee', '#bae6fd'] },
    { words: ['space', 'espace', 'galaxy', 'galaxie', 'cosmos'], colors: ['#090b22', '#312e81', '#7c3aed', '#d8b4fe'] },
    { words: ['sunset', 'coucher', 'orange', 'gold', 'doré'], colors: ['#2a1020', '#9a3412', '#fb923c', '#fde68a'] },
    { words: ['cyber', 'neon', 'néon', 'futur', 'sci-fi'], colors: ['#071226', '#1d4ed8', '#7c3aed', '#22d3ee'] },
    { words: ['red', 'rouge', 'fire', 'feu'], colors: ['#22070a', '#991b1b', '#ef4444', '#fdba74'] },
    { words: ['pink', 'rose', 'cherry', 'sakura'], colors: ['#260b1d', '#9d174d', '#f472b6', '#fce7f3'] },
    { words: ['minimal', 'white', 'blanc', 'clean'], colors: ['#111827', '#334155', '#cbd5e1', '#f8fafc'] }
  ];
  return palettes.find((palette) => palette.words.some((word) => p.includes(word)))?.colors || ['#07111f', '#163b63', '#5b8cff', '#9ad8ff'];
}

function numberFromHash(buffer, offset) {
  return buffer.readUInt32BE(offset % (buffer.length - 4));
}

function buildPromptSvg(prompt) {
  const cleanPrompt = String(prompt || '').trim().slice(0, 512) || 'Quantic';
  const hash = crypto.createHash('sha256').update(cleanPrompt, 'utf8').digest();
  const colors = promptPalette(cleanPrompt);
  const shapes = [];
  for (let i = 0; i < 14; i += 1) {
    const a = numberFromHash(hash, (i * 3) % 24);
    const b = numberFromHash(hash, (i * 5 + 2) % 24);
    const x = a % 1920;
    const y = b % 1080;
    const radius = 120 + ((a ^ b) % 430);
    const opacity = (0.08 + (((a >>> 8) % 26) / 100)).toFixed(2);
    const color = colors[i % colors.length];
    shapes.push(`<circle cx="${x}" cy="${y}" r="${radius}" fill="${color}" opacity="${opacity}"/>`);
  }
  const angle = numberFromHash(hash, 4) % 360;
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="0 0 1920 1080">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1" gradientTransform="rotate(${angle} .5 .5)">
      <stop offset="0" stop-color="${colors[0]}"/>
      <stop offset="0.48" stop-color="${colors[1]}"/>
      <stop offset="1" stop-color="${colors[2]}"/>
    </linearGradient>
    <filter id="blur"><feGaussianBlur stdDeviation="70"/></filter>
    <pattern id="grid" width="52" height="52" patternUnits="userSpaceOnUse">
      <path d="M 52 0 L 0 0 0 52" fill="none" stroke="#ffffff" stroke-opacity=".035" stroke-width="1"/>
    </pattern>
  </defs>
  <rect width="1920" height="1080" fill="url(#bg)"/>
  <g filter="url(#blur)">${shapes.join('')}</g>
  <rect width="1920" height="1080" fill="url(#grid)"/>
  <rect width="1920" height="1080" fill="#020617" opacity=".18"/>
</svg>`;
}

async function generatePromptWallpaper(app, prompt) {
  const cleanPrompt = String(prompt || '').trim().slice(0, 512);
  if (!cleanPrompt) throw new Error('Prompt vide');
  const dir = appearanceDir(app);
  await fs.promises.mkdir(dir, { recursive: true });
  const filename = 'quantic-generated-wallpaper.svg';
  await fs.promises.writeFile(path.join(dir, filename), buildPromptSvg(cleanPrompt), 'utf8');
  const colors = promptPalette(cleanPrompt);
  return {
    wallpaperMode: 'generated',
    wallpaperFile: filename,
    wallpaperPrompt: cleanPrompt,
    wallpaperVersion: Date.now(),
    accent: colors[2]
  };
}

async function importWallpaper(app, sourcePath) {
  const source = path.resolve(String(sourcePath || ''));
  const extension = path.extname(source).toLowerCase();
  if (!ALLOWED_EXTENSIONS.has(extension)) throw new Error('Format image non pris en charge');
  const stat = await fs.promises.stat(source);
  if (!stat.isFile() || stat.size <= 0 || stat.size > MAX_WALLPAPER_BYTES) throw new Error('Image invalide ou trop volumineuse');
  const dir = appearanceDir(app);
  await fs.promises.mkdir(dir, { recursive: true });
  const filename = `quantic-local-wallpaper${extension}`;
  await fs.promises.copyFile(source, path.join(dir, filename));
  return { wallpaperMode: 'local', wallpaperFile: filename, wallpaperPrompt: '', wallpaperVersion: Date.now() };
}

async function clearWallpaper(app) {
  const dir = appearanceDir(app);
  for (const name of ['quantic-generated-wallpaper.svg', 'quantic-local-wallpaper.png', 'quantic-local-wallpaper.jpg', 'quantic-local-wallpaper.jpeg', 'quantic-local-wallpaper.webp', 'quantic-local-wallpaper.svg']) {
    await fs.promises.rm(path.join(dir, name), { force: true }).catch(() => {});
  }
}

async function wallpaperDataUrl(app, appearance) {
  const normalized = normalizeAppearance(appearance);
  if (normalized.wallpaperMode === 'none' || !normalized.wallpaperFile) return '';
  const file = safeAppearancePath(app, normalized.wallpaperFile);
  if (!file) return '';
  try {
    const stat = await fs.promises.stat(file);
    if (!stat.isFile() || stat.size > MAX_WALLPAPER_BYTES) return '';
    const extension = path.extname(file).toLowerCase();
    const mime = extension === '.svg' ? 'image/svg+xml' : extension === '.png' ? 'image/png' : extension === '.webp' ? 'image/webp' : 'image/jpeg';
    const data = await fs.promises.readFile(file);
    return `data:${mime};base64,${data.toString('base64')}`;
  } catch {
    return '';
  }
}

module.exports = {
  DEFAULT_APPEARANCE,
  normalizeAppearance,
  generatePromptWallpaper,
  importWallpaper,
  clearWallpaper,
  wallpaperDataUrl,
  buildPromptSvg
};
