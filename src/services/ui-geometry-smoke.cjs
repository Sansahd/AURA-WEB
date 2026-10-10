'use strict';
const fs = require('node:fs');
const path = require('node:path');

function scheduleGeometrySmoke(win, app) {
  if (process.env.GEKKO_LAYOUT_SMOKE !== '1') return;
  setTimeout(async () => {
    let status = 2;
    try {
      if (!win || win.isDestroyed()) throw new Error('window_destroyed');
      const snapshot = await win.webContents.executeJavaScript(`(() => {
        const rect = selector => {
          const node = document.querySelector(selector);
          if (!node) return null;
          const r = node.getBoundingClientRect();
          return {
            left: Math.round(r.left), top: Math.round(r.top),
            right: Math.round(r.right), bottom: Math.round(r.bottom),
            width: Math.round(r.width), height: Math.round(r.height),
            visible: getComputedStyle(node).display !== 'none'
          };
        };
        const shell = rect('#shell');
        const home = rect('#home');
        const scene = rect('#home .gekko-home-scene');
        const dock = rect('#bottom-dock');
        const rail = rect('#bottom-dock > #sidestage-rail');
        const input = rect('#bottom-dock #address');
        const tabShelf = rect('#bottom-dock .gekko-tab-shelf');
        const windowControls = rect('#bottom-dock .gekko-rail-window-controls');
        return {
          width:innerWidth,height:innerHeight,dpr:devicePixelRatio,
          railPosition:document.body.dataset.railPosition,
          shell,home,scene,dock,rail,input,tabShelf,windowControls
        };
      })()`, true);
      const tolerance = 15;
      const errors = [];
      for (const key of ['shell', 'home', 'scene']) {
        if (!snapshot[key] || snapshot[key].right < snapshot.width - tolerance)
          errors.push(key + '_right_edge_missing');
      }
      if (!snapshot.dock || snapshot.dock.right < snapshot.width - tolerance ||
          snapshot.dock.width < snapshot.width - 2 * tolerance)
        errors.push('dock_does_not_span_window');
      if (!snapshot.input || !snapshot.input.visible || snapshot.input.width < 55)
        errors.push('omnibox_not_visible');
      if (!snapshot.rail || snapshot.rail.width < 50 ||
          snapshot.rail.right > snapshot.dock.right + 2)
        errors.push('unified_rail_not_inside_dock');
      if (!snapshot.windowControls || snapshot.windowControls.right > snapshot.width + 2)
        errors.push('window_controls_clipped');
      if (snapshot.railPosition !== 'bottom') errors.push('not_bottom_layout');
      console.log('[GEKKO_LAYOUT_SMOKE]', JSON.stringify({ ...snapshot, errors }));
      const screenshot = await win.capturePage();
      if (!screenshot.isEmpty()) {
        const outfile = path.join(process.cwd(), 'dist', 'gekko-glass-wide-smoke.png');
        fs.mkdirSync(path.dirname(outfile), { recursive: true });
        fs.writeFileSync(outfile, screenshot.toPNG());
        console.log('[GEKKO_LAYOUT_SCREENSHOT]', outfile);
      }
      status = errors.length ? 2 : 0;
    } catch (error) {
      console.error('[GEKKO_LAYOUT_SMOKE_ERROR]', error?.stack || error);
    } finally {
      app.exit(status);
    }
  }, 2600);
}

module.exports = { scheduleGeometrySmoke };
