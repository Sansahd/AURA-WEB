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
const {
  extractYouTubeVideoId,
  directRequestUrl,
  parseDirectRequest,
  isAllowedDirectStreamUrl,
  isDrmFormat,
  directPlayerSource
} = require('../src/services/youtube-direct.cjs');

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


test('GEKKO Direct extracts only valid YouTube video ids', () => {
  assert.equal(extractYouTubeVideoId('https://www.youtube.com/watch?v=dQw4w9WgXcQ'), 'dQw4w9WgXcQ');
  assert.equal(extractYouTubeVideoId('https://youtu.be/dQw4w9WgXcQ?t=4'), 'dQw4w9WgXcQ');
  assert.equal(extractYouTubeVideoId('https://www.youtube.com/shorts/dQw4w9WgXcQ'), 'dQw4w9WgXcQ');
  assert.equal(extractYouTubeVideoId('https://www.youtube.com/embed/dQw4w9WgXcQ'), 'dQw4w9WgXcQ');
  assert.equal(extractYouTubeVideoId('https://example.com/watch?v=dQw4w9WgXcQ'), '');
});

test('GEKKO Direct protocol round-trips a video id', () => {
  const request = directRequestUrl('dQw4w9WgXcQ');
  assert.equal(request, 'gekko-direct://youtube/dQw4w9WgXcQ');
  assert.equal(parseDirectRequest(request), 'dQw4w9WgXcQ');
  assert.equal(parseDirectRequest('https://www.youtube.com/watch?v=dQw4w9WgXcQ'), '');
  assert.equal(directRequestUrl('bad id'), '');
});

test('GEKKO Direct only accepts HTTPS googlevideo streams', () => {
  assert.equal(isAllowedDirectStreamUrl('https://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=abc'), true);
  assert.equal(isAllowedDirectStreamUrl('http://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=abc'), false);
  assert.equal(isAllowedDirectStreamUrl('https://googlevideo.com.evil.example/videoplayback?id=abc'), false);
  assert.equal(isAllowedDirectStreamUrl('https://example.com/video.mp4'), false);
});

test('GEKKO Direct refuses formats marked as DRM', () => {
  assert.equal(isDrmFormat({ is_drm: true }), true);
  assert.equal(isDrmFormat({ drm_families: ['widevine'] }), true);
  assert.equal(isDrmFormat({ mime_type: 'video/mp4' }), false);
});

test('GEKKO Direct player is reversible and preserves normal YouTube fallback', () => {
  const source = directPlayerSource({
    videoId: 'dQw4w9WgXcQ',
    url: 'https://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=abc',
    quality: '720p',
    mime: 'video/mp4'
  });
  assert.match(source, /GEKKO DIRECT/);
  assert.match(source, /Player YouTube/);
  assert.match(source, /nativeVideo\.currentTime/);
  assert.match(source, /video\.addEventListener\('error'/);
  assert.match(source, /destroy\(true\)/);
});

test('YouTube guard exposes manual Direct mode and automatic persistent-ad fallback', () => {
  const source = youtubeGuardSource();
  assert.match(source, /gekko-direct-button/);
  assert.match(source, /gekko-direct:\/\/youtube\//);
  assert.match(source, /Date\.now\(\) - state\.adSince >= 1800/);
  assert.match(source, /directResult/);
});
