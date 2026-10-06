const test = require('node:test');
const assert = require('node:assert/strict');

const {
  shouldBlock,
  isYouTubeAdRequest,
  isYouTubeHost,
  isYouTubeMediaHost
} = require('../src/services/privacy.cjs');
const {
  isYouTubeUrl,
  youtubeGuardSource
} = require('../src/services/youtube-guard.cjs');

test('YouTube hosts are detected narrowly', () => {
  assert.equal(isYouTubeHost('www.youtube.com'), true);
  assert.equal(isYouTubeHost('music.youtube.com'), true);
  assert.equal(isYouTubeHost('youtube.evil.example'), false);
  assert.equal(isYouTubeUrl('https://youtu.be/abc'), true);
  assert.equal(isYouTubeUrl('https://example.com/watch?v=abc'), false);
});

test('YouTube media delivery is never treated as an ad endpoint', () => {
  assert.equal(isYouTubeMediaHost('rr1---sn-ab5l6n7z.googlevideo.com'), true);
  assert.equal(
    shouldBlock(
      'https://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=abc',
      'https://www.youtube.com/watch?v=abc'
    ),
    false
  );
  assert.equal(
    shouldBlock(
      'https://i.ytimg.com/vi/abc/hqdefault.jpg',
      'https://www.youtube.com/watch?v=abc'
    ),
    false
  );
});

test('dedicated Google ad infrastructure is blocked for YouTube', () => {
  for (const url of [
    'https://googleads.g.doubleclick.net/pagead/id',
    'https://www.googleadservices.com/pagead/conversion/',
    'https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js'
  ]) {
    assert.equal(shouldBlock(url, 'https://www.youtube.com/watch?v=abc'), true, url);
  }
});

test('explicit YouTube ad endpoints are blocked', () => {
  for (const url of [
    'https://www.youtube.com/pagead/paralleladview?ai=x',
    'https://www.youtube.com/api/stats/ads?ver=2',
    'https://www.youtube.com/ptracking?ei=123'
  ]) {
    const parsed = new URL(url);
    assert.equal(isYouTubeAdRequest(parsed, 'https://www.youtube.com/watch?v=abc'), true, url);
    assert.equal(shouldBlock(url, 'https://www.youtube.com/watch?v=abc'), true, url);
  }
});

test('normal YouTube playback and API routes stay allowed', () => {
  for (const url of [
    'https://www.youtube.com/watch?v=abc',
    'https://www.youtube.com/youtubei/v1/player?prettyPrint=false',
    'https://www.youtube.com/youtubei/v1/next?prettyPrint=false',
    'https://www.youtube.com/api/stats/watchtime?docid=abc'
  ]) {
    assert.equal(shouldBlock(url, 'https://www.youtube.com/watch?v=abc'), false, url);
  }
});

test('player guard scrubs player ad structures and includes a playback fallback', () => {
  const source = youtubeGuardSource();
  assert.match(source, /adPlacements/);
  assert.match(source, /playerAds/);
  assert.match(source, /adSlots/);
  assert.match(source, /ytInitialPlayerResponse/);
  assert.match(source, /Response\.prototype\.json/);
  assert.match(source, /ad-showing/);
  assert.match(source, /ytp-ad-skip-button/);
  assert.match(source, /playbackRate = 16/);
  assert.match(source, /video\.currentTime/);
});
