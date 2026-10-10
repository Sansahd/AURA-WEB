'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const main=fs.readFileSync(path.join(__dirname,'../src/main.cjs'),'utf8');
const section=main.split('function togglePanelTab(url) {')[1]?.split('\nfunction toggleTabMute(')[0];
test('SOCIAL rail opens a real tab and immediately attempts secure login',()=>{
 assert.ok(section);
 assert.match(section,/createTab\(normalized, true\)/);
 assert.match(section,/isSocialPage\(normalized\) && !isPrivateMode\(\)/);
 assert.match(section,/void loginSocialWithQuanticId\(\)/);
 assert.match(section,/if \(existing\.id === activeId\) return closeTab\(existing\.id\)/);
});
test('SOCIAL SSO is not triggered for Quantic Mail or in private browsing',()=>{
 assert.match(section,/if \(activated && isSocialPage\(normalized\) && !isPrivateMode\(\)\)/);
 assert.match(section,/if \(isSocialPage\(normalized\) && !isPrivateMode\(\)\)/);
});
