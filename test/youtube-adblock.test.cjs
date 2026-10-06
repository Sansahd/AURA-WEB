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
  MAX_DIRECT_HEIGHT,
  selectAdaptiveFormats,
  streamPayloadIsAllowed,
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

test('player guard scrubs ad structures and exposes Direct fallback', () => {
  const source = youtubeGuardSource();
  assert.match(source, /adPlacements/);
  assert.match(source, /playerAds/);
  assert.match(source, /adSlots/);
  assert.match(source, /ytInitialPlayerResponse/);
  assert.match(source, /Response\.prototype\.json/);
  assert.match(source, /ad-showing/);
  assert.match(source, /ytp-ad-skip-button/);
  assert.match(source, /playbackRate = 16/);
  assert.match(source, /gekko-direct-button/);
  assert.match(source, /gekko-direct:\/\/youtube\//);
  assert.match(source, /Date\.now\(\) - state\.adSince >= 1800/);
  assert.match(source, /__gekkoDirectPlayerV2/);
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
  assert.equal(isDrmFormat({ drm_track_type: 'WIDEVINE' }), true);
  assert.equal(isDrmFormat({ mime_type: 'video/mp4' }), false);
});

test('GEKKO Direct adaptive selector prefers 4K high-fps video and default original audio', () => {
  const formats = [
    {
      has_video: true, has_audio: false, height: 1080, width: 1920, fps: 60,
      bitrate: 8_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '1080p60'
    },
    {
      has_video: true, has_audio: false, height: 1440, width: 2560, fps: 60,
      bitrate: 14_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '1440p60'
    },
    {
      has_video: true, has_audio: false, height: 2160, width: 3840, fps: 30,
      bitrate: 18_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '2160p'
    },
    {
      has_video: true, has_audio: false, height: 2160, width: 3840, fps: 60,
      bitrate: 24_000_000, mime_type: 'video/webm; codecs="av01.0.12M.08"', quality_label: '2160p60'
    },
    {
      has_video: true, has_audio: false, height: 4320, width: 7680, fps: 60,
      bitrate: 60_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '4320p60'
    },
    {
      has_video: true, has_audio: false, height: 2160, width: 3840, fps: 120,
      bitrate: 40_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '2160p120',
      drm_families: ['widevine']
    },
    {
      has_video: false, has_audio: true, bitrate: 128_000, audio_channels: 2,
      mime_type: 'audio/webm; codecs="opus"', is_original: true
    },
    {
      has_video: false, has_audio: true, bitrate: 160_000, audio_channels: 2,
      mime_type: 'audio/webm; codecs="opus"', is_original: true,
      audio_track: { audio_is_default: true }
    }
  ];

  const selected = selectAdaptiveFormats(formats);
  assert.equal(MAX_DIRECT_HEIGHT, 2160);
  assert.equal(selected.video[0].height, 2160);
  assert.equal(selected.video[0].fps, 60);
  assert.match(selected.video[0].mime_type, /av01/);
  assert.equal(selected.video.some((format) => format.height > 2160), false);
  assert.equal(selected.video.some((format) => format.drm_families?.length), false);
  assert.equal(selected.audio[0].bitrate, 160_000);
  assert.equal(selected.audio[0].audio_track.audio_is_default, true);
});

test('GEKKO Direct adaptive payload validates both tracks and progressive fallback origin', () => {
  const good = {
    mode: 'adaptive',
    progressiveFallback: {
      url: 'https://rr3---sn-c.googlevideo.com/videoplayback?p=1',
      mime: 'video/mp4'
    },
    videoCandidates: [
      { url: 'https://rr1---sn-a.googlevideo.com/videoplayback?v=1' },
      { url: 'https://rr2---sn-b.googlevideo.com/videoplayback?v=2' }
    ],
    audioCandidates: [
      { url: 'https://rr1---sn-a.googlevideo.com/videoplayback?a=1' }
    ]
  };
  assert.equal(streamPayloadIsAllowed(good), true);
  assert.equal(streamPayloadIsAllowed({
    ...good,
    audioCandidates: [{ url: 'https://evil.example/audio.webm' }]
  }), false);
  assert.equal(streamPayloadIsAllowed({
    ...good,
    progressiveFallback: { url: 'https://evil.example/fallback.mp4' }
  }), false);
});

test('GEKKO Direct V2 synchronizes split audio with video controls and drift correction', () => {
  const source = directPlayerSource({
    mode: 'adaptive',
    videoId: 'dQw4w9WgXcQ',
    quality: '2160p60',
    height: 2160,
    fps: 60,
    videoCandidates: [{
      url: 'https://rr1---sn-a.googlevideo.com/videoplayback?v=1',
      mime: 'video/webm; codecs="vp9"',
      quality: '2160p60',
      height: 2160,
      fps: 60
    }],
    audioCandidates: [{
      url: 'https://rr1---sn-a.googlevideo.com/videoplayback?a=1',
      mime: 'audio/webm; codecs="opus"'
    }],
    progressiveFallback: {
      url: 'https://rr2---sn-b.googlevideo.com/videoplayback?p=1',
      mime: 'video/mp4; codecs="avc1.640028, mp4a.40.2"',
      quality: '720p',
      height: 720,
      fps: 30
    }
  });

  assert.match(source, /__gekkoDirectPlayerV2/);
  assert.match(source, /selectedVideo/);
  assert.match(source, /selectedAudio/);
  assert.match(source, /drift > 0\.22/);
  assert.match(source, /audio\.currentTime = target/);
  assert.match(source, /video\.addEventListener\('play'/);
  assert.match(source, /video\.addEventListener\('pause'/);
  assert.match(source, /video\.addEventListener\('seeking'/);
  assert.match(source, /video\.addEventListener\('ratechange'/);
  assert.match(source, /video\.addEventListener\('volumechange'/);
  assert.match(source, /setInterval\(\(\) =>/);
  assert.match(source, /250/);
});

test('GEKKO Direct V2 downgrades adaptive playback to progressive before abandoning Direct mode', () => {
  const source = directPlayerSource({
    mode: 'adaptive',
    videoId: 'dQw4w9WgXcQ',
    videoCandidates: [{
      url: 'https://rr1---sn-a.googlevideo.com/videoplayback?v=1',
      mime: 'video/webm; codecs="vp9"',
      quality: '2160p60',
      height: 2160,
      fps: 60
    }],
    audioCandidates: [{
      url: 'https://rr1---sn-a.googlevideo.com/videoplayback?a=1',
      mime: 'audio/webm; codecs="opus"'
    }],
    progressiveFallback: {
      url: 'https://rr2---sn-b.googlevideo.com/videoplayback?p=1',
      mime: 'video/mp4',
      quality: '720p'
    }
  });

  assert.match(source, /downgradeToProgressive/);
  assert.match(source, /progressiveFallback/);
  assert.match(source, /video\.src = progressive\.url/);
  assert.match(source, /audioFailed = true/);
  assert.match(source, /destroy\(true, 10000\)/);
  assert.match(source, /Player YouTube/);
  assert.match(source, /destroy\(true, 30000\)/);
});

test('GEKKO Direct V2 keeps normal YouTube fallback reversible at current playback position', () => {
  const source = directPlayerSource({
    mode: 'progressive',
    videoId: 'dQw4w9WgXcQ',
    url: 'https://rr1---sn-a.googlevideo.com/videoplayback?p=1',
    mime: 'video/mp4',
    quality: '720p'
  });
  assert.match(source, /nativeVideo\.currentTime = safeTime\(video\.currentTime \|\| startTime\)/);
  assert.match(source, /nativeVideo\.playbackRate = video\.playbackRate/);
  assert.match(source, /nativeVideo\.play/);
});
