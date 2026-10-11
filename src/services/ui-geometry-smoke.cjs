'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { components } = require('electron');

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
        const homeSearch = rect('#home-search-input');
        const favoriteTiles = document.querySelectorAll('#home-favorites-list .gekko-home-favorite').length;
        const dockSurface = getComputedStyle(document.querySelector('#bottom-dock'));
        const dockBackground = dockSurface.backgroundColor;
        const dockBackgroundImage = dockSurface.backgroundImage;
        const tabShelf = rect('#bottom-dock .gekko-tab-shelf');
        const windowControls = rect('#bottom-dock .gekko-rail-window-controls');
        return {
          width:innerWidth,height:innerHeight,dpr:devicePixelRatio,
          railPosition:document.body.dataset.railPosition,
          shell,home,scene,dock,rail,input,homeSearch,favoriteTiles,dockBackground,dockBackgroundImage,tabShelf,windowControls
        };
      })()`, true);
      const tolerance = 16;
      const errors = [];
      const expected = process.env.GEKKO_LAYOUT_POSITION || 'bottom';
      if (typeof components?.whenReady !== 'function') errors.push('castlabs_widevine_runtime_missing');
      const vertical = ['left','right'].includes(expected);
      const contentLeft = expected === 'left' ? 96 : 0;
      const contentRight = expected === 'right' ? snapshot.width - 96 : snapshot.width;
      const contentTop = expected === 'top' ? 60 : 0;
      const contentBottom = expected === 'bottom' ? snapshot.height - 60 : snapshot.height;
      if (!snapshot.shell || snapshot.shell.width < snapshot.width - tolerance)
        errors.push('shell_does_not_fill_window');
      for (const key of ['home', 'scene']) {
        if (!snapshot[key] ||
            Math.abs(snapshot[key].left-contentLeft)>tolerance ||
            Math.abs(snapshot[key].right-contentRight)>tolerance ||
            Math.abs(snapshot[key].top-contentTop)>tolerance ||
            Math.abs(snapshot[key].bottom-contentBottom)>tolerance)
          errors.push(key+'_incorrect_'+expected+'_bounds');
      }
      if (!snapshot.dock || (
         vertical
         ? snapshot.dock.height < snapshot.height - tolerance*2 ||
           snapshot.dock.width > 120 ||
           (expected==='left' ? snapshot.dock.left > tolerance : snapshot.dock.right < snapshot.width-tolerance)
         : snapshot.dock.width < snapshot.width - tolerance*2 ||
           snapshot.dock.height > 75 ||
           (expected==='top' ? snapshot.dock.top > tolerance : snapshot.dock.bottom < snapshot.height-tolerance)
      )) errors.push('dock_does_not_span_chosen_edge');
      if (!snapshot.input || !snapshot.input.visible || snapshot.input.width < (vertical ? 6 : 55))
        errors.push('omnibox_not_visible');
      if (!snapshot.homeSearch || !snapshot.homeSearch.visible ||
          snapshot.homeSearch.width < 190 ||
          Math.abs((snapshot.homeSearch.left+snapshot.homeSearch.right)/2-
                   (snapshot.home.left+snapshot.home.right)/2)>23)
        errors.push('home_center_search_missing_or_offcenter');
      if (snapshot.favoriteTiles < 2) errors.push('local_home_favorites_missing');
      // CSS must not silently reapply an opaque 97% backplate from the older themes.
      const match=/rgba?\([^,]+,[^,]+,[^,]+(?:,\s*([0-9.]+))?\)/.exec(snapshot.dockBackground||'');
      const alpha=match ? Number(match[1]??1) : 1;
      if (alpha > .90 || snapshot.dockBackgroundImage!=='none')
        errors.push('dock_material_not_translucent');
      if (!snapshot.rail || (
        vertical
          ? snapshot.rail.height < 30 || snapshot.rail.bottom > snapshot.dock.bottom + 2
          : snapshot.rail.width < 50 || snapshot.rail.right > snapshot.dock.right + 2
      )) errors.push('unified_rail_not_inside_dock');
      if (!snapshot.windowControls ||
          snapshot.windowControls.right > snapshot.width + 2 ||
          snapshot.windowControls.bottom > snapshot.height + 2)
        errors.push('window_controls_clipped');
      if (snapshot.railPosition !== expected) errors.push('wrong_layout_'+expected);
      console.log('[GEKKO_LAYOUT_SMOKE]', JSON.stringify({ ...snapshot, errors }));
      const screenshot = await win.capturePage();
      if (!screenshot.isEmpty()) {
        const outfile = path.join(process.cwd(), 'dist', 'gekko-glass-wide-smoke-'+(process.env.GEKKO_LAYOUT_POSITION||'bottom')+'.png');
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
