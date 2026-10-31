const {test}=require('node:test');const assert=require('node:assert/strict');const vm=require('node:vm');const fs=require('node:fs');
const source=fs.readFileSync(require('node:path').join(__dirname,'../assets/search-links.js'),'utf8');
function boot(){
  const sent=[],listeners={};const window={ipc:{postMessage:raw=>sent.push(JSON.parse(raw))}};window.top=window;
  const location=new URL('https://www.xiaohongshu.com/search_result?keyword=old');
  vm.runInNewContext(source,{window,location,URL,addEventListener:(key,fn)=>listeners[key]=fn});
  function node(text='',parent=null,href=null){const n={textContent:text,parentElement:parent,getAttribute:key=>key==='href'?href:null,closest(selector){if(selector==='a[href]')return href===null?null:this;if(selector.includes('input'))return null;if(selector.includes('/explore/'))return href?.includes('/explore/')?this:null;return null;},querySelectorAll:()=>[]};return n;}
  const card=node('大家都在搜 下水道连接器 下水管地漏接口');card.querySelectorAll=selector=>selector.startsWith('a[')?[]:[node('大家都在搜')];const row=node('下水道连接器',card),image=node('',row),label=node('下水道连接器',row);
  function click(target,extra={}){let prevented=false;listeners.click({button:0,target,preventDefault(){prevented=true;},stopImmediatePropagation(){},...extra});return prevented;}
  return {window,sent,card,row,image,label,node,click};
}
test('related card text and image clicks become ordinary keyword searches',()=>{
  for(const target of ['image','label']){const p=boot();assert.equal(p.click(p[target]),true);assert.deepEqual(p.sent.at(-1),{kind:'keyword_search',keyword:'下水道连接器'});}
});
test('search links drop AI parameters but unrelated note links and headings are untouched',()=>{
  const p=boot();p.click(p.node('related',p.card,'/search_result?keyword=%E6%B3%95%E5%BC%8F&source=ai'));assert.deepEqual(p.sent.at(-1),{kind:'keyword_search',keyword:'法式'});
  const before=p.sent.length;assert.equal(p.click(p.node('note',p.card,'/explore/abcdef0123456789abcdef01')),false);assert.equal(p.click(p.node('大家都在搜',p.card)),false);assert.equal(p.click(p.node('ordinary text')),false);assert.equal(p.click(p.node('用户',null,'/search_result?keyword=room&type=user')),false);assert.equal(p.sent.length,before);
});
test('modified clicks and non-left buttons keep native behavior',()=>{
  const p=boot();p.click(p.label,{ctrlKey:true});p.click(p.label,{button:2});assert.equal(p.sent.length,0);
});
