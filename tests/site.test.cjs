const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const script=fs.readFileSync(require('node:path').join(__dirname,'../assets/site.js'),'utf8');
const id='abcdef0123456789abcdef01';
function read(path,{modal=false,stateId=id,mapKey=id,wrapped=false}={}){
  const location=new URL('https://www.xiaohongshu.com'+path);
  const window={ipc:{postMessage(){}},__INITIAL_STATE__:{note:{currentNoteId:{value:stateId},noteDetailMap:{[mapKey]:{note:{title:'测试图集',imageList:[{urlDefault:'http://sns-img-bd.xhscdn.com/image'}]}}}}}};
  if(wrapped){const note=window.__INITIAL_STATE__.note;note.noteDetailMap={value:note.noteDetailMap};window.__INITIAL_STATE__.note={value:note};}
  window.top=window;window.window=window;
  const document={readyState:'loading',title:'小红书',querySelector:s=>modal&&s.includes('#noteContainer')?{}:null,addEventListener(){}};
  vm.runInNewContext(script,{window,document,location,URL,history:{pushState(){},replaceState(){}},addEventListener(){},setTimeout(){}});
  return window.__xhsReadNote();
}
test('extracts notes from explore, discovery and search result routes',()=>{
  for(const path of [`/explore/${id}`,`/discovery/item/${id}`,`/search_result/${id}?xsec_token=abc`]){
    const note=read(path);assert.equal(note.id,id);assert.equal(note.assets[0].url,'https://sns-img-bd.xhscdn.com/image');
  }
});
test('modal state resolves a canonical source and preserves access query',()=>{
  const note=read('/search_result?keyword=test&xsec_token=abc',{modal:true});
  assert.equal(note.id,id);assert.equal(new URL(note.url).pathname,'/explore/'+id);
  assert.equal(new URL(note.url).searchParams.get('xsec_token'),'abc');
});
test('closed modal does not expose a stale note',()=>{
  assert.equal(read('/search_result?keyword=test').id,'');
  assert.equal(read('/explore',{modal:true,stateId:'../../bad'}).id,'');
});

test('unwraps Vue state and the legacy -1 detail entry',()=>{
  const note=read('/explore/'+id,{wrapped:true,mapKey:'-1'});
  assert.equal(note.id,id);assert.equal(note.assets.length,1);
});
test('reads query note IDs only while a note modal is open',()=>{
  const path='/search_result?noteId='+id;
  assert.equal(read(path,{modal:true,stateId:''}).id,id);
  assert.equal(read(path,{stateId:''}).id,'');
});
