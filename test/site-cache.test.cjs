const test = require('node:test');
const assert = require('node:assert/strict');
const { SITES, matchSites, firstSite, instantSite, prewarmSite, prewarmSites } = require('../src/renderer/site-cache.js');

test('popular site cache is local, broad and deterministic', () => {
  assert.ok(Array.isArray(SITES));
  assert.ok(SITES.length >= 180);
  assert.equal(firstSite('you')?.name, 'YouTube');
  assert.equal(firstSite('goo')?.name, 'Google');
  assert.equal(firstSite('ama')?.name, 'Amazon France');
  assert.equal(firstSite('wik')?.name, 'Wikipedia');
  assert.equal(firstSite('git')?.name, 'GitHub');
  assert.equal(firstSite('spo')?.name, 'Spotify');
});

test('popular site cache activates from two characters', () => {
  assert.ok(matchSites('yo').some((site) => site.name === 'YouTube'));
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
  assert.ok(matches.every((site) => site.url.startsWith('https://')));
});


test('reserved aliases launch the requested world sites immediately', () => {
  assert.equal(instantSite('you')?.name, 'YouTube');
  assert.equal(instantSite('spo')?.name, 'Spotify');
  assert.equal(instantSite('wiki')?.name, 'Wikipedia');
  assert.equal(instantSite('net')?.name, 'Netflix');
  assert.equal(instantSite('ama')?.name, 'Amazon France');
  assert.equal(instantSite('goo')?.name, 'Google');
  assert.equal(instantSite('git')?.name, 'GitHub');
  assert.equal(instantSite('random'), null);
});

test('two-letter unique prefixes prewarm before the instant alias completes', () => {
  assert.equal(prewarmSite('yo')?.name, 'YouTube');
  assert.equal(prewarmSite('sp')?.name, 'Spotify');
  assert.equal(prewarmSite('wi')?.name, 'Wikipedia');
  assert.equal(prewarmSite('ne')?.name, 'Netflix');
  assert.equal(prewarmSite('am')?.name, 'Amazon France');
});


test('hypercache covers public services, retail, banking, health and transport', () => {
  const names = new Set(SITES.map((site) => site.name));
  for (const name of [
    'Service-Public.fr','Impots.gouv.fr','Ameli','CAF','France Travail',
    'SNCF Connect','RATP','Doctolib','Carrefour','E.Leclerc','Fnac',
    'BoursoBank','Orange','EDF','Leboncoin'
  ]) assert.equal(names.has(name), true, name);
});

test('hypercache preconnect candidates start from the first letter', () => {
  const y = prewarmSites('y', 3);
  assert.ok(y.some((site) => site.name === 'YouTube'));
  assert.ok(prewarmSites('a', 3).length >= 2);
  assert.ok(prewarmSites('s', 3).length >= 2);
});
