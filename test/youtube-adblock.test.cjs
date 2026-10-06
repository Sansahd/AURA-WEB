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
  codecFromMime,
  isVideoOnly,
  isAudioOnly,
  selectAdaptiveFormats,
  streamIsUsable,
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

test('GEKKO Direct selects the best adaptive pair up to 4K', () => {
  const formats = [
    {
      has_video: true, has_audio: false, height: 1080, fps: 60, bitrate: 4_000_000,
      mime_type: 'video/mp4; codecs="avc1.64002a"', quality_label: '1080p60'
    },
    {
      has_video: true, has_audio: false, height: 1440, fps: 30, bitrate: 6_000_000,
      mime_type: 'video/webm; codecs="vp9"', quality_label: '1440p'
    },
    {
      has_video: true, has_audio: false, height: 2160, fps: 60, bitrate: 12_000_000,
      mime_type: 'video/webm; codecs="vp9"', quality_label: '2160p60'
    },
    {
      has_video: true, has_audio: false, height: 4320, fps: 60, bitrate: 25_000_000,
      mime_type: 'video/webm; codecs="vp9"', quality_label: '4320p60'
    },
    {
      has_video: false, has_audio: true, bitrate: 128_000, audio_channels: 2,
      mime_type: 'audio/mp4; codecs="mp4a.40.2"'
    },
    {
      has_video: false, has_audio: true, bitrate: 160_000, audio_channels: 2,
      mime_type: 'audio/webm; codecs="opus"'
    }
  ];

  const selected = selectAdaptiveFormats(formats);
  assert.equal(selected.video.height, 2160);
  assert.equal(selected.video.quality_label, '2160p60');
  assert.equal(selected.audio.bitrate, 160_000);
  assert.deepEqual(selected.availableHeights, [2160, 1440, 1080]);
  assert.equal(codecFromMime(selected.video.mime_type), 'vp9');
  assert.equal(isVideoOnly(selected.video), true);
  assert.equal(isAudioOnly(selected.audio), true);
});

test('GEKKO Direct ignores DRM adaptive candidates and respects 2160p ceiling', () => {
  const formats = [
    {
      has_video: true, has_audio: false, height: 2160, fps: 60, bitrate: 20_000_000,
      mime_type: 'video/webm; codecs="vp9"', is_drm: true
    },
    {
      has_video: true, has_audio: false, height: 1440, fps: 60, bitrate: 9_000_000,
      mime_type: 'video/webm; codecs="vp9"'
    },
    {
      has_audio: true, has_video: false, bitrate: 128_000,
      mime_type: 'audio/webm; codecs="opus"'
    }
  ];
  const selected = selectAdaptiveFormats(formats);
  assert.equal(selected.video.height, 1440);
});

test('GEKKO Direct validates both legs of an adaptive stream', () => {
  assert.equal(streamIsUsable({
    mode: 'adaptive',
    video: { url: 'https://v.googlevideo.com/videoplayback?id=v' },
    audio: { url: 'https://a.googlevideo.com/videoplayback?id=a' }
  }), true);
  assert.equal(streamIsUsable({
    mode: 'adaptive',
    video: { url: 'https://v.googlevideo.com/videoplayback?id=v' },
    audio: { url: 'https://evil.example/audio' }
  }), false);
});

test('GEKKO Direct player is reversible and preserves normal YouTube fallback', () => {
  const source = directPlayerSource({
    mode: 'adaptive',
    videoId: 'dQw4w9WgXcQ',
    quality: '2160p60',
    availableHeights: [2160, 1440, 1080],
    video: {
      url: 'https://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=video',
      quality: '2160p60',
      mime: 'video/webm; codecs="vp9"',
      height: 2160,
      fps: 60
    },
    audio: {
      url: 'https://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=audio',
      mime: 'audio/webm; codecs="opus"',
      bitrate: 160000
    },
    fallback: {
      url: 'https://rr1---sn-ab5l6n7z.googlevideo.com/videoplayback?id=fallback',
      quality: '720p',
      mime: 'video/mp4'
    }
  });
  assert.match(source, /GEKKO DIRECT/);
  assert.match(source, /Player YouTube/);
  assert.match(source, /nativeVideo\.currentTime/);
  assert.match(source, /A\/V SYNC/);
  assert.match(source, /syncAudio/);
  assert.match(source, /HARD_DRIFT/);
  assert.match(source, /SOFT_DRIFT/);
  assert.match(source, /audio\.playbackRate/);
  assert.match(source, /setInterval/);
  assert.match(source, /250/);
  assert.match(source, /useProgressiveFallback/);
  assert.match(source, /video\.addEventListener\('error'/);
  assert.match(source, /audio\.addEventListener\('error'/);
  assert.match(source, /destroy\(true, 30000\)/);
});

test('YouTube guard exposes manual Direct mode and automatic persistent-ad fallback', () => {
  const source = youtubeGuardSource();
  assert.match(source, /gekko-direct-button/);
  assert.match(source, /gekko-direct:\/\/youtube\//);
  assert.match(source, /Date\.now\(\) - state\.adSince >= 1800/);
  assert.match(source, /directResult/);
  assert.match(source, /resetDirect/);
});


test('GEKKO Direct adaptive selector prefers 4K high-fps video and strong default audio', () => {
  const formats = [
    {
      has_video: true, has_audio: false, height: 1080, width: 1920, fps: 60,
      bitrate: 8_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '1080p60'
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
      has_video: true, has_audio: false, height: 4320, width: 7680, fps: 30,
      bitrate: 60_000_000, mime_type: 'video/webm; codecs="vp9"', quality_label: '4320p'
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
  assert.equal(selected.audio[0].bitrate, 160_000);
  assert.equal(selected.audio[0].audio_track.audio_is_default, true);
  assert.equal(selected.video.some((format) => format.height > 2160), false);
});

test('GEKKO Direct adaptive payload requires all media URLs to remain on googlevideo HTTPS', () => {
  const good = {
    mode: 'adaptive',
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
    }]
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
