'use strict';

const { app, protocol, net } = require('electron');
const path = require('node:path');
const fs = require('node:fs');
const { pathToFileURL } = require('node:url');

const SCHEME = 'quantic-ui';
const SHELL_URL = `${SCHEME}://app/renderer/index.html`;
let schemeRegistered = false;
let handlerInstalled = false;

function uiRoot() {
  return app.isPackaged
    ? path.join(process.resourcesPath, 'quantic-ui')
    : path.resolve(__dirname, '..');
}

function registerQuanticUiScheme() {
  if (schemeRegistered) return;
  protocol.registerSchemesAsPrivileged([{
    scheme: SCHEME,
    privileges: {
      standard: true,
      secure: true,
      bypassCSP: false,
      allowServiceWorkers: false,
      supportFetchAPI: true,
      corsEnabled: false,
      stream: true
    }
  }]);
  schemeRegistered = true;
}

function safeUiPath(requestUrl) {
  try {
    const url = new URL(requestUrl);
    if (url.protocol !== `${SCHEME}:` || url.hostname !== 'app') return '';

    const root = uiRoot();
    const relativeUrlPath = decodeURIComponent(url.pathname).replace(/^\/+/, '') || 'renderer/index.html';
    const candidate = path.resolve(root, relativeUrlPath);
    const relative = path.relative(root, candidate);

    if (!relative || relative === '.') return '';
    if (relative.startsWith('..') || path.isAbsolute(relative)) return '';
    return candidate;
  } catch {
    return '';
  }
}

async function installQuanticUiProtocol() {
  if (handlerInstalled) return;

  await protocol.handle(SCHEME, async (request) => {
    const filePath = safeUiPath(request.url);
    if (!filePath) return new Response('Forbidden', { status: 403 });

    try {
      if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
        console.error('[quantic-ui] resource missing', filePath);
        return new Response('Not found', { status: 404 });
      }

      // Electron recommends forwarding local resources through net.fetch(file://)
      // rather than manually building a Response from fs bytes. This also keeps
      // Chromium responsible for file MIME/range handling and behaves consistently
      // in packaged Windows builds.
      const response = await net.fetch(pathToFileURL(filePath).toString());

      if (!response.ok) {
        console.error('[quantic-ui] file fetch failed', response.status, filePath);
        return new Response('Not found', { status: 404 });
      }

      const headers = new Headers(response.headers);
      headers.set('cache-control', 'no-store');
      headers.set('x-content-type-options', 'nosniff');

      return new Response(response.body, {
        status: response.status,
        headers
      });
    } catch (error) {
      console.error('[quantic-ui] resource load failed', filePath, error?.stack || error?.message || error);
      return new Response('Internal shell error', { status: 500 });
    }
  });

  handlerInstalled = true;
}

async function verifyQuanticUiShell() {
  const response = await net.fetch(SHELL_URL);
  if (!response.ok) {
    throw new Error(`Quantic shell probe failed with HTTP ${response.status}`);
  }

  const html = await response.text();
  if (!/<title>GEKKO<\/title>/i.test(html) || !/id=["']bottom-dock["']/i.test(html)) {
    throw new Error('Quantic shell probe returned unexpected content');
  }
  return true;
}

module.exports = {
  SCHEME,
  SHELL_URL,
  registerQuanticUiScheme,
  installQuanticUiProtocol,
  verifyQuanticUiShell,
  safeUiPath,
  uiRoot
};
