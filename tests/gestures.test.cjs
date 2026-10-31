const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const script=fs.readFileSync(path.join(__dirname,'../assets/gestures.js'),'utf8').replace('__GESTURES_ENABLED__','true');
function boot(){
  let time=0,back=0;const sent=[],listeners={},frames=new Map(),timers=new Map();let serial=0;
  function node(){return {children:[],attrs:{},isConnected:true,hidden:false,style:{},setAttribute(k,v){this.attrs[k]=v},append(...items){this.children.push(...items)},setPointerCapture(){},releasePointerCapture(){},closest:()=>null};}
  const root=node(),body=node(),document={body,head:root,documentElement:root,createElement:node,createElementNS:node,addEventListener:(key,fn)=>listeners[key]=fn};
  const location=new URL('https://www.xiaohongshu.com/explore/abcdef0123456789abcdef01');
  const note={id:'abcdef0123456789abcdef01',url:location.href,assets:[{url:'https://sns-img-bd.xhscdn.com/1'},{url:'https://sns-img-bd.xhscdn.com/2',live_url:'https://sns-video-bd.xhscdn.com/live'}]};
  const window={ipc:{postMessage:raw=>sent.push(JSON.parse(raw))},__stillDockNavigation:{isOpen:()=>true,back:()=>back++},__xhsReadNote:()=>note,__xhsReadCurrentImage:()=>({note,index:1})};window.top=window;
  vm.runInNewContext(script,{window,document,location,innerWidth:1200,innerHeight:800,performance:{now:()=>time},setTimeout:fn=>{const id=++serial;timers.set(id,fn);return id},clearTimeout:id=>timers.delete(id),requestAnimationFrame:fn=>{const id=++serial;frames.set(id,fn);return id},cancelAnimationFrame:id=>frames.delete(id),addEventListener:(key,fn)=>listeners[key]=fn});
  function emit(type,extra={}){let prevented=false,stopped=false;const event={pointerId:1,pointerType:'mouse',button:2,buttons:2,clientX:200,clientY:160,target:node(),preventDefault(){prevented=true},stopImmediatePropagation(){stopped=true},...extra};listeners[type]?.(event);return {prevented,stopped};}
  return {window,document,location,note,sent,body,emit,get back(){return back},advance:ms=>time+=ms,flush(){for(const [id,fn] of [...frames]){frames.delete(id);fn()}},actions:()=>sent.filter(v=>v.kind==='gesture')};
}
function stroke(p,x,y){p.emit('pointerdown');p.emit('pointermove',{clientX:x,clientY:y});p.flush();return p.emit('pointerup',{clientX:x,clientY:y,buttons:0});}
test('right stroke renders a bounded trail and executes once only on release',()=>{
  const p=boot();assert.equal(p.body.children.length,0);p.emit('pointerdown');p.emit('pointermove',{clientX:300});p.flush();assert.equal(p.actions().length,0);assert.equal(p.body.children.length,1);
  assert.match(p.body.children[0].children[1].textContent,/下载栏/);p.emit('pointerup',{clientX:300,buttons:0});p.emit('pointerup',{clientX:300,buttons:0});assert.equal(p.actions().length,1);assert.equal(p.actions()[0].action,'toggle_sidebar');
});
test('left returns through the existing native navigation handler',()=>{const p=boot();stroke(p,110,160);assert.equal(p.back,1);assert.equal(p.actions().length,0)});
test('down captures the currently visible image and its Live metadata',()=>{const p=boot();stroke(p,200,260);assert.equal(p.actions()[0].index,1);assert.equal(p.actions()[0].note.assets[1].live_url,p.note.assets[1].live_url)});
test('a short right click retains the native context menu',()=>{const p=boot();stroke(p,215,166);assert.equal(p.actions().length,0);assert.equal(p.emit('contextmenu').prevented,false)});
test('long diagonal, upward and returning strokes cancel instead of guessing',()=>{
  for(const [x,y] of [[280,240],[200,70]]){const p=boot();stroke(p,x,y);assert.equal(p.actions().length,0);assert.equal(p.emit('contextmenu').prevented,true)}
  const p=boot();p.emit('pointerdown');p.emit('pointermove',{clientX:310});p.emit('pointerup',{clientX:205,buttons:0});assert.equal(p.actions().length,0);assert.equal(p.emit('contextmenu').prevented,true);
});
test('Escape cancels the stroke, suppresses its menu and leaves the next ordinary right click intact',()=>{
  const p=boot();p.emit('pointerdown');p.emit('pointermove',{clientX:300});assert.equal(p.emit('keydown',{key:'Escape'}).prevented,true);p.emit('pointerup',{clientX:300,buttons:0});assert.equal(p.actions().length,0);assert.equal(p.emit('contextmenu').prevented,true);
  p.emit('pointerdown');p.emit('pointerup',{buttons:0});assert.equal(p.emit('contextmenu').prevented,false);assert.equal(p.back,0);
});
test('focus loss, pointer cancellation and capture loss never execute',()=>{
  for(const type of ['blur','pointercancel','lostpointercapture']){const p=boot();p.emit('pointerdown');p.emit('pointermove',{clientX:300});p.emit(type);p.emit('pointerup',{clientX:300,buttons:0});assert.equal(p.actions().length,0,type)}
});
test('release outside the viewport and overlong strokes cancel',()=>{const p=boot();stroke(p,-10,160);assert.equal(p.back,0);const q=boot();q.emit('pointerdown');q.advance(5100);q.emit('pointerup',{clientX:300,buttons:0});assert.equal(q.actions().length,0)});
test('editable fields, modifiers and disabled gestures preserve normal right clicks',()=>{
  for(const extra of [{shiftKey:true},{ctrlKey:true},{target:{closest:()=>({})}},{pointerType:'pen'}]){const p=boot();p.emit('pointerdown',extra);p.emit('pointerup',{clientX:300,buttons:0});assert.equal(p.actions().length,0);assert.equal(p.emit('contextmenu').prevented,false)}
  const p=boot();p.window.__stillDockGestures.setEnabled(false);stroke(p,300,160);assert.equal(p.actions().length,0);
});
test('navigation during a stroke cancels instead of applying an action to another page',()=>{const p=boot();p.emit('pointerdown');p.location.href='https://www.xiaohongshu.com/search_result?keyword=next';p.emit('pointerup',{clientX:300,buttons:0});assert.equal(p.actions().length,0)});
test('ambiguous current-image detection does not enqueue a guessed download',()=>{const p=boot();p.window.__xhsReadCurrentImage=()=>({error:'cannot resolve'});stroke(p,200,260);assert.equal(p.actions().length,0)});
test('gesture point storage is capped for long strokes',()=>{const p=boot();p.emit('pointerdown');for(let x=204;x<=1000;x+=4)p.emit('pointermove',{clientX:x});p.flush();const points=p.body.children[0].children[0].children[0].attrs.points;assert.ok(points.split(' ').length<=128);p.emit('pointerup',{clientX:1000,buttons:0});assert.equal(p.actions().length,1)});

test('adding another mouse button or modifier mid-stroke cancels',()=>{
  const p=boot();p.emit('pointerdown');p.emit('pointermove',{clientX:300,buttons:3});p.emit('pointerup',{clientX:300,buttons:1});assert.equal(p.actions().length,0);
  const q=boot();q.emit('pointerdown');q.emit('pointerup',{clientX:300,buttons:0,shiftKey:true});assert.equal(q.actions().length,0);
});

test('inputs inside a material shadow root preserve their normal right-click behavior',()=>{
  const p=boot();p.emit('pointerdown',{composedPath:()=>[{closest:()=>({})},{closest:()=>null}]});p.emit('pointerup',{clientX:300,buttons:0});assert.equal(p.actions().length,0);assert.equal(p.emit('contextmenu').prevented,false);
});
