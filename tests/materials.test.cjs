const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const source=fs.readFileSync(require('node:path').join(__dirname,'../assets/materials.js'),'utf8').replace('__MATERIAL_CONFIG__','{enabled:true,includeLive:false}');
function boot(total=120){
  const sent=[],listeners={},frames=new Map(),timers=new Map(),images=[],resizes=[];let serial=0;
  function node(tag='div'){
    const attrs={},events={},e={tag,children:[],parentNode:null,style:{},dataset:{},hidden:false,inert:false,clientWidth:1000,clientHeight:620,scrollTop:0,textContent:'',attrs,events};
    Object.defineProperty(e,'isConnected',{get(){return e===document.body||e===document.head||!!e.parentNode?.isConnected||!!e.host?.isConnected;}});
    e.classList={toggle(name,on){const list=new Set((e.className||'').split(' ').filter(Boolean));on?list.add(name):list.delete(name);e.className=[...list].join(' ');}};
    e.append=(...items)=>{for(const child of items){child.remove?.();child.parentNode=e;e.children.push(child);}};
    e.remove=()=>{if(e.parentNode){const i=e.parentNode.children.indexOf(e);if(i>=0)e.parentNode.children.splice(i,1);e.parentNode=null;}};
    e.insertBefore=(child,before)=>{child.remove();child.parentNode=e;const i=e.children.indexOf(before);if(i<0)e.children.push(child);else e.children.splice(i,0,child);};
    e.setAttribute=(k,v)=>attrs[k]=String(v);e.getAttribute=k=>attrs[k]??null;e.removeAttribute=k=>{delete attrs[k];if(k==='src')delete e.src;};
    e.getClientRects=()=>[{}];e.addEventListener=(k,fn)=>{(events[k]||=[]).push(fn);};e.getRootNode=()=>e.parentNode?e.parentNode.getRootNode():e;
    e.attachShadow=()=>{const root=node('shadow');root.host=e;e.shadowRoot=root;return root;};
    e.querySelectorAll=selector=>all(e).filter(n=>matches(n,selector));e.closest=selector=>matches(e,selector)?e:e.parentNode?.closest(selector)||null;
    e.showModal=()=>e.open=true;e.close=()=>{e.open=false;for(const fn of events.close||[])fn();};e.click=()=>e.onclick?.({preventDefault(){}});
    return e;
  }
  function matches(e,s){return s[0]==='.'?(e.className||'').split(' ').includes(s.slice(1)):s[0]==='#'?e.id===s.slice(1):s[0]==='['?s.slice(1,-1) in e.attrs:e.tag===s;}
  function all(e){return e.children.flatMap(child=>[child,...all(child)]);}
  const document={body:node('body'),head:node('head'),documentElement:node('html'),readyState:'complete',createElement:node,querySelector:s=>s.includes('#noteContainer')?native:null,querySelectorAll:()=>[],addEventListener:(k,fn)=>{(listeners[k]||=[]).push(fn);}};
  document.body.parentNode=document.documentElement;document.head.parentNode=document.documentElement;
  const mask=node();mask.className='note-detail-mask';const native=node();native.id='noteContainer';const video=node('video');video.paused=false;video.pause=()=>video.paused=true;native.append(video);mask.append(native);document.body.append(mask);
  let note={id:'abcdef0123456789abcdef01',title:'<script>plain text</script>',author:'author',url:'https://www.xiaohongshu.com/explore/abcdef0123456789abcdef01',assets:Array.from({length:total},(_,i)=>({url:'https://sns-img-bd.xhscdn.com/full'+i,preview:'https://sns-img-bd.xhscdn.com/thumb'+i,original_url:'https://ci.xiaohongshu.com/hq'+i,sequence:i+1,live_url:i===0?'https://sns-video-bd.xhscdn.com/live':''}))};
  const window={ipc:{postMessage:raw=>sent.push(JSON.parse(raw))},__xhsReadNote:()=>note};window.top=window;
  const context={window,document,URL,getComputedStyle:()=>({display:'block',visibility:'visible'}),location:new URL(note.url),requestAnimationFrame:fn=>{const id=++serial;frames.set(id,fn);return id;},cancelAnimationFrame:id=>frames.delete(id),setTimeout:fn=>{const id=++serial;timers.set(id,fn);return id;},clearTimeout:id=>timers.delete(id),MutationObserver:function(){this.observe=()=>{};},IntersectionObserver:function(fn){this.targets=new Set();this.observe=t=>this.targets.add(t);this.unobserve=t=>this.targets.delete(t);this.disconnect=()=>this.targets.clear();this.deliver=()=>fn([...this.targets].map(target=>({target,isIntersecting:true})));images.push(this);},ResizeObserver:function(fn){this.observe=()=>{};this.disconnect=()=>this.disconnected=true;this.fire=fn;resizes.push(this);},addEventListener:(k,fn)=>{(listeners[k]||=[]).push(fn);}};
  vm.runInNewContext(source,context);
  return {window,document,native,mask,video,sent,images,resizes,listeners,get note(){return note;},set note(value){note=value;},get api(){return window.__stillDockMaterial;},get host(){return document.body.children.find(n=>n.id==='stilldock-material');},find(s){return this.host?.shadowRoot.querySelectorAll(s)||[];},button(text){return this.find('button').find(n=>n.textContent===text);},flush(){for(const [id,fn] of [...frames]){frames.delete(id);fn();}},all};
}

