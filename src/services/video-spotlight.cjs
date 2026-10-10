'use strict';
// Executed only after Electron observes actual media playback. No network,
// no tracking, no DOM mutations of the video itself. Four translucent strips
// surround the video, avoiding filters on the player and its controls.
function setupVideoSpotlight() {
  if (window.__gekkoVideoSpotlightV1) {
    return window.__gekkoVideoSpotlightV1.enable();
  }
  const root = document.documentElement;
  if (!root) return false;
  const holder = document.createElement('div');
  holder.setAttribute('data-gekko-cinema', '');
  holder.style.cssText = 'position:fixed;inset:0;z-index:2147483646;pointer-events:none;contain:layout style;display:none';
  const shadow = holder.attachShadow({mode:'closed'});
  const rects = Array.from({length:4}, () => {
    const div=document.createElement('div');
    div.style.cssText='position:fixed;background:rgba(3,6,13,.70);pointer-events:none;transition:background .15s ease';
    shadow.append(div);
    return div;
  });
  root.append(holder);
  let clock = null;
  let enabled = false;
  const put = (node,left,top,width,height) => {
    const style = node.style;
    style.left = Math.round(left)+'px';
    style.top = Math.round(top)+'px';
    style.width = Math.max(0,Math.round(width))+'px';
    style.height = Math.max(0,Math.round(height))+'px';
  };
  const valid = node => {
    const b = node.getBoundingClientRect();
    const W = document.documentElement.clientWidth || innerWidth;
    const H = document.documentElement.clientHeight || innerHeight;
    if (b.width < Math.min(330,W*.38) || b.height < Math.min(175,H*.28)) return null;
    if (b.bottom < 0 || b.top > H || b.right < 0 || b.left > W) return null;
    const style = window.getComputedStyle(node);
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) < .2) return null;
    const left = Math.max(0,b.left-8),top=Math.max(0,b.top-8);
    const right = Math.min(W,b.right+8),bottom=Math.min(H,b.bottom+8);
    return {left,top,right,bottom,W,H,area:Math.max(0,right-left)*Math.max(0,bottom-top)};
  };
  function update() {
    if (!enabled || !root.isConnected) { holder.style.display='none'; return false; }
    let selection = null;
    for (const video of document.querySelectorAll('video')) {
      if (video.paused || video.ended || video.readyState < 2) continue;
      const b=valid(video);
      if (b && (!selection || b.area > selection.area)) selection=b;
    }
    // Embedded players may live inside cross-origin frames; we cannot inspect
    // their documents, but we can preserve their visible iframe rectangle.
    if (!selection) {
      for (const frame of document.querySelectorAll('iframe[src]')) {
        if (!/(youtube(?:-nocookie)?\.com|player\.vimeo\.com|dailymotion\.com|video)/i.test(frame.src)) continue;
        const b=valid(frame);
        if (b && (!selection || b.area>selection.area)) selection=b;
      }
    }
    if (!selection) { holder.style.display='none'; return false; }
    const {left,top,right,bottom,W,H}=selection;
    holder.style.display='block';
    put(rects[0],0,0,W,top);
    put(rects[1],0,top,left,bottom-top);
    put(rects[2],right,top,W-right,bottom-top);
    put(rects[3],0,bottom,W,H-bottom);
    return true;
  }
  const api = {
    enable() {
      enabled=true;
      const visibleVideo = update();
      if (!clock) clock=setInterval(update,260);
      return visibleVideo;
    },
    disable() {
      enabled=false;
      clearInterval(clock);
      clock=null;
      holder.style.display='none';
    }
  };
  window.__gekkoVideoSpotlightV1=api;
  return api.enable();
}
function enableVideoSpotlightScript() { return '('+setupVideoSpotlight.toString()+')()'; }
const disableVideoSpotlightScript =
  'globalThis.__gekkoVideoSpotlightV1?.disable(); true';
module.exports = { enableVideoSpotlightScript,disableVideoSpotlightScript };
