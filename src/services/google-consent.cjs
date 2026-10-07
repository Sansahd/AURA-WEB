'use strict';

function isGoogleConsentUrl(raw = '') {
  try {
    const host = new URL(raw).hostname.toLowerCase();
    return host === 'consent.google.com'
      || host === 'google.com'
      || host.endsWith('.google.com')
      || /^([a-z0-9-]+\.)?google\.[a-z.]{2,}$/i.test(host);
  } catch {
    return false;
  }
}

function googleConsentRefusalSource(waitMs = 0) {
  const wait = Math.max(0, Math.min(1500, Number(waitMs) || 0));
  return `(async () => {
    const KEY = '__gekkoGoogleConsentRejectV1';
    if (window[KEY]?.installed) {
      try { window[KEY].scan?.(); } catch {}
      return { installed: true, reused: true };
    }

    const rejectLabels = [
      'tout refuser',
      'refuser tout',
      'reject all',
      'reject everything',
      'alle ablehnen',
      'alles ablehnen',
      'rechazar todo',
      'rechazar todos',
      'rifiuta tutto',
      'rejeitar tudo',
      'odrzuć wszystko',
      'alles weigeren'
    ];

    const norm = (value) => String(value || '')
      .replace(/\\s+/g, ' ')
      .trim()
      .toLocaleLowerCase();

    const labelOf = (node) => norm([
      node?.innerText,
      node?.textContent,
      node?.getAttribute?.('aria-label'),
      node?.getAttribute?.('value')
    ].filter(Boolean).join(' '));

    const isReject = (label) => rejectLabels.some((needle) =>
      label === needle || label.startsWith(needle + ' ') || label.includes(' ' + needle + ' ')
    );

    const state = {
      installed: true,
      rejected: false,
      attempts: 0,
      scan: null,
      observer: null
    };

    const scan = () => {
      if (state.rejected) return true;
      state.attempts += 1;

      const nodes = [
        ...document.querySelectorAll('button, [role="button"], input[type="submit"], input[type="button"]')
      ];

      for (const node of nodes) {
        const label = labelOf(node);
        if (!label || !isReject(label)) continue;
        try {
          node.click();
          state.rejected = true;
          state.observer?.disconnect();
          return true;
        } catch {}
      }
      return false;
    };

    state.scan = scan;
    window[KEY] = state;

    scan();

    if (!state.rejected) {
      state.observer = new MutationObserver(() => scan());
      state.observer.observe(document.documentElement || document, {
        subtree: true,
        childList: true,
        attributes: true,
        attributeFilter: ['aria-label', 'value']
      });

      setTimeout(() => {
        try { state.observer?.disconnect(); } catch {}
      }, 12000);
    }

    if (state.rejected || ${wait} <= 0) {
      return { installed: true, rejected: state.rejected };
    }

    const deadline = Date.now() + ${wait};
    while (!state.rejected && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 40));
      scan();
    }

    return { installed: true, rejected: state.rejected };
  })()`;
}

async function installGoogleConsentRefusal(webContents) {
  if (!webContents || webContents.isDestroyed?.()) return { ok: false, reason: 'webcontents-unavailable' };
  const url = webContents.getURL?.() || '';
  if (!isGoogleConsentUrl(url)) return { ok: false, reason: 'not-google' };

  try {
    const host = (() => { try { return new URL(url).hostname.toLowerCase(); } catch { return ''; } })();
    const waitMs = host === 'consent.google.com' ? 900 : 0;
    const result = await webContents.executeJavaScript(googleConsentRefusalSource(waitMs), true);
    return { ok: true, ...(result || {}) };
  } catch (error) {
    return { ok: false, reason: String(error?.message || error) };
  }
}

module.exports = {
  isGoogleConsentUrl,
  googleConsentRefusalSource,
  installGoogleConsentRefusal
};