test('large albums mount a bounded card window and load only observed preview sources',()=>{
  const p=boot(400);assert.equal(p.api.isOpen(),true);assert.ok(p.find('.card').length<40);assert.equal(p.video.paused,true);assert.equal(p.mask.inert,true);
  const initial=p.find('img');assert.ok(initial.every(img=>!img.src));p.images[0].deliver();
  assert.ok(p.find('.card').every(card=>p.all(card).filter(n=>n.tag==='img').every(img=>img.src.includes('/thumb'))));
  const viewport=p.find('.viewport')[0];viewport.scrollTop=15000;viewport.events.scroll[0]();p.flush();
  assert.ok(p.find('.card').length<40);assert.ok(initial.slice(0,-1).every(img=>!img.src));
  viewport.clientWidth=480;p.resizes[0].fire();assert.equal(p.find('.tiles')[0].style.gridTemplateColumns,'repeat(2,minmax(0,1fr))');
  assert.equal(p.find('h1')[0].textContent,'<script>plain text</script>');
});
test('selection remains indexed to the full album across recycling and side panel updates',()=>{
  const p=boot();p.button('清空').click();assert.equal(p.sent.at(-1).kind,'material_selection');assert.equal(p.sent.at(-1).indices.length,0);
  p.find('.media')[2].click();assert.deepEqual(p.sent.at(-1).indices,[2]);
  p.api.setSelection([0,119]);assert.equal(p.find('.count')[0].textContent,'已选 2 / 120');
  const viewport=p.find('.viewport')[0];viewport.scrollTop=11000;viewport.events.scroll[0]();p.flush();
  p.button('反选').click();assert.equal(p.sent.at(-1).indices.length,118);assert.equal(p.sent.at(-1).indices.includes(119),false);
});
test('downloads use selected original indices and LIVE configuration without loading high-resolution sources',()=>{
  const p=boot(4);p.api.setSelection([1,3]);p.button('下载选中 2 张').click();
  const request=p.sent.at(-1);assert.equal(request.kind,'material_download');assert.deepEqual(request.indices,[1,3]);assert.equal(request.include_live,false);
  assert.equal(p.button('正在加入队列…').disabled,true);p.api.result(true,'queued');assert.equal(p.find('.feedback')[0].textContent,'queued');
  const live=p.find('.live-option')[0].children[0];live.checked=true;live.onchange();assert.deepEqual(p.sent.at(-1),{kind:'material_live',value:true});
  p.button('下载选中 2 张').click();assert.equal(p.sent.at(-1).include_live,true);
  assert.ok(p.find('img').every(img=>!img.src||!img.src.includes('/hq')));
});
test('preview loads one web image and releases its source on closing and note changes',()=>{
  const p=boot(4);p.find('.zoom')[1].click();assert.equal(p.find('.preview-image')[0].src,p.note.assets[1].url);assert.equal(p.find('.preview-image')[0].src,p.note.assets[1].url);
  p.button('下一张').click();assert.equal(p.find('.preview-image')[0].src,p.note.assets[2].url);assert.equal(p.api.closePreview(),true);assert.equal(p.find('.preview-image')[0].src,undefined);assert.equal(p.api.closePreview(),false);
  p.find('.zoom')[0].click();const image=p.find('.preview-image')[0],observer=p.resizes[0];p.note={id:'',assets:[]};p.api.update(p.note);
  assert.equal(p.host,undefined);assert.equal(image.src,undefined);assert.equal(observer.disconnected,true);assert.equal(p.mask.inert,false);assert.equal('data-sd-suspended' in p.mask.attrs,false);
});
test('native carousel is reversible and videos or unknown posts keep their original page',()=>{
  const p=boot(4);p.button('轮播视图').click();assert.equal(p.host,undefined);assert.equal(p.mask.inert,false);
  const launcher=p.document.body.children.find(n=>n.id==='stilldock-material-launcher');assert.ok(launcher);launcher.click();assert.equal(p.api.isOpen(),true);
  p.api.setConfig({enabled:false});assert.equal(p.api.isOpen(),false);assert.equal(p.document.body.children.some(n=>n.id==='stilldock-material-launcher'),false);
  p.note={...p.note,assets:[{url:'https://sns-video-bd.xhscdn.com/v',video:true}]};p.api.setConfig({enabled:true});assert.equal(p.api.isOpen(),false);
});
test('stale page data cannot submit previously selected images and back does not reopen the overlay',()=>{
  const p=boot(4);const before=p.sent.filter(s=>s.kind==='material_download').length;
  p.note={...p.note,assets:[{url:'https://sns-img-bd.xhscdn.com/changed',sequence:9}]};p.button('下载选中 4 张').click();assert.equal(p.sent.filter(s=>s.kind==='material_download').length,before);
  p.api.prepareBack();p.api.update(p.note);assert.equal(p.api.isOpen(),false);
  p.note={id:'',assets:[]};p.api.update(p.note);p.note={id:'abcdef0123456789abcdef02',assets:[{url:'https://sns-img-bd.xhscdn.com/new'}]};p.api.update(p.note);assert.equal(p.api.isOpen(),true);
});
test('login or verification dialogs take precedence over the material workspace',()=>{
  const p=boot(4);const dialog={closest:()=>null,getClientRects:()=>[{}]};p.document.querySelectorAll=()=>[dialog];p.api.update(p.note);assert.equal(p.api.isOpen(),false);assert.equal(p.mask.inert,false);
  p.document.querySelectorAll=()=>[];p.api.update(p.note);assert.equal(p.api.isOpen(),true);
});

