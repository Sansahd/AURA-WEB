const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const { spawn } = require('node:child_process');

const DEFAULT_SYSTEM_TOR = 'socks5://127.0.0.1:9050';
const DEFAULT_TOR_BROWSER = 'socks5://127.0.0.1:9150';

function sleep(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }

function parseProxy(raw) {
  const value = String(raw || '').trim();
  const normalized = value.includes('://') ? value : `socks5://${value}`;
  const url = new URL(normalized);
  if (url.protocol !== 'socks5:') throw new Error('Quantic Veil attend un relais SOCKS5.');
  return { url: `socks5://${url.hostname}:${Number(url.port || 1080)}`, host: url.hostname, port: Number(url.port || 1080) };
}

function checkTcpProxy(proxy, timeoutMs = 700) {
  let parsed;
  try { parsed = parseProxy(proxy); } catch { return Promise.resolve(false); }
  return new Promise((resolve) => {
    let done = false;
    const socket = net.createConnection({ host: parsed.host, port: parsed.port });
    const finish = (ok) => {
      if (done) return;
      done = true;
      try { socket.destroy(); } catch {}
      resolve(Boolean(ok));
    };
    socket.setTimeout(timeoutMs, () => finish(false));
    socket.once('error', () => finish(false));
    socket.once('connect', () => finish(true));
  });
}

function checkSocksRoute(proxy, timeoutMs = 2500, targetHost = 'example.com', targetPort = 443) {
  let parsed;
  try { parsed = parseProxy(proxy); } catch { return Promise.resolve(false); }
  const domain = Buffer.from(String(targetHost), 'utf8');
  return new Promise((resolve) => {
    let done = false;
    let phase = 0;
    let buffer = Buffer.alloc(0);
    const socket = net.createConnection({ host: parsed.host, port: parsed.port });
    const finish = (ok) => {
      if (done) return;
      done = true;
      try { socket.destroy(); } catch {}
      resolve(Boolean(ok));
    };
    socket.setTimeout(timeoutMs, () => finish(false));
    socket.on('error', () => finish(false));
    socket.on('connect', () => socket.write(Buffer.from([0x05, 0x01, 0x00])));
    socket.on('data', (chunk) => {
      buffer = Buffer.concat([buffer, chunk]);
      if (phase === 0 && buffer.length >= 2) {
        if (buffer[0] !== 0x05 || buffer[1] !== 0x00) return finish(false);
        buffer = buffer.subarray(2);
        phase = 1;
        const port = Number(targetPort);
        socket.write(Buffer.concat([
          Buffer.from([0x05, 0x01, 0x00, 0x03, domain.length]),
          domain,
          Buffer.from([(port >> 8) & 0xff, port & 0xff])
        ]));
      }
      if (phase === 1 && buffer.length >= 2) finish(buffer[0] === 0x05 && buffer[1] === 0x00);
    });
  });
}

function findTorBinary(root, depth = 4) {
  if (!root || depth < 0) return '';
  try {
    if (!fs.existsSync(root)) return '';
    const stat = fs.statSync(root);
    if (stat.isFile()) return /^tor(?:\.exe)?$/i.test(path.basename(root)) ? root : '';
    for (const entry of fs.readdirSync(root)) {
      const p = path.join(root, entry);
      const st = fs.statSync(p);
      if (st.isFile() && /^tor(?:\.exe)?$/i.test(entry)) return p;
      if (st.isDirectory()) {
        const found = findTorBinary(p, depth - 1);
        if (found) return found;
      }
    }
  } catch {}
  return '';
}

function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      const port = address && typeof address === 'object' ? address.port : 0;
      server.close(() => port ? resolve(port) : reject(new Error('Port local indisponible')));
    });
  });
}

class QuanticVeil {
  constructor(app) {
    this.app = app;
    this.connected = false;
    this.status = 'initialisation';
    this.proxy = '';
    this.transport = 'Quantic Veil';
    this.error = '';
    this.process = null;
    this.healthTimer = null;
    this.healthBusy = false;
    this.ensurePromise = null;
  }

  snapshot() {
    return { connected: this.connected, status: this.status, proxy: this.proxy, transport: this.transport, error: this.error };
  }

  invalidate(message = 'Le relais privé ne répond plus.') {
    this.connected = false;
    this.status = 'indisponible';
    this.error = message;
  }

  async tryProxy(proxy, transport) {
    if (!(await checkSocksRoute(proxy, 2500))) return false;
    this.proxy = parseProxy(proxy).url;
    this.transport = transport;
    this.connected = true;
    this.status = 'prêt';
    this.error = '';
    return true;
  }

