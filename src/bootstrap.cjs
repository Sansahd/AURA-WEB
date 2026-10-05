'use strict';

const { app, ipcMain, components, dialog } = require('electron');
const fs = require('node:fs');
const path = require('node:path');
const { installIpcFirewall } = require('./security/ipc-firewall.cjs');
const { registerQuanticUiScheme, SHELL_URL } = require('./services/ui-protocol.cjs');

registerQuanticUiScheme();
app.enableSandbox();
app.setName('Quantic Glide AURA Career');

function logPath() {
  try {
    const root = app.getPath('userData');
    fs.mkdirSync(root, { recursive: true });
    return path.join(root, 'startup.log');
  } catch {
    return path.join(process.env.TEMP || process.cwd(), 'Quantic-Glide-AURA-Career-startup.log');
  }
}

function writeLog(label, value = '') {
  const line = '[' + new Date().toISOString() + '] ' + label + ' ' + (typeof value === 'string' ? value : JSON.stringify(value)) + '\n';
  try { fs.appendFileSync(logPath(), line); } catch {}
  try { console.log(line.trim()); } catch {}
}

function fatal(title, error) {
  const detail = error?.stack || error?.message || String(error || 'Erreur inconnue');
  writeLog(title, detail);
  try {
    dialog.showErrorBox(
      'Quantic Glide AURA Career — erreur de démarrage',
      detail + '\n\nJournal : ' + logPath()
    );
  } catch {}
}

process.on('uncaughtException', (error) => {
  fatal('uncaughtException', error);
  try { app.quit(); } catch {}
});
process.on('unhandledRejection', (error) => {
  fatal('unhandledRejection', error);
  try { app.quit(); } catch {}
});

installIpcFirewall(ipcMain, { shellUrl: SHELL_URL });

async function prepareProtectedMedia() {
  if (!components?.whenReady) {
    writeLog('DRM', 'API Widevine indisponible; démarrage sans DRM.');
    return false;
  }
  const timeout = new Promise((_, reject) => {
    const timer = setTimeout(() => reject(new Error('Widevine component initialization timeout')), 12000);
    timer.unref?.();
  });
  try {
    await Promise.race([components.whenReady(), timeout]);
    const status = typeof components.status === 'function' ? components.status() : {};
    writeLog('DRM ready', status);
    return true;
  } catch (error) {
    writeLog('DRM warning', error?.message || error);
    return false;
  }
}

(async () => {
  writeLog('BOOT', 'exe=' + process.execPath);
  writeLog('BOOT', 'electron=' + (process.versions.electron || 'unknown') + ' chrome=' + (process.versions.chrome || 'unknown'));
  await app.whenReady();
  writeLog('BOOT', 'app ready');
  await prepareProtectedMedia();
  try {
    require('./main.cjs');
    writeLog('BOOT', 'main.cjs loaded');
  } catch (error) {
    fatal('main.cjs failed', error);
    app.quit();
  }
})().catch((error) => {
  fatal('bootstrap failed', error);
  app.quit();
});
