'use strict';

const { protocol } = require('electron');
const path = require('node:path');
const fs = require('node:fs');

const SCHEME = 'quantic-ui';
const SHELL_URL = `${SCHEME}://app/renderer/index.html`;
const UI_ROOT = path.resolve(__dirname, '..');
let schemeRegistered = false;
let handlerInstalled = false;

const MIME = Object.freeze({
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.cjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon'
});

function registerQuanticUiScheme() {
  if (schemeRegistered) return;
  protocol.registerSchemesAsPrivileged([{
    scheme: SCHEME,
    privileges: {
      standard: true,
      secure: true,
      bypassCSP: false,
      allowServiceWorkers: false,
      supportFetchAPI: false,
      corsEnabled: false,
      stream: true
    }
  }]);
  schemeRegistered = true;
}

function safeUiPath(requestUrl) {
  const url = new URL(requestUrl);
  if (url.protocol !== `${SCHEME}:` || url.hostname !== 'app') return '';
  const relative = decodeURIComponent(url.pathname).replace(/^\/+/, '');
  const candidate = path.resolve(UI_ROOT, relative || 'renderer/index.html');
  const rootPrefix = `${UI_ROOT}${path.sep}`.toLowerCase();
  const normalized = candidate.toLowerCase();
  if (candidate !== UI_ROOT && !normalized.startsWith(rootPrefix)) return '';
  return candidate;
}

function installQuanticUiProtocol() {
  if (handlerInstalled) return;
  protocol.handle(SCHEME, (request) => {
    const filePath = safeUiPath(request.url);
    if (!filePath) return new Response('Forbidden', { status: 403 });
    try {
      const body = fs.readFileSync(filePath);
      const type = MIME[path.extname(filePath).toLowerCase()] || 'application/octet-stream';
      return new Response(body, {
        status: 200,
        headers: {
          'content-type': type,
          'cache-control': 'no-store',
          'x-content-type-options': 'nosniff'
        }
      });
    } catch (error) {
      console.error('[quantic-ui] resource load failed', filePath, error?.code || error?.message || error);
      return new Response('Not found', { status: 404 });
    }
  });
  handlerInstalled = true;
}

module.exports = {
  SCHEME,
  SHELL_URL,
  UI_ROOT,
  registerQuanticUiScheme,
  installQuanticUiProtocol,
  safeUiPath
};
