'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm=require('node:vm');
const {enableVideoSpotlightScript,disableVideoSpotlightScript}=require('../src/services/video-spotlight.cjs');

function createPage({video=true,playing=true,small=false}={}){
 const holder=[];
 const mk=()=>({
  style:{},attrs:{},isConnected:true,
  setAttribute(key,value){this.attrs[key]=value},
  attachShadow(){const shadow={children:[],append(v){this.children.push(v)}};this.shadow=shadow;return shadow},
  getBoundingClientRect(){return {left:220,top:120,right:small?450:960,bottom:small?250:540,
    width:small?230:740,height:small?130:420}}
 });
 const root=mk();root.clientWidth=1200;root.clientHeight=800;
 root.append=node=>holder.push(node);
 const player=mk();player.paused=!playing;player.ended=false;player.readyState=4;
 const document={documentElement:root,createElement:mk,
  querySelectorAll(selector){return selector==='video'&&video?[player]:[]}};
 const window={getComputedStyle:()=>({display:'block',visibility:'visible',opacity:'1'})};
 const intervals=[];
 const sandbox={window,document,innerWidth:1200,innerHeight:800,
  setInterval(fn,time){intervals.push({fn,time});return intervals.length},
  clearInterval(){}};
 return {sandbox,holder,intervals,player};
}

test('cinematic video focus dims only the four regions outside a large playing video',()=>{
 const {sandbox,holder,intervals}=createPage();
 const activated=vm.runInNewContext(enableVideoSpotlightScript(),sandbox);
 assert.equal(activated,true);
 assert.equal(holder.length,1);
 assert.equal(holder[0].attrs['data-gekko-cinema'],'');
 assert.equal(holder[0].style.display,'block');
 const parts=holder[0].shadow.children;
 assert.equal(parts.length,4);
 assert.equal(parts[0].style.height,'112px');
 assert.equal(parts[1].style.width,'212px');
 assert.equal(parts[2].style.width,'232px');
 assert.equal(parts[3].style.height,'252px');
 assert.equal(intervals.length,1);
 vm.runInNewContext(disableVideoSpotlightScript,sandbox);
 assert.equal(holder[0].style.display,'none');
});

test('small videos and pages without video never trigger spotlight',()=>{
 for(const options of [{small:true},{video:false},{playing:false}]){
  const {sandbox,holder}=createPage(options);
  assert.equal(vm.runInNewContext(enableVideoSpotlightScript(),sandbox),false);
  assert.equal(holder[0].style.display,'none');
 }
});

test('playback focus survives repeated enable without adding layers or extra intervals',()=>{
 const {sandbox,holder,intervals}=createPage();
 vm.runInNewContext(enableVideoSpotlightScript(),sandbox);
 vm.runInNewContext(enableVideoSpotlightScript(),sandbox);
 assert.equal(holder.length,1);
 assert.equal(intervals.length,1);
});
