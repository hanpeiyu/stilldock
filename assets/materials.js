// A bounded, disposable image workspace inside the existing WebView. No second browser.
(() => {
  if (window.top !== window || !/(^|\.)xiaohongshu\.com$/.test(location.hostname) || window.__stillDockMaterial) return;
  let config=__MATERIAL_CONFIG__,note=null,signature='',host,shadow,viewport,spacer,tiles,preview,previewImage,previewTitle,previewDownload,previewStatus,previewFeedback;
  let headerTitle,author,count,download,liveInput,liveLabel,feedback,launcher,previousFocus,forceInput,filterInput;
  let states=[],visibleIndices=[],filterMode='all',selection=new Set(),cards=new Map(),suspended=new Map(),imageObserver,resizeObserver,frame,sourceFrame;
  let rowHeight=280,columns=3,firstRow=-1,lastRow=-1,previewIndex=null,optedOut='',returning='',pending=false,pendingTimer;
  const send=value=>window.ipc.postMessage(JSON.stringify(value));
  const eligible=value=>!!value?.id&&Array.isArray(value.assets)&&value.assets.length>0&&value.assets.length<=400&&value.assets.every(a=>!a.video);
  const identity=raw=>{try{const url=new URL(raw),parts=url.pathname.split('/').filter(Boolean);return (/^sns-webpic-/.test(url.hostname)?parts.slice(2):parts).join('/').split('!')[0];}catch{return '';}};
  const sameImage=(a,b)=>(!a.sequence||!b.sequence||a.sequence===b.sequence)&&[a.url,a.original_url].filter(Boolean).some(u=>[b.url,b.original_url].filter(Boolean).some(v=>identity(u)&&identity(u)===identity(v)));
  const key=value=>value?.id+'|'+JSON.stringify((value?.assets||[]).map(a=>[a.sequence,a.url,a.preview,a.live_url]));
  const element=(tag,cls,text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  function button(text,action,cls=''){const node=element('button',cls,text);node.type='button';node.onclick=action;return node;}
  function active(){return !!host?.isConnected;}
  function releaseBoot(){document.documentElement?.removeAttribute('data-sd-material-pending');}
  function restoreNative(){for(const [node,record] of suspended){node.removeAttribute('data-sd-suspended');node.inert=record.inert;}suspended.clear();}
  function suspendNative(){
    const container=document.querySelector('#noteContainer,.note-container');
    const nodes=[container?.closest('.note-detail-mask')||container].filter(Boolean);
    for(const [node,record] of suspended)if(!nodes.includes(node)){node.removeAttribute('data-sd-suspended');node.inert=record.inert;suspended.delete(node);}
    for(const node of nodes){if(!suspended.has(node)){suspended.set(node,{inert:node.inert});node.setAttribute('data-sd-suspended','');node.inert=true;}for(const video of node.querySelectorAll('video'))if(!video.paused)video.pause();}
  }
  function releaseCard(card){imageObserver?.unobserve(card.image);card.image.onload=card.image.onerror=null;card.image.removeAttribute('src');card.node.remove();}
  function destroy(){
    if(frame){cancelAnimationFrame(frame);frame=null;}clearTimeout(pendingTimer);pending=false;
    imageObserver?.disconnect();resizeObserver?.disconnect();imageObserver=resizeObserver=null;
    for(const card of cards.values())releaseCard(card);cards.clear();
    if(previewImage){previewImage.onload=previewImage.onerror=null;previewImage.removeAttribute('src');}if(preview?.open)preview.close();
    const wasOpen=active();const hadFocus=document.activeElement===host;host?.remove();host=shadow=viewport=spacer=tiles=preview=previewImage=previewTitle=previewDownload=previewStatus=previewFeedback=null;
    headerTitle=author=count=download=liveInput=liveLabel=feedback=forceInput=filterInput=null;
    previewIndex=null;firstRow=lastRow=-1;restoreNative();if(hadFocus&&previousFocus?.isConnected)previousFocus.focus?.({preventScroll:true});previousFocus=null;releaseBoot();if(wasOpen)send({kind:'workspace',mode:'native',note_id:note?.id||''});
  }
  const stateLabels={new:'未下载',present:'本地存在',missing:'文件缺失',changed:'文件大小变化',unavailable:'暂时无法访问',queued:'已排队'};
  function statusAt(index){const state=states.find(s=>s.index===index);if(!state)return 'checking';if(config.includeLive&&note.assets[index]?.live_url&&state.state==='present'&&state.live_state!=='present')return state.live_state==='queued'?'queued':'live_missing';return state.state;}
  function statusText(index){const state=statusAt(index);return state==='checking'?'核对中…':state==='live_missing'?'静图已存 · LIVE 未齐':stateLabels[state]||'未下载';}
  function rebuildVisible(reset=false){const scroll=viewport?.scrollTop||0,anchor=visibleIndices[Math.floor(scroll/rowHeight)*columns];visibleIndices=(note?.assets||[]).map((_,i)=>i).filter(i=>filterMode==='all'||!['present','queued','checking','unavailable'].includes(statusAt(i)));let changed=false;if(filterMode!=='all'){for(const index of selection)if(!visibleIndices.includes(index)){selection.delete(index);changed=true;}}firstRow=lastRow=-1;if(viewport){const at=visibleIndices.findIndex(i=>i>=anchor);viewport.scrollTop=reset?0:at>=0?Math.floor(at/columns)*rowHeight+scroll%rowHeight:scroll;render(true);}if(changed)selectionChanged();}
  function setStates(value){if(!note||value.note_id!==note.id||JSON.stringify(value.expected_urls)!==JSON.stringify(note.assets.map(a=>a.url)))return;states=value.assets||[];if(filterMode!=='all')rebuildVisible();else{for(const [index,card] of cards){card.status.textContent=statusText(index);card.open.hidden=statusAt(index)!=='present'&&states.find(s=>s.index===index)?.state!=='present';}}}
  function selected(){return [...selection].sort((a,b)=>a-b);}
  function selectionChanged(emit=true){
    if(!active())return;
    const text='已选 '+selection.size+' / '+note.assets.length;
    if(count.textContent!==text)count.textContent=text;
    download.textContent=pending?'正在加入队列…':'下载选中 '+selection.size+' 张';download.disabled=pending||!selection.size;if(previewDownload)previewDownload.disabled=pending;
    for(const [index,card] of cards){card.input.checked=selection.has(index);card.node.classList.toggle('selected',selection.has(index));card.media.setAttribute('aria-pressed',String(selection.has(index)));}
    if(emit)send({kind:'material_selection',note_id:note.id,indices:selected(),expected_urls:note.assets.map(a=>a.url)});
  }
  function setSelection(indices){
    if(!note)return;
    selection=new Set(Array.isArray(indices)?indices.filter(i=>Number.isInteger(i)&&i>=0&&i<note.assets.length):note.assets.map((_,i)=>i));selectionChanged(false);
  }
  function toggle(index){selection.has(index)?selection.delete(index):selection.add(index);selectionChanged();}
  function result(ok,message,expectedId){if(!active()||(expectedId&&expectedId!==note.id))return;clearTimeout(pendingTimer);pending=false;feedback.textContent=message;feedback.setAttribute('role',ok?'status':'alert');if(previewFeedback)previewFeedback.textContent=message;selectionChanged(false);}
  function submit(indices=selected()){
    if(!active()||pending||!indices.length)return;
    const fresh=window.__xhsReadNote?.();
    if(key(fresh)!==signature){result(false,'图片列表已经变化，请重新选择');if(fresh)update(fresh);return;}
    pending=true;feedback.textContent='';if(previewFeedback)previewFeedback.textContent='正在加入下载队列…';selectionChanged(false);
    send({kind:'material_download',note:fresh,indices,include_live:!!config.includeLive,force:!!forceInput?.checked,expected_urls:note.assets.map(a=>a.url)});
    pendingTimer=setTimeout(()=>result(false,'尚未收到队列确认，请查看右侧下载栏'),6000);
  }
  function closePreview(){if(previewIndex===null)return false;previewIndex=null;if(preview?.open)preview.close();previewImage?.removeAttribute('src');return true;}
  function openPreview(index){
    if(!active()||!note.assets[index])return;
    previewIndex=index;const asset=note.assets[index];previewTitle.textContent='图片 '+(asset.sequence||index+1)+' / '+note.assets.length;
    previewStatus.textContent='正在加载预览…';previewFeedback.textContent='';previewImage.alt='图片 '+(asset.sequence||index+1);previewImage.src=asset.url;
    if(!preview.open)preview.showModal();
  }
  function mountCard(index){
    const asset=note.assets[index],node=element('article','card'),media=button('',()=>toggle(index),'media');
    const image=element('img'),placeholder=element('span','placeholder','加载预览…'),input=element('input'),number=element('span','number',String(asset.sequence||index+1));
    input.type='checkbox';input.setAttribute('aria-label','选择图片 '+(asset.sequence||index+1));input.onchange=()=>{input.checked?selection.add(index):selection.delete(index);selectionChanged();};
    image.loading='lazy';image.decoding='async';image.referrerPolicy='no-referrer';image.alt='图片 '+(asset.sequence||index+1);image.dataset.src=asset.preview||asset.url;
    image.onload=()=>{placeholder.hidden=true;};image.onerror=()=>{placeholder.textContent='预览加载失败 · 仍可下载';image.hidden=true;};
    media.setAttribute('aria-label','选择图片 '+(asset.sequence||index+1)+'，双击放大');media.dataset.index=String(index);node.dataset.index=String(index);
    media.ondblclick=event=>{event.preventDefault();openPreview(index);};media.onkeydown=event=>{if(event.key==='Enter'){event.preventDefault();openPreview(index);}};
    media.append(placeholder,image,number);if(asset.live_url)media.append(element('span','live','LIVE'));
    const foot=element('div','card-foot'),label=element('label');label.append(input,element('span','',String(asset.sequence||index+1).padStart(2,'0')));
    const size=element('span','size',asset.width&&asset.height?asset.width+' × '+asset.height:'');size.title='网页标注尺寸';
    const status=element('div','file-state',statusText(index)),open=button('位置',()=>send({kind:'material_open',index}),'open');open.hidden=states.find(s=>s.index===index)?.state!=='present';foot.append(label,size,open,button('放大',()=>openPreview(index),'zoom'));node.append(media,foot,status);
    tiles.append(node);const card={node,image,input,media,status,open};cards.set(index,card);
    if(imageObserver)imageObserver.observe(image);else {image.src=image.dataset.src;delete image.dataset.src;}
    return card;
  }
  function render(force=false){
    frame=null;if(!active())return;
    const width=viewport.clientWidth;const cols=Math.max(1,Math.min(5,Math.floor((width-32)/220)));
    const tileWidth=Math.max(150,(width-32-(cols-1)*16)/cols),height=Math.min(360,Math.max(210,tileWidth*1.05))+76;
    const rows=Math.ceil(visibleIndices.length/cols),start=Math.max(0,Math.floor(viewport.scrollTop/height)-2),end=Math.min(rows,Math.ceil((viewport.scrollTop+viewport.clientHeight)/height)+2);
    if(!force&&columns===cols&&rowHeight===height&&firstRow===start&&lastRow===end)return;
    columns=cols;rowHeight=height;firstRow=start;lastRow=end;
    spacer.style.height=(rows*height)+'px';tiles.style.gridTemplateColumns='repeat('+cols+',minmax(0,1fr))';tiles.style.gridAutoRows=(height-16)+'px';tiles.style.transform='translateY('+(start*height)+'px)';
    const from=start*cols,to=Math.min(visibleIndices.length,end*cols),wanted=new Set(visibleIndices.slice(from,to));
    for(const [index,card] of cards)if(!wanted.has(index)){releaseCard(card);cards.delete(index);}
    for(let position=from;position<to;position++){const index=visibleIndices[position],card=cards.get(index)||mountCard(index);const at=position-from;if(tiles.children[at]!==card.node)tiles.insertBefore(card.node,tiles.children[at]||null);}
    selectionChanged(false);
  }
  function schedule(){if(active()&&!frame)frame=requestAnimationFrame(()=>render());}
  function updateLauncher(){
    const show=!!document.body&&config.enabled&&eligible(note)&&!active()&&optedOut===note.id;
    if(!show){launcher?.remove();launcher=null;return;}
    if(!launcher){launcher=button('素材网格',()=>{optedOut='';update(note,true);});launcher.id='stilldock-material-launcher';document.body.append(launcher);}
  }
  function mount(){
    previousFocus=document.activeElement;host=element('div');host.id='stilldock-material';host.setAttribute('role','dialog');host.setAttribute('aria-modal','true');host.setAttribute('aria-label','帖子素材视图');shadow=host.attachShadow({mode:'open'});
    const style=element('style');style.textContent=`
      select{background:#28282e;color:#eee;border:1px solid #45454e;border-radius:7px;padding:5px;font:inherit}
      :host{position:fixed!important;inset:0!important;z-index:2147483000!important;display:block!important;color:#ededf0;background:#18181b;isolation:isolate;font:13px/1.6 'Segoe UI','Microsoft YaHei',sans-serif;color-scheme:dark}
      *{box-sizing:border-box}button,input{font:inherit}button{border:1px solid #45454e;border-radius:8px;padding:7px 12px;color:#ededf0;background:#28282e;cursor:pointer}button:hover{background:#35353d}button:disabled{opacity:.5;cursor:default}button:focus-visible,input:focus-visible{outline:2px solid #ff4568;outline-offset:3px}[hidden]{display:none!important}
      .workspace{height:100%;display:flex;flex-direction:column}.header{flex:none;padding:64px 24px 14px;border-bottom:1px solid #303036;background:#1b1b1f}.heading{display:flex;gap:16px;align-items:start}.metadata{flex:1;min-width:0}h1{margin:0;font-size:18px;line-height:1.6;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.author{margin-top:3px;color:#a3a3b0;font-size:12px}.tools{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin-top:12px}.tools .count{margin-left:auto;color:#b9b9c3;white-space:nowrap}.tools button{font-size:12px;padding:5px 10px}
      .viewport{flex:1;min-height:0;overflow:auto;overscroll-behavior:contain;scrollbar-gutter:stable;scrollbar-width:thin;scrollbar-color:#4a4a54 transparent}.viewport::-webkit-scrollbar{width:6px}.viewport::-webkit-scrollbar-thumb{background:#4a4a54;border-radius:5px}.spacer{position:relative;margin:16px}.tiles{position:absolute;left:0;right:0;top:0;display:grid;gap:16px}.card{min-width:0;overflow:hidden;border:1px solid #3c3c45;border-radius:12px;background:#222227;display:flex;flex-direction:column}.card.selected{border-color:#e44a68;box-shadow:inset 0 0 0 1px #e44a68}.media{position:relative;flex:1;min-height:0;width:100%;padding:0;border:0;border-radius:0;background:#111114;overflow:hidden;display:flex;align-items:center;justify-content:center}.media:hover{background:#111114}.media img{position:absolute;inset:0;width:100%;height:100%;object-fit:contain}.placeholder{color:#8f8f9e;font-size:12px;padding:10px}.number,.live{position:absolute;border-radius:5px;background:#18181bdd;padding:2px 6px;font-size:11px;z-index:1}.number{left:9px;top:9px}.live{right:9px;top:9px;color:#e7cfab}.file-state{height:22px;flex:none;padding:0 10px 5px;color:#a6b8a4;font-size:10px}.open{font-size:10px;padding:3px 5px}.card-foot{display:flex;gap:6px;align-items:center;height:42px;padding:0 9px;font-size:11px;flex:none}.card-foot label{display:flex;align-items:center;gap:5px;cursor:pointer}.card-foot input,.live-option input{accent-color:#ed3255;width:16px;height:16px;margin:0}.size{color:#9898a7;font-size:10px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.zoom{margin-left:auto;font-size:11px;padding:3px 7px;flex:none}
      .footer{flex:none;display:flex;align-items:center;gap:12px;flex-wrap:wrap;padding:12px 24px;border-top:1px solid #303036;background:#1b1b1f}.live-option{display:flex;align-items:center;gap:6px;color:#b6b6c1;font-size:12px}.download{margin-left:auto;min-width:155px;border-color:#ed3255;background:#dd3254;color:white;font-weight:600}.download:hover{background:#ef4566}.feedback{flex-basis:100%;font-size:12px;color:#c3c3cc;min-height:0}.feedback:empty{display:none}
      dialog{background:#17171b;color:#eee;border:1px solid #50505b;border-radius:12px;padding:12px;max-width:calc(100vw - 32px);max-height:calc(100vh - 32px);width:min(1000px,calc(100vw - 32px))}dialog::backdrop{background:#000c}.preview-head,.preview-tools{display:flex;align-items:center;gap:8px}.preview-head{justify-content:space-between;margin-bottom:8px}.preview-head span{font-size:13px}.preview-image{display:block;width:100%;height:calc(100vh - 240px);object-fit:contain;background:#111}.preview-tools{margin-top:10px;justify-content:center}.preview-tools button{font-size:12px}.preview-help{color:#9797a5;font-size:11px;text-align:center;margin-top:6px}
      @media(max-width:600px){.header{padding:62px 16px 10px}.heading{gap:8px}h1{font-size:15px}.heading>button{font-size:11px;padding:6px 8px}.footer{padding:10px 16px}.download{min-width:125px}.tools{gap:5px}.tools .count{font-size:11px}}
    `;
    const workspace=element('section','workspace'),header=element('header','header'),heading=element('div','heading'),metadata=element('div','metadata');
    headerTitle=element('h1','',note.title||'图片笔记');author=element('div','author',note.author?'@ '+note.author:'');metadata.append(headerTitle,author);
    heading.append(metadata,button('轮播视图',()=>{optedOut=note.id;destroy();updateLauncher();}));
    const tools=element('div','tools');count=element('span','count');
    filterInput=element('select');filterInput.setAttribute('aria-label','筛选图片');for(const [value,text] of [['all','全部图片'],['needed','只看未下载 / 缺失']]){const option=element('option','',text);option.value=value;filterInput.append(option);}filterInput.value=filterMode;filterInput.onchange=()=>{filterMode=filterInput.value;rebuildVisible(true);};
    tools.append(filterInput,button('选中缺失项',()=>{selection=new Set(note.assets.map((_,i)=>i).filter(i=>['missing','changed','live_missing'].includes(statusAt(i))));selectionChanged();}),button('全选',()=>{for(const i of visibleIndices)selection.add(i);selectionChanged();}),button('反选',()=>{for(const i of visibleIndices)selection.has(i)?selection.delete(i):selection.add(i);selectionChanged();}),button('清空',()=>{selection.clear();selectionChanged();}),count);header.append(heading,tools);
    viewport=element('div','viewport');viewport.tabIndex=0;viewport.setAttribute('aria-label','素材图片网格');spacer=element('div','spacer');tiles=element('div','tiles');spacer.append(tiles);viewport.append(spacer);
    const footer=element('footer','footer');liveLabel=element('label','live-option');liveInput=element('input');liveInput.type='checkbox';liveInput.checked=!!config.includeLive;
    liveInput.onchange=()=>{config.includeLive=liveInput.checked;setStates({note_id:note.id,expected_urls:note.assets.map(a=>a.url),assets:states});send({kind:'material_live',value:liveInput.checked});};liveLabel.append(liveInput,element('span','','同时保存 LIVE 动态片段'));
    const forceLabel=element('label','live-option');forceInput=element('input');forceInput.type='checkbox';forceLabel.append(forceInput,element('span','','重新下载已有图'));forceLabel.title='默认跳过本地仍存在的素材，保留在原位置；勾选后另存，不覆盖';
    download=button('下载选中',()=>submit(),'download');feedback=element('div','feedback');feedback.setAttribute('role','status');footer.append(liveLabel,forceLabel,download,feedback);
    preview=element('dialog');preview.setAttribute('aria-label','大图预览');const previewHead=element('div','preview-head');previewTitle=element('span');previewHead.append(previewTitle,button('关闭 · Esc',closePreview));
    previewImage=element('img','preview-image');previewImage.decoding='async';previewImage.referrerPolicy='no-referrer';previewStatus=element('div','preview-help');previewStatus.setAttribute('role','status');previewFeedback=element('div','preview-help');previewFeedback.setAttribute('role','status');const ownStatus=previewStatus;previewImage.onload=()=>{ownStatus.textContent='';};previewImage.onerror=()=>{ownStatus.textContent='预览加载失败，仍可下载图片';};
    const previewTools=element('div','preview-tools');previewDownload=button('下载这张',()=>submit([previewIndex]));previewTools.append(button('上一张',()=>openPreview((previewIndex+note.assets.length-1)%note.assets.length)),previewDownload,button('下一张',()=>openPreview((previewIndex+1)%note.assets.length)));
    preview.append(previewHead,previewImage,previewStatus,previewTools,previewFeedback,element('div','preview-help','← → 切换图片 · Esc 关闭预览'));
    let pressedBackdrop=false;
    const outside=event=>{const box=preview.getBoundingClientRect();return event.clientX<box.left||event.clientX>box.right||event.clientY<box.top||event.clientY>box.bottom;};
    preview.addEventListener('pointerdown',event=>{pressedBackdrop=event.button===0&&event.target===preview&&outside(event);});
    preview.addEventListener('pointerup',event=>{const dismiss=pressedBackdrop&&event.button===0&&event.target===preview&&outside(event);pressedBackdrop=false;if(dismiss){event.preventDefault();event.stopPropagation();closePreview();}});
    preview.addEventListener('cancel',event=>{event.preventDefault();event.stopPropagation();window.__stillDockNavigation?.suppressEscapeRelease?.();closePreview();});
const ownPreview=preview,ownImage=previewImage;preview.addEventListener('close',()=>{if(ownPreview.open)return;if(preview===ownPreview)previewIndex=null;ownImage.removeAttribute('src');});
    workspace.append(header,viewport,footer);shadow.append(style,workspace,preview);document.body.append(host);
    for(const name of ['click','dblclick','pointerdown','pointerup','keydown'])shadow.addEventListener(name,event=>event.stopPropagation());
    viewport.addEventListener('scroll',schedule,{passive:true});
    if(typeof IntersectionObserver==='function')imageObserver=new IntersectionObserver(entries=>{for(const entry of entries)if(entry.isIntersecting&&entry.target.dataset.src){entry.target.src=entry.target.dataset.src;delete entry.target.dataset.src;imageObserver.unobserve(entry.target);}},{root:viewport,rootMargin:'200px'});
    if(typeof ResizeObserver==='function'){resizeObserver=new ResizeObserver(()=>render(true));resizeObserver.observe(viewport);}
    liveLabel.hidden=!note.assets.some(a=>a.live_url);render(true);suspendNative();viewport.focus?.({preventScroll:true});releaseBoot();send({kind:'page',note,workspace:'grid'});send({kind:'workspace',mode:'grid',note_id:note.id});
  }
  function blockingDialog(){
    return [...document.querySelectorAll('.login-container,.login-modal,.captcha-container,[role="dialog"][aria-modal="true"],dialog[open]')].some(node=>node!==host&&!node.closest('#noteContainer,.note-detail-mask')&&node.getClientRects().length&&getComputedStyle(node).display!=='none'&&getComputedStyle(node).visibility!=='hidden');
  }
  function update(value,force=false){
    if(!value)return;
    const nextKey=key(value),different=nextKey!==signature;
    if(value.id!==note?.id){optedOut='';returning='';}
    if(different){const previous=note,chosen=new Set(selection);destroy();note=value;signature=nextKey;states=[];filterMode='all';selection=new Set((note.assets||[]).map((a,i)=>({a,i})).filter(({a})=>!previous?.assets?.length||previous.id!==note.id||previous.assets.some((old,j)=>chosen.has(j)&&sameImage(old,a))).map(({i})=>i));visibleIndices=(note.assets||[]).map((_,i)=>i);}
    else note=value;
    if(!config.enabled||!eligible(note)||optedOut===note.id||returning===note.id||blockingDialog()){if(active())destroy();updateLauncher();return;}
    if(!document.body)return;
    launcher?.remove();launcher=null;
    if(!active()){visibleIndices=note.assets.map((_,i)=>i);mount();if(filterMode!=='all')rebuildVisible();}else {
      if(headerTitle.textContent!==(note.title||'图片笔记'))headerTitle.textContent=note.title||'图片笔记';
      const name=note.author?'@ '+note.author:'';if(author.textContent!==name)author.textContent=name;
      suspendNative();if(force)render(true);
    }
  }
  function setConfig(next){if(next.enabled&&!config.enabled)optedOut='';config={...config,...next};if(liveInput)liveInput.checked=!!config.includeLive;update(window.__xhsReadNote?.()||note,true);if(note)setStates({note_id:note.id,expected_urls:note.assets.map(a=>a.url),assets:states});}
  function prepareBack(){if(active()){returning=note.id;destroy();}launcher?.remove();launcher=null;}
  function fallback(value){optedOut=value?.id||note?.id||'';destroy();if(value){if(key(value)!==signature)selection=new Set((value.assets||[]).map((_,i)=>i));note=value;signature=key(value);}updateLauncher();}
  window.__stillDockMaterial={fallback,update,setConfig,setSelection,setStates,closePreview,prepareBack,isOpen:active,isPreviewOpen:()=>!!preview?.open,result};
  const nativeStyle=element('style');nativeStyle.textContent='[data-sd-suspended]{visibility:hidden!important;pointer-events:none!important}#stilldock-material-launcher{position:fixed;right:20px;top:18px;z-index:2147483001;border:1px solid #676772;border-radius:8px;background:#242429;color:#eee;padding:9px 14px;font:13px/1.5 "Segoe UI","Microsoft YaHei",sans-serif;cursor:pointer}html[data-sd-material-pending] #noteContainer,html[data-sd-material-pending] .note-detail-mask{visibility:hidden!important}';
  function install(){
    (document.head||document.documentElement).append(nativeStyle);
    update(window.__xhsReadNote?.());
    // Shadow DOM changes do not retrigger the site's document-wide observers.
    new MutationObserver(records=>{const selector='#noteContainer,.note-detail-mask,.login-container,.login-modal,.captcha-container,[role="dialog"][aria-modal="true"],dialog[open]';const relevant=records.some(record=>record.target?.closest?.(selector)||[...record.addedNodes,...record.removedNodes].some(node=>node.id!=='stilldock-material'&&(node.matches?.(selector)||node.querySelector?.(selector))));if(!relevant||sourceFrame)return;sourceFrame=requestAnimationFrame(()=>{sourceFrame=null;try{update(window.__xhsReadNote?.());}catch{fallback(note);}});}).observe(document,{childList:true,subtree:true,attributes:true,attributeFilter:['class','style','aria-hidden']});
  }
  document.addEventListener('play',event=>{if(event.target?.closest?.('[data-sd-suspended]'))event.target.pause();},true);
  document.addEventListener('keydown',event=>{if(!active()||previewIndex!==null||event.key!=='Tab')return;const items=[...shadow.querySelectorAll('button,input,[tabindex]')].filter(n=>!n.disabled&&n.getClientRects().length&&!n.closest('[hidden]'));if(!items.length)return;const focused=shadow.activeElement,first=items[0],last=items[items.length-1];if(!focused||(event.shiftKey&&focused===first)||(!event.shiftKey&&focused===last)){event.preventDefault();(event.shiftKey?last:first).focus();}},true);
  document.addEventListener('keydown',event=>{if(!active()||previewIndex===null||event.ctrlKey||event.altKey||event.metaKey)return;if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();event.stopImmediatePropagation();openPreview((previewIndex+(event.key==='ArrowLeft'?-1:1)+note.assets.length)%note.assets.length);}},true);
  addEventListener('resize',schedule);
  addEventListener('pageshow',()=>update(window.__xhsReadNote?.()));
  addEventListener('pagehide',()=>{if(sourceFrame){cancelAnimationFrame(sourceFrame);sourceFrame=null;}destroy();launcher?.remove();launcher=null;note=null;signature='';selection.clear();});
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',install,{once:true});else install();
})();
