'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const { youtubeGuardSource } = require('../src/services/youtube-guard.cjs');

test('YouTube Direct failure restores native video and permits manual retry', () => {
  let now = 10000;
  let playCalls = 0;
  const navigationRequests = [];
  const elements = [];
  let directButton = null;

  const element = (tag) => {
    const node = {
      tagName: tag,
      isConnected: false,
      attributes: {},
      style: { setProperty() {} },
      children: [],
      setAttribute(name, value) { this.attributes[name] = value; },
      getAttribute(name) { return this.attributes[name] || null; },
      removeAttribute(name) { delete this.attributes[name]; },
      append(...children) {
        this.children.push(...children);
        children.forEach((child) => { child.isConnected = true; });
      },
      prepend(child) { this.append(child); },
      remove() { this.isConnected = false; },
      addEventListener(name, callback) { this.listeners ||= {}; this.listeners[name] = callback; }
    };
    elements.push(node);
    return node;
  };
  const video = {
    isConnected: true, muted: false, volume: 0.8, playbackRate: 1,
    duration: 30, currentTime: 0,
    pause() { this.paused = true; },
    play() { playCalls += 1; this.paused = false; return Promise.resolve(); }
  };
  const player = element('div');
  player.classList = { contains(name) { return name === 'ad-showing'; } };
  player.querySelector = () => video;
  const controls = element('div');
  controls.prepend = (node) => { directButton = node; node.isConnected = true; };
  const document = {
    documentElement: element('html'),
    querySelector(selector) {
      if (selector === '#movie_player') return player;
      if (selector === '.ytp-right-controls') return controls;
      return null;
    },
    querySelectorAll() { return []; },
    createElement: element,
    getElementById(id) { return id === 'gekko-direct-button' ? directButton : null; }
  };
  const pageUrl = 'https://www.youtube.com/watch?v=abcdefghijk';
  const location = {
    get href() { return pageUrl; },
    set href(url) { navigationRequests.push(url); }
  };
  const sandbox = {
    document,
    location,
    URL,
    window: { addEventListener() {} },
    Date: class extends Date { static now() { return now; } },
    MutationObserver: class { observe() {} disconnect() {} },
    setInterval() { return 1; },
    clearInterval() {},
    setTimeout() { return 1; },
    queueMicrotask(fn) { fn(); }
  };

  vm.runInNewContext(youtubeGuardSource(), sandbox);
  assert.equal(navigationRequests.length, 1, 'Direct should be requested for a watch page');
  assert.ok(elements.some((node) => node.id === 'gekko-youtube-shield' && node.isConnected));

  sandbox.window.__gekkoYoutubeAdGuardV1.directResult({ ok: false, error: 'stream blocked' });
  now += 2000;
  sandbox.window.__gekkoYoutubeAdGuardV1.refresh();

  const shield = elements.find((node) => node.id === 'gekko-youtube-shield');
  assert.equal(shield.isConnected, false, 'Failed Direct must not obscure native playback');
  assert.ok(playCalls > 0, 'Native video must be allowed to resume');
  assert.ok(elements.some((node) => node.id === 'gekko-direct-status'), 'Failure should be visible');
  assert.equal(typeof directButton.listeners.click, 'function');

  directButton.listeners.click({ preventDefault() {}, stopPropagation() {} });
  assert.equal(navigationRequests.length, 2, 'Manual retry must override the cooldown');
});

test('YouTube URL detection remains restricted to legitimate domains', () => {
  const { isYouTubeUrl } = require('../src/services/youtube-guard.cjs');
  assert.equal(isYouTubeUrl('https://www.youtube.com/watch?v=abcdefghijk'), true);
  assert.equal(isYouTubeUrl('https://youtube.com.evil.example/watch'), false);
  assert.equal(isYouTubeUrl('https://example.com/'), false);
});
