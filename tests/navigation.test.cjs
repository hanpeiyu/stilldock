const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../assets/navigation.js'),'utf8');
const noteId='abcdef0123456789abcdef01';
function boot({initial='/search_result?keyword=room',stored='',nativeClose=false}={}){
  const location=new URL('https://www.xiaohongshu.com'+initial),timers=[],listeners={},data=new Map([['stilldock.returnUrl',stored]]);let closeCount=0,backCount=0,assigned='';
  location.assign=url=>{assigned=url;location.href=url};
  function node(){const attrs=new Set();return {attrs,children:[],hidden:false,style:{},getAttribute:()=>null,setAttribute(k){attrs.add(k)},toggleAttribute(k,force){if(force)attrs.add(k);else attrs.delete(k)},getClientRects:()=>[{}],append(...items){this.children.push(...items)}};}
  const root=node(),body=node(),close=node();close.click=()=>{closeCount++;location.href='https://www.xiaohongshu.com/search_result?keyword=room'};
  const document={body,documentElement:root,head:root,readyState:'complete',referrer:'',createElement:node,querySelectorAll:s=>nativeClose&&s.includes('.close-circle')&&location.pathname.includes(noteId)?[close]:[],addEventListener:(name,fn)=>listeners[name]=fn};
  const stack=[location.href];const history={pushState(a,b,url){stack.push(new URL(url,location.href).href);location.href=stack.at(-1)},replaceState(a,b,url){stack[stack.length-1]=new URL(url,location.href).href;location.href=stack.at(-1)},back(){backCount++;if(stack.length>1){stack.pop();location.href=stack.at(-1);listeners.popstate?.();}}};
  const window={};window.top=window;
  vm.runInNewContext(source,{window,document,location,history,URL,sessionStorage:{getItem:k=>data.get(k),setItem:(k,v)=>data.set(k,v)},getComputedStyle:()=>({display:'block',visibility:'visible',opacity:'1'}),MutationObserver:function(){this.observe=()=>{}},setTimeout:fn=>{timers.push(fn);return timers.length},addEventListener:(name,fn)=>listeners[name]=fn});
  return {window,body,history,location,listeners,timers,get closeCount(){return closeCount},get backCount(){return backCount},get assigned(){return assigned}};
}
test('return button appears for notes, prefers native close and preserves the search page',()=>{
  const p=boot({nativeClose:true}),button=p.body.children[0];assert.equal(button.hidden,true);
  p.history.pushState(null,'','/search_result/'+noteId);assert.equal(button.hidden,false);
  button.onclick();assert.equal(p.closeCount,1);assert.equal(p.backCount,0);p.timers.shift()();assert.equal(button.hidden,true);assert.equal(p.assigned,'');
});
test('Escape returns to the preceding list when native close is unavailable',()=>{
  const p=boot();p.history.pushState(null,'','/explore/'+noteId);let prevented=false,stopped=false;
  p.listeners.keydown({key:'Escape',preventDefault(){prevented=true},stopImmediatePropagation(){stopped=true}});
  assert.equal(p.backCount,1);assert.ok(prevented&&stopped);assert.equal(p.location.pathname,'/search_result');
});
test('direct note entry uses a same-origin saved list and rejects external return URLs',()=>{
  const p=boot({initial:'/explore/'+noteId,stored:'https://www.xiaohongshu.com/search_result?keyword=saved'});p.window.__stillDockNavigation.back();assert.equal(p.assigned,'https://www.xiaohongshu.com/search_result?keyword=saved');
  const q=boot({initial:'/explore/'+noteId,stored:'https://evil.example/search_result'});q.window.__stillDockNavigation.back();assert.equal(q.assigned,'https://www.xiaohongshu.com/explore');
});
test('Escape on the list does not consume keyboard events',()=>{
  const p=boot();p.listeners.keydown({key:'Escape',preventDefault(){throw Error('should not prevent')},stopImmediatePropagation(){throw Error('should not stop')}});assert.equal(p.backCount,0);
});

test('Escape closes the material preview first and native close resumes only after workspace cleanup',()=>{
  const p=boot({initial:'/explore/'+noteId,nativeClose:true});let preview=true,prepared=0;
  p.window.__stillDockMaterial={isOpen:()=>true,closePreview(){const old=preview;preview=false;return old;},prepareBack(){prepared++;}};
  p.listeners.keydown({key:'Escape',preventDefault(){},stopImmediatePropagation(){}});assert.equal(p.closeCount,0);assert.equal(prepared,0);
  p.listeners.keyup({key:'Escape',preventDefault(){},stopImmediatePropagation(){}});
  p.listeners.keydown({key:'Escape',preventDefault(){},stopImmediatePropagation(){}});assert.equal(p.closeCount,1);assert.equal(prepared,1);
});

test('one held Escape consumes repeats and keyup without closing the underlying note',()=>{
  const p=boot({initial:'/explore/'+noteId,nativeClose:true});let preview=true;
  p.window.__stillDockMaterial={isOpen:()=>true,isPreviewOpen:()=>preview,closePreview(){if(!preview)return false;preview=false;return true;},prepareBack(){}};
  const event={key:'Escape',preventDefault(){},stopImmediatePropagation(){}};p.listeners.keydown(event);p.listeners.keydown({...event,repeat:true});assert.equal(p.closeCount,0);
  let prevented=false;p.listeners.keyup({...event,preventDefault(){prevented=true;}});assert.equal(prevented,true);assert.equal(p.closeCount,0);
  p.listeners.keydown(event);assert.equal(p.closeCount,1);
});
