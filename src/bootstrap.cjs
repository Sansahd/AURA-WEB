'use strict';

const { app, ipcMain, components } = require('electron');
const { installIpcFirewall } = require('./security/ipc-firewall.cjs');
const { registerQuanticUiScheme, SHELL_URL } = require('./services/ui-protocol.cjs');

// Register the packaged UI scheme before app readiness, then force every
// renderer into Chromium's OS-level sandbox before any WebContents exist.
registerQuanticUiScheme();
app.enableSandbox();

// Only the exact packaged Quantic shell may call privileged IPC handlers.
installIpcFirewall(ipcMain, { shellUrl: SHELL_URL });

async function prepareProtectedMedia() {
  if (!components?.whenReady) {
    console.warn('[drm] Widevine component API unavailable; protected media will remain disabled.');
    return false;
  }

  const timeout = new Promise((_, reject) => {
    const timer = setTimeout(() => reject(new Error('Widevine component initialization timeout')), 20000);
    timer.unref?.();
  });

  try {
    await Promise.race([components.whenReady(), timeout]);
    const status = typeof components.status === 'function' ? components.status() : {};
    console.log('[drm] Widevine components ready:', JSON.stringify(status));
    return true;
  } catch (error) {
    console.error('[drm] Widevine initialization failed:', error?.message || error);
    return false;
  }
}

(async () => {
  await app.whenReady();
  await prepareProtectedMedia();
  require('./main.cjs');
})().catch((error) => {
  console.error('[bootstrap] Quantic startup failed:', error?.stack || error);
  app.quit();
});
