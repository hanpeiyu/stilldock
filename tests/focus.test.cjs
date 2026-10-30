const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
test('known note layouts collapse information, expand on demand and restore in original mode',()=>{
  function node(){const attrs=new Set();return {attrs,isConnected:true,children:[],style:{setProperty(){}},classList:{add(){}},setAttribute(k){attrs.add(k)},removeAttribute(k){attrs.delete(k)},toggleAttribute(k,value){const set=value===undefined?!attrs.has(k):value;if(set)attrs.add(k);else attrs.delete(k);return set},querySelector:()=>null,append(...children){this.children.push(...children)},remove(){this.isConnected=false}};}
  const root=node(),body=node(),host=node(),media=node(),info=node();
  host.querySelector=s=>s==='.media-container'?media:s==='.interaction-container'?info:null;
  const document={body,documentElement:root,createElement:node,querySelector:()=>null,querySelectorAll:s=>s==='#noteContainer,.note-container'?[host]:[]};
  const window={__xhsReadNote:()=>({id:'abcdef0123456789abcdef01',title:'sample',author:'author'})};
  const source=fs.readFileSync(path.join(__dirname,'../assets/focus.js'),'utf8').replace('__CONFIG__',JSON.stringify({enabled:true,foldComments:false}));
  vm.runInNewContext(source,{window,document,location:{hostname:'www.xiaohongshu.com',pathname:'/explore/abcdef0123456789abcdef01'},getComputedStyle:()=>({backgroundColor:'#18181b'}),MutationObserver:function(){this.observe=()=>{}},setTimeout(){}});
  assert.ok(info.attrs.has('data-xhs-note-info'));assert.ok(host.attrs.has('data-xhs-material-note'));assert.equal(host.attrs.has('data-xhs-info-open'),false);
  const bar=host.children[0],button=bar.children[1];button.onclick();assert.ok(host.attrs.has('data-xhs-info-open'));
  window.__xhsFocusMode.update({enabled:false,foldComments:false});assert.equal(info.attrs.size,0);assert.equal(media.attrs.size,0);assert.equal(host.attrs.size,0);assert.equal(bar.isConnected,false);
});
