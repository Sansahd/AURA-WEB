'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { isAllowedIconSource, fetchSmallIcon } = require('../src/services/favicon-cache.cjs');
const read = p => fs.readFileSync(path.join(__dirname, '..', p), 'utf8');

test('the native browser reports favicons and the shell shows safe inline PNGs', () => {
  const main=read('src/main.cjs'), ui=read('src/renderer/renderer.js');
  const css=read('src/renderer/gekko-24-layout.css');
  assert.match(main, /wc\.on\('page-favicon-updated'/);
  assert.match(main, /fetchSmallIcon\(\{/);
  assert.match(main, /favicon: tab\.favicon \|\| ''/);
  assert.match(main, /faviconRequest/);
  assert.match(ui, /node\._favicon\.src = favicon/);
  assert.match(ui, /node\._favicon\.dataset\.source/);
  assert.match(ui, /data:image\\/png;base64/);
  assert.match(css, /\.tab-favicon\[hidden\]/);
  assert.match(css, /body\[data-rail-position="top"\] #sidestage-rail \.tab-favicon/);
});

test('favicons are restricted to the same origin family to avoid tracking', () => {
  const page='https://www.example.com/article';
  assert.equal(isAllowedIconSource('https://www.example.com/favicon.ico',page),true);
  assert.equal(isAllowedIconSource('https://example.com/static/icon.png',page),true);
  assert.equal(isAllowedIconSource('https://cdn.www.example.com/a.png',page),true);
  assert.equal(isAllowedIconSource('https://unrelated-tracker.invalid/a.png',page),false);
  assert.equal(isAllowedIconSource('file:///C:/secret',page),false);
  assert.equal(isAllowedIconSource('javascript:alert(1)',page),false);
  assert.equal(isAllowedIconSource('http://www.example.com/favicon.ico',page),false);
  assert.equal(isAllowedIconSource('https://localhost/favicon.ico',page),false);
});

test('favicons fetched in a tab session are rasterized to a bounded inert PNG', async () => {
  const icon=Buffer.from([1,2,3,4]);
  const image={
    createFromBuffer(buffer) {
      assert.deepEqual(buffer,icon);
      return { isEmpty:()=>false,resize:()=>({ toPNG:()=>Buffer.from('png') }) };
    }
  };
  const fetch=async url => {
    assert.equal(url,'https://example.com/favicon.ico');
    return {
      ok:true,
      headers:{ get:x=>x==='content-type'?'image/x-icon':null },
      body: { getReader:()=>({
        read: async () => ({value:icon,done:false}),
        cancel:async()=>{},
        releaseLock:()=>{}
      }) }
    };
  };
  // The downloader must stop at its bound: use a finite stream in this test.
  let next=0;
  const finite=async url => {
    const result=await fetch(url);
    result.body.getReader=()=>({
      read:async()=>++next===1?{value:icon,done:false}:{done:true},
      releaseLock:()=>{}
    });
    return result;
  };
  const got=await fetchSmallIcon({
    candidates:['https://example.com/favicon.ico'],
    pageUrl:'https://example.com/',
    fetch:finite,nativeImage:image
  });
  assert.equal(got,'data:image/png;base64,'+Buffer.from('png').toString('base64'));
  assert.equal(next,2);
});

test('truncated overlarge icon payloads are rejected safely', async () => {
  const got=await fetchSmallIcon({
    candidates:['https://example.com/favicon.ico'],
    pageUrl:'https://example.com/',
    fetch:async()=>({
      ok:true,
      headers:{ get:()=>null },
      body:{ getReader:()=>({
        read:async()=>({done:false,value:Buffer.alloc(150000)}),
        cancel:async()=>{},
        releaseLock:()=>{}
      })}
    }),
    nativeImage:{createFromBuffer:()=>{throw Error('must not decode oversized icon');}}
  });
  assert.equal(got,'');
});

test('new-tab plus lives within the tabs list and follows the last open tab',()=>{
  const ui=read('src/renderer/renderer.js');
  const css=read('src/renderer/gekko-24-layout.css');
  assert.match(ui,/tabsEl\.append\(plusButton\)/);
  assert.doesNotMatch(ui,/shelf\.append\(plusButton\)/);
  assert.match(css, /#sidestage-rail \.tabs > \.plus/);
  assert.match(css, /body\[data-rail-position="top"\] #sidestage-rail \.tabs > \.plus/);
});

test('full-bleed bars leave no mismatched background exposed at rounded window corners',()=>{
  const css=read('src/renderer/gekko-24-layout.css');
  const start=css.lastIndexOf('GEKKO 2.4 beta 10');
  const tail=css.slice(start);
  assert.ok(start>0);
  assert.match(tail,/#bottom-dock/);
  assert.match(tail,/left:0!important/);
  assert.match(tail,/right:0!important/);
  assert.match(tail,/bottom:0!important/);
  assert.match(tail,/border-radius:0!important/);
  assert.match(tail,/#sidestage-rail:not\(\.rail-minimized\)/);
  assert.match(tail,/#sidestage-rail\.rail-minimized/);
  assert.match(tail,/\.tab-favicon\{/);
});
