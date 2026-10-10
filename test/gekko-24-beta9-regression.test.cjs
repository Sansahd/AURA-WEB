'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const source = file => fs.readFileSync(path.join(__dirname, '..', file), 'utf8');
const vpn = require('../src/services/vpn-system.cjs');

test('custom accent controls the entire bottom and side bars through linear gradients', () => {
  const css = source('src/renderer/gekko-24-layout.css');
  const last = css.slice(css.lastIndexOf('/* GEKKO 2.4 beta 9'));
  assert.match(last, /#bottom-dock\s*\{[\s\S]*?background:linear-gradient\(/);
  assert.match(last, /#sidestage-rail:not\(\.rail-minimized\)/);
  assert.match(last, /--quantic-accent/);
  assert.doesNotMatch(last, /radial-gradient\(/);
  assert.doesNotMatch(last, /filter:blur\(/);
});

test('settings, favorites and app launchers toggle instead of creating duplicate tabs', () => {
  const main = source('src/main.cjs'), ui = source('src/renderer/renderer.js');
  const preload = source('src/preload.cjs'), firewall = source('src/security/ipc-firewall.cjs');
  assert.match(main, /function togglePanelTab\(url\)/);
  assert.match(main, /existing\.id === activeId\) return closeTab\(existing\.id\)/);
  assert.match(main, /const activated = activateTab\(existing\.id\)/);
  assert.match(main, /createTab\(normalized, true\)/);
  assert.match(main, /allowedInternal/);
  assert.match(main, /allowedApps/);
  assert.match(ui, /window\.quantic\.togglePanelTab\('quantic:\/\/settings'\)/);
  assert.match(ui, /window\.quantic\.togglePanelTab\(button\.dataset\.appUrl\)/);
  assert.match(ui, /window\.quantic\.togglePanelTab\('quantic:\/\/favorites'\)/);
  assert.match(preload, /togglePanelTab:/);
  assert.match(firewall, /'toggle-panel-tab':/);
});

test('per-tab mute uses Electron audio API, state and accessible controls', () => {
  const main = source('src/main.cjs'), ui = source('src/renderer/renderer.js');
  const preload = source('src/preload.cjs'), firewall = source('src/security/ipc-firewall.cjs');
  const css = source('src/renderer/gekko-24-layout.css');
  assert.match(main, /function toggleTabMute\(id\)/);
  assert.match(main, /wc\.setAudioMuted\(tab\.muted\)/);
  assert.match(main, /wc\.setAudioMuted\(Boolean\(tab\.muted\)\)/);
  assert.match(main, /muted: Boolean\(tab\.muted\)/);
  assert.match(main, /muted: Boolean\(item\.muted\)/);
  assert.match(main, /audible: Boolean\(tab\.audible\)/);
  assert.match(ui, /window\.quantic\.toggleTabMute\(tab\.id\)/);
  assert.match(ui, /setAttribute\('aria-pressed', String\(Boolean\(tab\.muted\)\)\)/);
  assert.match(ui, /tab\.muted \? '🔇' : '🔊'/);
  assert.match(preload, /toggleTabMute:/);
  assert.match(firewall, /'toggle-tab-mute':/);
  assert.match(css, /\.tab\.muted \.tab-sound/);
});

test('Windows WireGuard parser detects named, running local tunnels', () => {
  const services = vpn.parseWireGuardServices(
    'SERVICE_NAME: WireGuardTunnel$Paris\n        STATE              : 4  RUNNING\n' +
    'SERVICE_NAME: WireGuardTunnel$Berlin\n        STATE              : 1  STOPPED\n');
  assert.equal(services.length, 2);
  assert.deepEqual(services[0], { name: 'Paris', running: true });
  assert.deepEqual(services[1], { name: 'Berlin', running: false });
});

test('VPN never auto-enrolls unknown public relays or claims local VPN on non-Windows', async () => {
  const s = await vpn.wireGuardStatus({ platform: 'linux' });
  assert.equal(s.available, false);
  assert.equal(s.active, false);
  const s2 = await vpn.wireGuardStatus({ platform: 'win32', executable: '',
    exec: () => { throw Error('must not spawn'); } });
  assert.equal(s2.reason, 'not_installed');
  const main = source('src/main.cjs'), ui = source('src/renderer/renderer.js');
  assert.match(main, /'vpn-status'/);
  assert.match(main, /'vpn-open'/);
  assert.match(ui, /Routage et DNS non encore vérifiés/);
  assert.match(ui, /GEKKO ne télécharge pas de relais VPN publics non vérifiés/);
});
