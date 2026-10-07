const test = require('node:test');
const assert = require('node:assert/strict');
const { SITES, matchSites, firstSite } = require('../src/renderer/site-cache.js');

test('popular site cache is local, broad and deterministic', () => {
  assert.ok(Array.isArray(SITES));
  assert.ok(SITES.length >= 35);
  assert.equal(firstSite('you')?.name, 'YouTube');
  assert.equal(firstSite('goo')?.name, 'Google');
  assert.equal(firstSite('ama')?.name, 'Amazon');
  assert.equal(firstSite('wik')?.name, 'Wikipedia');
  assert.equal(firstSite('git')?.name, 'GitHub');
  assert.equal(firstSite('spo')?.name, 'Spotify');
});

test('popular site cache activates from three characters', () => {
  assert.deepEqual(matchSites('yo'), []);
  assert.ok(matchSites('you').length >= 1);
  assert.ok(matchSites('inst').some((site) => site.name === 'Instagram'));
});

test('popular site cache avoids hijacking normal multi-word searches and URLs', () => {
  assert.deepEqual(matchSites('youtube musique'), []);
  assert.deepEqual(matchSites('https://youtube.com'), []);
  assert.deepEqual(matchSites('google.com'), []);
});

test('ambiguous prefixes remain ranked suggestions instead of a network lookup', () => {
  const matches = matchSites('twi', 5);
  assert.ok(matches.length >= 2);
  assert.ok(matches.some((site) => site.name === 'X · Twitter'));
  assert.ok(matches.some((site) => site.name === 'Twitch'));
  assert.ok(matches.every((site) => /^https:///.test(site.url)));
});