  torCandidates() {
    const list = [];
    if (process.env.QUANTIC_TOR_PATH) list.push(process.env.QUANTIC_TOR_PATH);
    list.push(path.join(__dirname, '..', '..', 'vendor', 'tor'));
    if (process.resourcesPath) list.push(path.join(process.resourcesPath, 'veil', 'tor'));
    if (process.platform === 'win32') {
      const pf = process.env.ProgramFiles || 'C:\\Program Files';
      const pfx86 = process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)';
      const local = process.env.LOCALAPPDATA || '';
      list.push(
        path.join(pf, 'Tor Browser', 'Browser', 'TorBrowser', 'Tor'),
        path.join(pfx86, 'Tor Browser', 'Browser', 'TorBrowser', 'Tor'),
        path.join(local, 'Programs', 'Tor Browser', 'Browser', 'TorBrowser', 'Tor')
      );
    }
    return list;
  }

  locateTor() {
    for (const candidate of this.torCandidates()) {
      const found = findTorBinary(candidate);
      if (found) return found;
    }
    return '';
  }

  async startManagedTor() {
    const tor = this.locateTor();
    if (!tor) return false;
    const socksPort = await freePort();
    const dataDir = path.join(this.app.getPath('userData'), 'veil', 'tor-data');
    fs.mkdirSync(dataDir, { recursive: true });
    const proxy = `socks5://127.0.0.1:${socksPort}`;
    this.status = 'connexion au réseau privé…';
    this.transport = 'Tor intégré';

    this.process = spawn(tor, [
      '--SocksPort', `127.0.0.1:${socksPort}`,
      '--DataDirectory', dataDir,
      '--AvoidDiskWrites', '1',
      '--ClientOnly', '1',
      '--Log', 'notice stdout'
    ], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });

    let lastLog = '';
    let bootstrapped = false;
    let resolveBootstrap;
    const bootstrapPromise = new Promise((resolve) => { resolveBootstrap = resolve; });
    const capture = (chunk) => {
      const text = String(chunk || '');
      lastLog = text.trim().slice(-700) || lastLog;
      if (!bootstrapped && /Bootstrapped 100%/i.test(text)) {
        bootstrapped = true;
        resolveBootstrap(true);
      }
    };
    this.process.stdout?.on('data', capture);
    this.process.stderr?.on('data', capture);
    this.process.once('exit', () => {
      if (!bootstrapped) resolveBootstrap(false);
      this.process = null;
      this.invalidate('Le transport privé s’est arrêté.');
    });

    const ready = await Promise.race([bootstrapPromise, sleep(65000).then(() => false)]);
    if (!ready || !this.process) {
      this.error = lastLog || 'Tor n’a pas terminé son démarrage.';
      return false;
    }

    // One real SOCKS CONNECT validates the route. We no longer poll the Internet
    // every 800 ms during bootstrap, which was a major source of startup latency.
    if (!(await checkSocksRoute(proxy, 3000))) {
      this.error = lastLog || 'Le circuit privé ne répond pas.';
      return false;
    }

    this.proxy = proxy;
    this.connected = true;
    this.status = 'prêt';
    this.error = '';
    return true;
  }

  async ensure() {
    if (this.connected && this.proxy) return true;
    if (this.ensurePromise) return this.ensurePromise;
    this.ensurePromise = this._ensure().finally(() => { this.ensurePromise = null; });
    return this.ensurePromise;
  }

  async _ensure() {
    this.connected = false;
    this.status = 'vérification…';
    this.error = '';

    const configured = String(process.env.QUANTIC_VEIL_PROXY || '').trim();
    if (configured && await this.tryProxy(configured, 'Relais Quantic configuré')) return this.startHealth();

    if (this.process) {
      try { this.process.kill(); } catch {}
      this.process = null;
      await sleep(120);
    }

    if (await this.startManagedTor()) return this.startHealth();
    if (await this.tryProxy(DEFAULT_SYSTEM_TOR, 'Tor système')) return this.startHealth();
    if (await this.tryProxy(DEFAULT_TOR_BROWSER, 'Tor Browser')) return this.startHealth();

    this.connected = false;
    this.status = 'indisponible';
    this.error = this.error || 'Aucun relais privé utilisable. Exécutez npm run fetch:tor ou configurez QUANTIC_VEIL_PROXY.';
    return false;
  }

  startHealth() {
    clearInterval(this.healthTimer);
    this.healthTimer = setInterval(async () => {
      if (!this.proxy || this.healthBusy) return;
      this.healthBusy = true;
      try {
        // A cheap TCP health check is enough here. Real page navigation will detect
        // route failures; periodic full Internet CONNECT checks were unnecessary.
        const ok = await checkTcpProxy(this.proxy, 700);
        this.connected = ok;
        this.status = ok ? 'prêt' : 'indisponible';
        if (!ok) this.error = 'Le relais privé ne répond plus.';
      } finally {
        this.healthBusy = false;
      }
    }, 45000);
    this.healthTimer.unref?.();
    return true;
  }

  stop() {
    clearInterval(this.healthTimer);
    try { this.process?.kill(); } catch {}
    this.process = null;
  }
}

module.exports = { QuanticVeil, checkSocksRoute, checkTcpProxy };
