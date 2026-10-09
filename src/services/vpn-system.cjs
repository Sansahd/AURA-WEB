'use strict';
/* Local WireGuard integration. This detects official Windows tunnel services,
 * and opens the installed app. It NEVER downloads or trusts random relay lists,
 * never stores private keys, and does not mislabel Tor or SOCKS as a full VPN. */
const fs = require('node:fs');
const path = require('node:path');
const { execFile, spawn } = require('node:child_process');
const { promisify } = require('node:util');
const execFileAsync = promisify(execFile);

function officialWireGuardPath(platform = process.platform, env = process.env) {
  if (platform !== 'win32') return '';
  const roots = [env.ProgramFiles, env['ProgramFiles(x86)'], env.ProgramW6432].filter(Boolean);
  for (const root of roots) {
    const file = path.join(root, 'WireGuard', 'wireguard.exe');
    if (fs.existsSync(file)) return file;
  }
  return '';
}

function parseWireGuardServices(output) {
  const blocks = String(output || '').split(/(?=(?:SERVICE_NAME|NOM_DU_SERVICE)\s*:)/i);
  return blocks.flatMap(block => {
    const match = block.match(/WireGuardTunnel\$([^\s\r\n]+)/i);
    if (!match) return [];
    // The service state number 4 is RUNNING regardless of UI language.
    const running = /(?:STATE|ÉTAT|ETAT)\s*:\s*4\b/im.test(block);
    return [{ name: match[1], running }];
  });
}

async function wireGuardStatus(options = {}) {
  const platform = options.platform || process.platform;
  const executable = options.executable === undefined ? officialWireGuardPath(platform) : options.executable;
  if (platform !== 'win32') return { available: false, active: false, tunnels: [], reason: 'windows_only' };
  if (!executable) return { available: false, active: false, tunnels: [], reason: 'not_installed' };
  try {
    const systemRoot = process.env.SystemRoot || 'C:\\Windows';
    const sc = path.join(systemRoot, 'System32', 'sc.exe');
    const exec = options.exec || execFileAsync;
    const { stdout } = await exec(sc, ['query', 'state=', 'all'], {
      timeout: 2200, maxBuffer: 4 * 1024 * 1024, windowsHide: true
    });
    const tunnels = parseWireGuardServices(stdout);
    return { available: true, active: tunnels.some(t => t.running), tunnels, reason: 'detected' };
  } catch {
    return { available: true, active: false, tunnels: [], reason: 'unknown' };
  }
}

function openWireGuard() {
  const executable = officialWireGuardPath();
  if (!executable) return { ok: false, reason: 'not_installed' };
  try {
    const child = spawn(executable, [], { detached: true, stdio: 'ignore', windowsHide: false });
    child.on('error', () => {}); // Avoid an unhandled child error if Windows blocks launch.
    child.unref();
    return { ok: true };
  } catch {
    return { ok: false, reason: 'launch_failed' };
  }
}

module.exports = { officialWireGuardPath, parseWireGuardServices, wireGuardStatus, openWireGuard };
