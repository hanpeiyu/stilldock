const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const html=fs.readFileSync(path.join(__dirname,'../assets/panel.html'),'utf8');
function panel(){
  const elements=new Map(),sent=[];
  function element(tag='div'){
    const e={tag,children:[],dataset:{},style:{},value:'',checked:false,hidden:false,classList:{toggle(){}},setAttribute(k,v){this[k]=v},removeAttribute(k){delete this[k];if(k==='data-src')delete this.dataset.src},addEventListener(){},replaceChildren(...items){this.children=items},append(...items){this.children.push(...items)},remove(){},showModal(){this.open=true},close(){this.open=false}};
    e.querySelectorAll=s=>{const found=[];function visit(node){for(const child of node.children||[]){if(typeof child==='object'){if(s==='input'&&child.tag==='input'||s==='input:checked'&&child.tag==='input'&&child.checked||s==='img[data-src]'&&child.tag==='img'&&child.dataset.src)found.push(child);visit(child);}}}visit(e);return found};return e;
  }
  const document={getElementById(id){if(!elements.has(id))elements.set(id,element());return elements.get(id)},createElement:element,createTextNode:s=>s,querySelectorAll:()=>[],body:element()};
  const window={ipc:{postMessage:raw=>sent.push(JSON.parse(raw))}};
  vm.runInNewContext(html.match(/<script>([\s\S]*?)<\/script>/)[1],{window,document,setTimeout:()=>1,clearTimeout(){}});
  const snapshot=note=>window.receive({kind:'snapshot',jobs:[],note,collapsed:false,folder:'D:/Downloads',focus:true,ai:true,comments:true,original:false,about:'test'});
  return {elements,sent,snapshot,receive:window.receive};
}
test('download stays disabled on the feed but can retry extraction for an opened note',()=>{
  const p=panel();p.snapshot({});assert.equal(p.elements.get('download').disabled,true);
  p.snapshot({id:'abcdef0123456789abcdef01',title:'note',assets:[]});
  const button=p.elements.get('download');assert.equal(button.disabled,false);button.onclick();
  assert.equal(button.disabled,true);assert.equal(p.sent.at(-1).action,'download');assert.equal(p.sent.at(-1).indices,null);
  p.snapshot({id:'abcdef0123456789abcdef01',assets:[]});assert.equal(button.disabled,false);
});
test('partial selection requires at least one image before sending a task',()=>{
  const p=panel();p.snapshot({id:'abcdef0123456789abcdef01',assets:[{url:'https://sns-img-bd.xhscdn.com/a'},{url:'https://sns-img-bd.xhscdn.com/b'}]});
  p.elements.get('pick-some').checked=true;p.elements.get('select-none').onclick();
  const count=p.sent.length;p.elements.get('download').onclick();assert.equal(p.sent.length,count);
  p.elements.get('asset-picks').querySelectorAll=()=>[{value:'1'}];p.elements.get('download').onclick();
  assert.deepEqual(p.sent.at(-1),{action:'download',indices:[1],include_live:false,note_id:'abcdef0123456789abcdef01',expected_urls:['https://sns-img-bd.xhscdn.com/a','https://sns-img-bd.xhscdn.com/b']});
});

test('thumbnails load on demand, retain selection on progress updates and release preview on note change',()=>{
  const p=panel(),note={id:'abcdef0123456789abcdef01',assets:[{url:'https://sns-img-bd.xhscdn.com/full1',preview:'https://sns-img-bd.xhscdn.com/small1'},{url:'https://sns-img-bd.xhscdn.com/full2'}]};
  p.snapshot(note);
  const grid=p.elements.get('asset-picks'),card=grid.children[0],image=card.children[0].children[1];
  assert.equal(image.src,undefined);assert.equal(grid.hidden,true);
  const toggle=p.elements.get('pick-some');toggle.checked=true;toggle.onchange();
  assert.equal(image.src,note.assets[0].preview);assert.equal(grid.hidden,false);
  p.elements.get('select-none').onclick();assert.equal(p.elements.get('pick-count').textContent,'已选 0 / 2');
  p.snapshot(note);assert.equal(grid.children[0],card);assert.equal(grid.querySelectorAll('input:checked').length,0);
  card.children[1].onclick();assert.equal(p.elements.get('preview-image').src,note.assets[0].url);
  p.snapshot({id:'other',assets:[]});assert.equal(p.elements.get('preview-image').src,undefined);assert.equal(grid.children.length,0);
});

test('LIVE selection is opt-in and new notes return from history to the download tab',()=>{
  const p=panel();p.snapshot({id:'abcdef0123456789abcdef01',assets:[{url:'https://sns-img-bd.xhscdn.com/still',live_url:'https://sns-video-bd.xhscdn.com/motion',width:4032,height:3024}]});
  assert.equal(p.elements.get('live-option').hidden,false);assert.equal(p.elements.get('include-live').checked,false);
  p.elements.get('include-live').checked=true;p.elements.get('download').onclick();assert.equal(p.sent.at(-1).include_live,true);
  p.receive({kind:'current_note'});assert.equal(p.elements.get('downloads').hidden,false);assert.equal(p.elements.get('history').hidden,true);assert.equal(p.elements.get('downloads').scrollTop,0);
});

test('LIVE control stays visible before opening a note and saves its preference',()=>{
  const p=panel();p.snapshot({});assert.equal(p.elements.get('live-option').hidden,false);
  const toggle=p.elements.get('include-live');toggle.checked=true;toggle.onchange();assert.deepEqual(p.sent.at(-1),{action:'setting',key:'include_live',value:true});
  p.snapshot({id:'abcdef0123456789abcdef01',assets:[]});assert.equal(toggle.checked,true);assert.equal(p.elements.get('quality').textContent,'');
});

test('save layout and optional JSON controls send typed settings and show defaults',()=>{
  const p=panel();p.snapshot({});assert.equal(p.elements.get('output_layout').value,'note');assert.equal(p.elements.get('export_json').checked,false);
  const layout=p.elements.get('output_layout');layout.value='flat';layout.onchange();assert.deepEqual(p.sent.at(-1),{action:'setting',key:'output_layout',value:'flat'});
  const json=p.elements.get('export_json');json.checked=true;json.onchange();assert.deepEqual(p.sent.at(-1),{action:'setting',key:'export_json',value:true});
});
