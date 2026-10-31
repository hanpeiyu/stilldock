const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
function boot({focus=true,ai=true,route='/explore',rootPresent=true,materials=false}={}){
  const attrs=new Set(),nodes=[],timers=[],observers=[];
  const root={setAttribute:k=>attrs.add(k),removeAttribute:k=>attrs.delete(k),append:n=>nodes.push(n)};
  const document={documentElement:rootPresent?root:null,createElement:()=>({})};const window={};
  const source=fs.readFileSync(path.join(__dirname,'../assets/bootstrap.js'),'utf8').replace('__BOOT_CONFIG__',JSON.stringify({focus,ai,materials}));
  vm.runInNewContext(source,{window,document,location:{hostname:'www.xiaohongshu.com',pathname:route},setTimeout:(fn,ms)=>{timers.push({fn,ms});return timers.length},clearTimeout(){},addEventListener(){},MutationObserver:function(fn){observers.push(fn);this.observe=()=>{};this.disconnect=()=>{};}});
  return {attrs,nodes,timers,window,document,root,observers};
}
test('early styles and visibility gate apply before DOMContentLoaded',()=>{
  const b=boot();assert.ok(b.attrs.has('data-xhs-focus'));assert.ok(b.attrs.has('data-xhs-boot'));assert.match(b.nodes[0].textContent,/visibility:hidden/);
  b.window.__xhsReleaseBoot();assert.equal(b.attrs.has('data-xhs-boot'),false);
});
test('timeout releases unknown layouts and original mode does not conceal the page',()=>{
  const b=boot();assert.equal(b.timers[0].ms,1500);b.timers[0].fn();assert.equal(b.attrs.has('data-xhs-boot'),false);
  assert.equal(boot({focus:false,ai:false}).attrs.size,0);
  assert.equal(boot({route:'/search_result'}).attrs.has('data-xhs-boot'),false);
});
test('document creation without an html root attaches styles as soon as it exists',()=>{
  const b=boot({rootPresent:false});assert.equal(b.nodes.length,0);b.document.documentElement=b.root;b.observers[0]();assert.equal(b.nodes.length,1);assert.ok(b.attrs.has('data-xhs-boot'));
});

test('direct note bootstrap has a bounded material transition and leaves ordinary pages alone',()=>{
  const p=boot({route:'/explore/abcdef0123456789abcdef01',materials:true});assert.equal(p.attrs.has('data-sd-material-pending'),true);
  assert.equal(p.timers[0].ms,1000);p.timers[0].fn();assert.equal(p.attrs.has('data-sd-material-pending'),false);
  assert.equal(boot({route:'/search_result',materials:true}).attrs.has('data-sd-material-pending'),false);
});
