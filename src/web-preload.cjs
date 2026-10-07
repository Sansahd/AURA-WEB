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

try {
  if (typeof contextBridge?.executeInMainWorld === 'function') {
    contextBridge.executeInMainWorld({ func: installFingerprintGuard });
  } else {
    webFrame.executeJavaScript(`(${installFingerprintGuard.toString()})()`, true).catch(() => {});
  }
} catch {}
