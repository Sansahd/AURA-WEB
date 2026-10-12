const test=require('node:test'),assert=require('node:assert/strict');
const {parseRss,searchPublicWeb,safeUrl}=require('../src/services/gekko-search.cjs');
test('GEKKO search sanitizes source links and parses result feed',()=>{
 assert.deepEqual(parseRss('<item><title>Un &amp; deux</title><link>https://example.org/a</link><description>Essai</description></item>'),
 [{title:'Un & deux',url:'https://example.org/a',snippet:'Essai'}]);
 assert.equal(safeUrl('javascript:alert(1)'),'');
 assert.equal(safeUrl('http://localhost/admin'),'');
});
test('GEKKO result fetch never navigates to Brave',async()=>{
 let sources=[];const fetcher=async u=>{sources.push(u);return new Response(
 '<rss><item><title>Test</title><link>https://example.org/x</link><description>OK</description></item></rss>',
 {status:200,headers:{'content-type':'application/rss+xml'}})};
 const result=await searchPublicWeb('question',{fetch:fetcher});
 assert.equal(result.status,'ok');assert.equal(result.results[0].url,'https://example.org/x');
 assert.equal(sources.some(u=>u.includes('brave.com')),false);
});
