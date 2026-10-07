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
  protocol.handle(SCHEME, async (request) => {
    const filePath = safeUiPath(request.url);
    if (!filePath) return new Response('Forbidden', { status: 403 });

    try {
      if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
        return new Response('Not found', { status: 404 });
      }
      return net.fetch(pathToFileURL(filePath).toString());
    } catch (error) {
      console.error('[quantic-ui] resource load failed', filePath, error?.stack || error?.message || error);
      return new Response('Internal shell error', { status: 500 });
    }
  });
  handlerInstalled = true;
}

async function verifyQuanticUiShell() {
  const response = await net.fetch(SHELL_URL);
  if (!response.ok) throw new Error(`Quantic shell probe failed with HTTP ${response.status}`);
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
