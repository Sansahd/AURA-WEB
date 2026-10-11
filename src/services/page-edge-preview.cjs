'use strict';
// Paint-only preview of the visible website EDGE, in the same window. Never
// persisted, transmitted over network, or taken in private/DRM playback.
// Native WebContentsView is above GEKKO's shell and reserves 60/96px outside
// the page; this tiny capture visually continues the site beneath the glass.
// It is not a screenshot of the desktop or of any other application.
function edgeRect(bounds, position, band=72){
  const width=Math.max(1,Math.floor(bounds?.width||0));
  const height=Math.max(1,Math.floor(bounds?.height||0));
  const thickness=Math.min(band, position==='top'||position==='bottom'?height:width);
  if(position==='top')return {x:0,y:0,width,height:thickness};
  if(position==='bottom')return {x:0,y:height-thickness,width,height:thickness};
  if(position==='left')return {x:0,y:0,width:thickness,height};
  if(position==='right')return {x:width-thickness,y:0,width:thickness,height};
  return null;
}
function previewLayout(position){
  return ['left','right'].includes(position)
    ? {position:position+' center',size:'96px 100%'}
    : {position:'center '+position,size:'100% 60px'};
}
async function captureEdge(wc,bounds,position){
  if(!wc||wc.isDestroyed()||wc.isLoading())return null;
  const rect=edgeRect(bounds,position);
  if(!rect || rect.width<1 || rect.height<1)return null;
  const image=await wc.capturePage(rect);
  if(!image||image.isEmpty())return null;
  const small=image.resize(
    rect.width>=rect.height
      ? {width:Math.min(520,rect.width),quality:'good'}
      : {height:Math.min(650,rect.height),quality:'good'});
  const url=small.toDataURL();
  if(!/^data:image\/png;base64,[a-z0-9+/=]+$/i.test(url)||url.length>240000)return null;
  return {image:url,...previewLayout(position)};
}
module.exports={edgeRect,previewLayout,captureEdge};
