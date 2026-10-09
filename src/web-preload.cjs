'use strict';

const { contextBridge, webFrame } = require('electron');

function installFingerprintGuard() {
  'use strict';

  const defineGetter = (target, property, getter) => {
    try {
      Object.defineProperty(target, property, {
        configurable: true,
        enumerable: true,
        get: getter
      });
    } catch {}
  };

  const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : null;
  if (navProto) {
    defineGetter(navProto, 'globalPrivacyControl', () => true);
    defineGetter(navProto, 'doNotTrack', () => '1');
    defineGetter(navProto, 'hardwareConcurrency', () => 8);
    defineGetter(navProto, 'deviceMemory', () => 8);
    defineGetter(navProto, 'webdriver', () => false);
  }

  try {
    const uaData = navigator.userAgentData;
    const proto = uaData && Object.getPrototypeOf(uaData);
    const nativeHighEntropy = proto?.getHighEntropyValues;
    if (typeof nativeHighEntropy === 'function' && !proto.__gekkoFingerprintGuard) {
      Object.defineProperty(proto, '__gekkoFingerprintGuard', { value: true, configurable: true });
      Object.defineProperty(proto, 'getHighEntropyValues', {
        configurable: true,
        value: async function gekkoHighEntropy(hints = []) {
          const result = await nativeHighEntropy.call(this, hints);
          if (!result || typeof result !== 'object') return result;

          if ('model' in result) result.model = '';
          if ('platformVersion' in result) result.platformVersion = '10.0.0';
          if ('uaFullVersion' in result) {
            const major = String(result.uaFullVersion || '').split('.')[0] || '0';
            result.uaFullVersion = major + '.0.0.0';
          }
          if (Array.isArray(result.fullVersionList)) {
            result.fullVersionList = result.fullVersionList.map((entry) => {
              const major = String(entry?.version || '').split('.')[0] || '0';
              return { ...entry, version: major + '.0.0.0' };
            });
          }
          return result;
        }
      });
    }
  } catch {}

  const patchWebGL = (Ctor) => {
    const proto = Ctor?.prototype;
    if (!proto || proto.__gekkoFingerprintGuard) return;

    const nativeGetParameter = proto.getParameter;
    if (typeof nativeGetParameter !== 'function') return;

    try {
      Object.defineProperty(proto, '__gekkoFingerprintGuard', { value: true, configurable: true });
      Object.defineProperty(proto, 'getParameter', {
        configurable: true,
        value: function gekkoGetParameter(parameter) {
          // WEBGL_debug_renderer_info values. Standardise only the high-entropy
          // vendor/renderer pair; leave capabilities untouched for compatibility.
          if (parameter === 0x9245) return 'Google Inc. (Google)';
          if (parameter === 0x9246) return 'ANGLE (Generic GPU)';
          return nativeGetParameter.call(this, parameter);
        }
      });
    } catch {}
  };

  try { patchWebGL(globalThis.WebGLRenderingContext); } catch {}
  try { patchWebGL(globalThis.WebGL2RenderingContext); } catch {}

  try {
    Object.defineProperty(globalThis, '__gekkoPrivacyGuard', {
      configurable: false,
      enumerable: false,
      value: Object.freeze({
        gpc: true,
        dnt: true,
        hardwareBucket: 8,
        memoryBucket: 8,
        webglStandardized: true
      })
    });
  } catch {}
}

// Run only on YouTube, before its page scripts parse the initial player response.
// This complements the later DOM-ready guard; no work is done on other websites.
function installYouTubeEarlyAdFilter() {
  const host = String(location.hostname || '').toLowerCase();
  if (host !== 'youtube.com' && !host.endsWith('.youtube.com') &&
      host !== 'youtube-nocookie.com' && !host.endsWith('.youtube-nocookie.com')) return;

  const keys = ['adPlacements', 'playerAds', 'adSlots', 'adBreakParams', 'adParams'];
  const sanitize = (response) => {
    if (!response || typeof response !== 'object') return response;
    if (!response.streamingData && !response.playabilityStatus) return response;
    for (const key of keys) {
      try { delete response[key]; } catch {}
    }
    return response;
  };

  try {
    const nativeParse = JSON.parse;
    if (!JSON.__gekkoEarlyAdFilter) {
      Object.defineProperty(JSON, '__gekkoEarlyAdFilter', { value: true });
      JSON.parse = function gekkoEarlyPlayerParse(input, ...rest) {
        const data = nativeParse.call(this, input, ...rest);
        if (typeof input === 'string' && input.includes('"streamingData"') &&
            (input.includes('"adPlacements"') || input.includes('"playerAds"') || input.includes('"adSlots"'))) {
          sanitize(data);
        }
        return data;
      };
    }
  } catch {}

  try {
    let current = sanitize(window.ytInitialPlayerResponse);
    const descriptor = Object.getOwnPropertyDescriptor(window, 'ytInitialPlayerResponse');
    if (!descriptor || descriptor.configurable) {
      Object.defineProperty(window, 'ytInitialPlayerResponse', {
        configurable: true,
        enumerable: true,
        get() { return current; },
        set(value) { current = sanitize(value); }
      });
    }
  } catch {}
}

try {
  if (typeof contextBridge?.executeInMainWorld === 'function') {
    contextBridge.executeInMainWorld({ func: installFingerprintGuard });
    contextBridge.executeInMainWorld({ func: installYouTubeEarlyAdFilter });
  } else {
    webFrame.executeJavaScript(`(${installFingerprintGuard.toString()})()`, true).catch(() => {});
    webFrame.executeJavaScript(`(${installYouTubeEarlyAdFilter.toString()})()`, true).catch(() => {});
  }
} catch {}