test('per-card state filtering never changes original indices and LIVE missing can be selected',()=>{
  const p=boot(4);p.api.setStates({note_id:p.note.id,expected_urls:p.note.assets.map(a=>a.url),assets:[{index:0,state:'present',live_state:'new'},{index:1,state:'missing'},{index:2,state:'queued'},{index:3,state:'unavailable'}]});
  assert.deepEqual(p.find('.file-state').map(n=>n.textContent),['本地存在','文件缺失','已排队','暂时无法访问']);
  p.find('.open')[0].click();assert.deepEqual(p.sent.at(-1),{kind:'material_open',index:0});
  const filter=p.find('select')[0];filter.value='needed';filter.onchange();assert.deepEqual(p.find('.card').map(n=>Number(n.dataset.index)),[1]);
  p.button('下载选中 1 张').click();assert.deepEqual(p.sent.at(-1).indices,[1]);p.api.result(true,'queued');
  p.api.setConfig({includeLive:true});p.button('选中缺失项').click();assert.deepEqual(p.sent.at(-1).indices,[0,1]);p.button('轮播视图').click();p.document.body.children.find(n=>n.id==='stilldock-material-launcher').click();assert.deepEqual(p.find('.card').map(n=>Number(n.dataset.index)),[0,1]);
});
test('source signature refresh preserves selected images and rejects stale status responses',()=>{
  const p=boot(3);p.button('清空').click();p.find('.media')[1].click();const old=p.note.assets.map(a=>a.url);
  p.note={...p.note,assets:p.note.assets.map(a=>({...a,url:a.url+'?sig=new'}))};p.api.update(p.note);
  assert.equal(p.find('.count')[0].textContent,'已选 1 / 3');assert.equal(p.find('.card')[1].children[1].children[0].children[0].checked,true);
  p.api.setStates({note_id:p.note.id,expected_urls:old,assets:[{index:0,state:'present'}]});assert.equal(p.find('.file-state')[0].textContent,'核对中…');
  const force=p.find('.live-option').find(label=>label.children.some(n=>n.textContent==='重新下载已有图')).children[0];force.checked=true;p.button('下载选中 1 张').click();assert.equal(p.sent.at(-1).force,true);assert.deepEqual(p.sent.at(-1).indices,[1]);
});

test('preview backdrop dismisses one layer while image drags and dialog padding do not',()=>{
  const p=boot(3);p.find('.zoom')[0].click();const dialog=p.find('dialog')[0];dialog.getBoundingClientRect=()=>({left:100,right:900,top:100,bottom:700});
  const emit=(type,x,target=dialog)=>{for(const fn of dialog.events[type]||[])fn({button:0,clientX:x,clientY:300,target,preventDefault(){},stopPropagation(){}});};
  emit('pointerdown',300,p.find('.preview-image')[0]);emit('pointerup',20);assert.equal(dialog.open,true);
  emit('pointerdown',300);emit('pointerup',300);assert.equal(dialog.open,true);
  emit('pointerdown',20);emit('pointerup',20);assert.equal(dialog.open,false);assert.equal(p.api.isOpen(),true);
});
