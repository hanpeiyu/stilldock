const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const script=fs.readFileSync(require('node:path').join(__dirname,'../assets/site.js'),'utf8');
const id='abcdef0123456789abcdef01';
function read(path,{modal=false,stateId=id,mapKey=id,wrapped=false,images,current=false,slideIndex,activeSrc,counter,hiddenMedia=false,hiddenModal=false}={}){
  const location=new URL('https://www.xiaohongshu.com'+path);
  const window={ipc:{postMessage(){}},__INITIAL_STATE__:{note:{currentNoteId:{value:stateId},noteDetailMap:{[mapKey]:{note:{title:'测试图集',imageList:[{urlDefault:'http://sns-img-bd.xhscdn.com/image'}]}}}}}};
  if(images)window.__INITIAL_STATE__.note.noteDetailMap[mapKey].note.imageList=images;
  if(wrapped){const note=window.__INITIAL_STATE__.note;note.noteDetailMap={value:note.noteDetailMap};window.__INITIAL_STATE__.note={value:note};}
  window.top=window;window.window=window;
  const active={getAttribute:()=>slideIndex===undefined?null:String(slideIndex),querySelector:()=>activeSrc?{currentSrc:activeSrc}:null};
  const media={getClientRects:()=>hiddenMedia?[]:[{}],querySelector:s=>s==='.swiper-slide-active'&&(slideIndex!==undefined||activeSrc)?active:s==='.swiper-pagination-current'&&counter?{textContent:String(counter)}:null};
  const document={readyState:'loading',title:'小红书',querySelector:s=>current&&s.includes('.media-container')?media:modal&&s.includes('#noteContainer')?(hiddenModal?{getClientRects:()=>[]}:{}):null,addEventListener(){}};
  vm.runInNewContext(script,{window,document,location,URL,getComputedStyle:()=>({visibility:'visible'}),history:{pushState(){},replaceState(){}},addEventListener(){},setTimeout(){}});
  return current?window.__xhsReadCurrentImage():window.__xhsReadNote();
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

test('extracts a full image candidate separately from the preview and chooses the best LIVE stream',()=>{
  const note=read('/explore/'+id,{images:[{urlDefault:'https://sns-webpic-qc.xhscdn.com/date/sign/1040abc!nd_dft_wlteh_webp_3',urlPre:'https://sns-img-bd.xhscdn.com/preview',width:4032,height:3024,stream:{h264:[{width:720,height:1280,masterUrl:'https://sns-video-bd.xhscdn.com/low'},{width:1080,height:1920,masterUrl:'http://sns-video-bd.xhscdn.com/high'}]}}]});
  const asset=note.assets[0];assert.equal(asset.original_url,'https://ci.xiaohongshu.com/1040abc?imageView2/format/jpeg');assert.equal(asset.preview,'https://sns-img-bd.xhscdn.com/preview');assert.equal(asset.live_url,'https://sns-video-bd.xhscdn.com/high');assert.equal(asset.width,4032);
});
test('unknown image hosts are never rewritten and untrusted LIVE URLs are rejected',()=>{
  const note=read('/explore/'+id,{images:[{urlDefault:'https://unknown.xhscdn.com/a!b',stream:{h264:[{masterUrl:'https://evil.example/video'}]}}]});
  assert.equal(note.assets[0].original_url,'');assert.equal(note.assets[0].live_url,'');
});

test('current-image extraction handles looped Swiper indices, image identity and counters',()=>{
  const images=[{urlDefault:'https://sns-img-bd.xhscdn.com/first!small'},{urlDefault:'https://sns-img-bd.xhscdn.com/second!small'},{urlDefault:'https://sns-img-bd.xhscdn.com/third!small'}];
  assert.equal(read('/explore/'+id,{images,current:true,slideIndex:2}).index,2);
  assert.equal(read('/explore/'+id,{images,current:true,activeSrc:'https://sns-img-bd.xhscdn.com/second!preview'}).index,1);
  assert.equal(read('/explore/'+id,{images,current:true,counter:3}).index,2);
  assert.match(read('/explore/'+id,{images,current:true}).error,/无法确认/);
  assert.equal(read('/explore/'+id,{current:true}).index,0);
});

test('a hidden or closing media container never yields a stale current image',()=>{
  assert.match(read('/explore/'+id,{current:true,hiddenMedia:true}).error,/打开/);
});

test('missing media does not shift original image numbers or select the wrong slide',()=>{
  const images=[{urlDefault:''},{urlDefault:'https://sns-img-bd.xhscdn.com/second'}];
  const active=read('/explore/'+id,{images,current:true,slideIndex:1});assert.equal(active.index,0);assert.equal(active.note.assets[0].sequence,2);
  assert.match(read('/explore/'+id,{images,current:true,slideIndex:0}).error,/无法确认/);
});

test('a retained but display-hidden native modal cannot keep a stale material note open',()=>{
  assert.equal(read('/search_result?keyword=test',{modal:true,hiddenModal:true}).id,'');
});
