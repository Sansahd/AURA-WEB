'use strict';
const { spawn } = require('node:child_process');
const electron = require('electron');
const child = spawn(electron, ['.'], {
  cwd: require('node:path').join(__dirname, '..'),
  env: { ...process.env, GEKKO_LAYOUT_SMOKE: '1' },
  stdio: 'inherit',
  windowsHide: false
});
const timeout = setTimeout(() => {
  console.error('[GEKKO_LAYOUT_SMOKE] timed out after 45 seconds');
  child.kill();
  process.exitCode = 2;
}, 45000);
child.on('error', err => {
  clearTimeout(timeout);
  console.error('[GEKKO_LAYOUT_SMOKE] could not launch Electron:', err);
  process.exitCode = 2;
});
child.on('exit', (code, signal) => {
  clearTimeout(timeout);
  console.log('[GEKKO_LAYOUT_SMOKE] Electron exit', code, signal || '');
  process.exitCode = code === 0 ? 0 : 2;
});
