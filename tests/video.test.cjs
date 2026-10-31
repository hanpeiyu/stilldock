const {test}=require('node:test');const assert=require('node:assert/strict');const vm=require('node:vm');const fs=require('node:fs');
const source=fs.readFileSync(require('node:path').join(__dirname,'../assets/video.js'),'utf8');
function boot(){
  const sent=[],timers=new Map();let serial=0;
  function element(tag='div'){const node={tag,children:[],events:{},attrs:{},checked:false,hidden:false,append(...items){for(const child of items){child.parent=this;this.children.push(child);}},remove(){if(this.parent){this.parent.children=this.parent.children.filter(c=>c!==this);this.parent=null;}},setAttribute(k,v){this.attrs[k]=v;},removeAttribute(k){delete this.attrs[k];},addEventListener(k,fn){this.events[k]=fn;},attachShadow(){this.shadowRoot=element('shadow');return this.shadowRoot;}};Object.defineProperty(node,'isConnected',{get(){return node===document.body||!!node.parent?.isConnected;}});return node;}
  const document={readyState:'complete',body:element('body'),head:element('head'),documentElement:element('html'),createElement:element,createTextNode:text=>({textContent:text})};
  let note={id:'abcdef0123456789abcdef01',url:'https://www.xiaohongshu.com/explore/abcdef0123456789abcdef01',media_type:'video',assets:[{url:'https://sns-video-bd.xhscdn.com/movie',video:true}]};
  const window={ipc:{postMessage:raw=>sent.push(JSON.parse(raw))},__xhsReadNote:()=>note};window.top=window;
  vm.runInNewContext(source,{window,document,location:new URL(note.url),setTimeout:fn=>{const id=++serial;timers.set(id,fn);return id;},clearTimeout:id=>timers.delete(id),addEventListener(){}});
  function all(node){return node.children.flatMap(child=>[child,...all({...child,children:child.children||[]})]);}
  return {sent,window,document,get note(){return note;},set note(value){note=value;},get host(){return document.body.children.find(n=>n.id==='stilldock-video-actions');},find(tag){return all(this.host.shadowRoot).filter(n=>n.tag===tag);}};
}
test('video has an in-page download action using the existing queue and scoped feedback',()=>{
  const p=boot();assert.equal(p.window.__stillDockVideo.isOpen(),true);assert.equal(p.find('button')[0].textContent,'下载视频');p.find('input')[0].checked=true;p.find('button')[0].onclick();
  assert.equal(p.sent.at(-1).kind,'material_download');assert.deepEqual(p.sent.at(-1).indices,[0]);assert.equal(p.sent.at(-1).force,true);assert.equal(p.find('button')[0].disabled,true);
  p.window.__stillDockVideo.result(true,'queued','wrong-note');assert.equal(p.find('button')[0].disabled,true);p.window.__stillDockVideo.result(true,'queued',p.note.id);assert.equal(p.find('button')[0].disabled,false);
});
test('unresolved video cannot submit and switching to images releases the toolbar',()=>{
  const p=boot();p.note={...p.note,assets:[]};p.window.__stillDockVideo.update(p.note);assert.equal(p.find('button')[0].disabled,true);
  p.note={id:p.note.id,assets:[{url:'https://sns-img-bd.xhscdn.com/photo',video:false}]};p.window.__stillDockVideo.update(p.note);assert.equal(p.window.__stillDockVideo.isOpen(),false);assert.equal(p.host,undefined);
});
