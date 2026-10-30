const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const script=fs.readFileSync(path.join(__dirname,'../assets/site.js'),'utf8');
function boot(initial='/explore',{adapterThrows=false}={}){
  const sent=[],listeners={};const location=new URL('https://www.xiaohongshu.com'+initial);
  const document={readyState:'complete',title:'XHS',querySelector:()=>null,querySelectorAll:()=>[],addEventListener:(name,fn)=>listeners[name]=fn};
  const window={ipc:{postMessage:s=>sent.push(JSON.parse(s))},__xhsInstallAdapters(){if(adapterThrows)throw Error('style adapter');}};window.top=window;
  const history={pushState(a,b,url){location.href=new URL(url,location.href).href},replaceState(a,b,url){location.href=new URL(url,location.href).href}};
  const observer={observe(){}};
  vm.runInNewContext(script,{window,document,location,URL,history,MutationObserver:function(){return observer},addEventListener:(name,fn)=>listeners[name]=fn,setTimeout:()=>1});
  return {sent,listeners,history,count:()=>sent.filter(v=>v.kind==='search_start').length};
}
test('SPA search and new keyword expand once, unchanged route stays manually collapsible',()=>{
  const p=boot();assert.equal(p.count(),0);
  p.history.pushState(null,'','/search_result?keyword=one');assert.equal(p.count(),1);
  p.history.replaceState(null,'','/search_result?keyword=one&source=web');assert.equal(p.count(),1);
  p.history.pushState(null,'','/search_result?keyword=two');assert.equal(p.count(),2);
});
test('restoring a result page expands even if the appearance adapter fails',()=>{
  assert.equal(boot('/search_result?keyword=one',{adapterThrows:true}).count(),1);
});
test('Enter, search button, form submission and history links trigger expansion',()=>{
  for(const type of ['keydown','click','submit']){
    const p=boot();const target={closest:s=>s.includes('.input-box')||s.includes('button')?{}:null};
    p.listeners[type]({type,target,key:'Enter'});assert.equal(p.count(),1,type);
  }
  const p=boot();p.listeners.click({type:'click',target:{closest:s=>s==='a[href]'?{getAttribute:()=>'/search_result?keyword=history'}:null}});assert.equal(p.count(),1);
});
test('typing, unrelated clicks and IME Enter do not expand the panel',()=>{
  const p=boot();const target={closest:()=>({})};
  p.listeners.keydown({type:'keydown',target,key:'a'});
  p.listeners.keydown({type:'keydown',target,key:'Enter',isComposing:true});
  p.listeners.click({type:'click',target:{closest:()=>null}});assert.equal(p.count(),0);
});
