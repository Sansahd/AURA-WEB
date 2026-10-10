'use strict';

// Only a 1x1 average RGB reaches the browser shell. No page screenshot,
// bitmap, URL or personal page content is written to disk or sent remotely.
async function sampleRailColor(webContents, bounds, position) {
  if (!webContents || webContents.isDestroyed?.() || !bounds?.width || !bounds?.height) return null;
  if (!['right', 'left'].includes(position)) return null;
  const width = Math.min(32, Math.max(1, Math.floor(bounds.width)));
  const height = Math.min(620, Math.max(1, Math.floor(bounds.height)));
  const x = position === 'right' ? Math.max(0, Math.floor(bounds.width) - width) : 0;
  const image = await webContents.capturePage({ x, y: 0, width, height });
  if (!image || image.isEmpty?.()) return null;
  const sample = image.resize({ width: 1, height: 1 }).toBitmap();
  if (!sample || sample.length < 4) return null;
  // Electron's bitmap representation on Windows is BGRA.
  const [b, g, r] = sample;
  return `rgb(${r}, ${g}, ${b})`;
}

module.exports = { sampleRailColor };
