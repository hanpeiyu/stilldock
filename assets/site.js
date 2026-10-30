(() => {
  if (window.top !== window || !/(^|\.)xiaohongshu\.com$/.test(location.hostname)) return;
  const send = value => window.ipc.postMessage(JSON.stringify(value));
  let last='',timer,lastSearchRoute='';
  // Search behavior belongs to the browser bridge, independent of focus-mode CSS.
  function searchRoute(){
    const url=new URL(location.href);
    if(/\/(?:explore|discovery\/item|search_result)\/[a-f0-9]{24}(?:\/|$)/i.test(url.pathname)||/^[a-f0-9]{24}$/i.test(url.searchParams.get('noteId')||url.searchParams.get('note_id')||''))return;
    if(url.pathname.replace(/\/$/,'')!=='/search_result'){lastSearchRoute='';return;}
    const route='/search_result?'+(url.searchParams.get('keyword')||url.searchParams.get('query')||'');
    if(route!==lastSearchRoute){lastSearchRoute=route;send({kind:'search_start'});}
  }
  function searchIntent(event){
    const target=event.target;
    if(!target||typeof target.closest!=='function'||event.isComposing)return;
    const root=target.closest('.input-box,.search-container,.search-input,.search-box,[data-xhs-native-search]');
    const input=target.closest('#search-input,input[type="search"],input[placeholder*="搜索"]');
    if(event.type==='keydown'){
      if(event.key==='Enter'&&(root||input))send({kind:'search_start'});
    }else if(event.type==='submit'){
      if(root||target.querySelector?.('#search-input,input[type="search"],input[placeholder*="搜索"]'))send({kind:'search_start'});
    }else if(event.type==='click'){
      const link=target.closest('a[href]');
      const href=link?.getAttribute('href')||'';
      const button=target.closest('button,[role="button"],.search-icon,.search-submit,.search-button,.search-icon-box');
      let searchLink=false;try{if(href){const url=new URL(href,location.href);searchLink=url.pathname.replace(/\/$/,'')==='/search_result'&&!/^[a-f0-9]{24}$/i.test(url.searchParams.get('noteId')||url.searchParams.get('note_id')||'');}}catch{}
      if(searchLink||(root&&button))send({kind:'search_start'});
    }
  }
  document.addEventListener('keydown',searchIntent,true);
  document.addEventListener('submit',searchIntent,true);
  document.addEventListener('click',searchIntent,true);
  window.__xhsReadNote = () => {
    const unwrap=v=>{for(let i=0;i<5&&v&&typeof v==='object';i++){if('_value' in v)v=v._value;else if('value' in v)v=v.value;else break;}return v;};
    const state=unwrap(unwrap(window.__INITIAL_STATE__)?.note);
    const valid=id=>typeof id==='string'&&/^[a-f0-9]{24}$/i.test(id);
    const pathId=(location.pathname.match(/\/(?:explore|discovery\/item|search_result)\/([a-f0-9]{24})(?:\/|$)/i)||[])[1];
    const sourceUrl=new URL(location.href);
    const modal=document.querySelector('#noteContainer,.note-detail-mask');
    const queryId=sourceUrl.searchParams.get('noteId')||sourceUrl.searchParams.get('note_id');
    const modalId=unwrap(state?.currentNoteId);
    const map=unwrap(state?.noteDetailMap)||{};
    const id=pathId||(modal&&(valid(queryId)?queryId:valid(modalId)?modalId:''))||'';
    const entry=unwrap(map[id]);
    let note=unwrap(entry?.note)||unwrap(unwrap(map['-1'])?.note);
    if(note&&note.noteId&&note.noteId!==id)note=null;
    if(!note&&id){
      for(const value of Object.values(map)){const candidate=unwrap(unwrap(value)?.note);if(candidate&&(candidate.noteId===id||candidate.id===id)){note=candidate;break;}}
    }
    const q=s=>document.querySelector(s),text=s=>(q(s)?.textContent||'').trim();
    const source=new URL(location.href);
    if(id&&!pathId)source.pathname='/explore/'+id;
    const result={id,title:note?.title||text('#detail-title')||document.title,author:note?.user?.nickname||text('.author .username'),url:source.href,cover:'',image_count:note?.imageList?.length||0,assets:[]};
    const https=u=>{if(typeof u!=='string'||! /^(?:https?:)?\/\//i.test(u))return '';try{const value=new URL(u.replace(/^http:/,'https:'),location.href);return value.protocol==='https:'&&/(^|\.)(xhscdn|xiaohongshu)\.com$/.test(value.hostname)?value.href:'';}catch{return '';}};
    function bestStream(stream){
      const entries=Object.values(unwrap(stream)||{}).flat().filter(v=>v&&typeof v==='object');
      entries.sort((a,b)=>(Number(b.width||0)*Number(b.height||0)-Number(a.width||0)*Number(a.height||0))||(Number(b.avgBitrate||b.videoBitrate||0)-Number(a.avgBitrate||a.videoBitrate||0)));
      for(const entry of entries){for(const value of [entry.masterUrl,...(Array.isArray(entry.backupUrls)?entry.backupUrls:[])]){const url=https(value);if(url)return url;}}
      return '';
    }
    function originalImage(image,web){
      const supplied=https(image.urlOriginal||image.originalUrl);if(supplied)return supplied;
      if(!web)return '';
      const parsed=new URL(web),parts=parsed.pathname.split('/').filter(Boolean);
      let token='';
      if(/^sns-webpic-[a-z0-9-]+\.xhscdn\.com$/.test(parsed.hostname)&&parts.length>=3)token=parts.slice(2).join('/').split('!')[0];
      else if(/^sns-img-[a-z0-9-]+\.xhscdn\.com$/.test(parsed.hostname)&&parsed.pathname.includes('!'))token=parts.join('/').split('!')[0];
      return token&&/^[a-z0-9_/-]+$/i.test(token)?'https://ci.xiaohongshu.com/'+token+'?imageView2/format/jpeg':'';
    }
    if(note){
      if(note.type==='video'){
        let video=bestStream(note.video?.media?.stream);
        if(!video&&note.video?.consumer?.originVideoKey)video='https://sns-video-bd.xhscdn.com/'+note.video.consumer.originVideoKey;
        if(video)result.assets.push({url:video,video:true});
      } else for(const [position,image] of (note.imageList||[]).entries()){
        const url=https(image.urlDefault||image.url||image.infoList?.find(i=>i.imageScene==='WB_DFT')?.url||image.urlPre);
        if(url)result.assets.push({url,video:false,preview:https(image.urlPre)||url,original_url:originalImage(image,url),live_url:bestStream(image.stream||image.livePhoto?.stream),width:Number(image.width)||0,height:Number(image.height)||0,sequence:position+1});
      }
      result.cover=https(note.imageList?.[0]?.urlPre||note.imageList?.[0]?.urlDefault);
    }
    // Fall back only to the opened note's image slides, never the search feed.
    if(id&&!result.assets.length&&note?.type!=='video'){
      const seen=new Set();
      for(const image of document.querySelectorAll?.('#noteContainer .swiper-slide img, .note-detail-mask .swiper-slide img')||[]){
        const url=https(image.getAttribute('data-src')||image.currentSrc||image.src);
        if(url&&!seen.has(url)){seen.add(url);result.assets.push({url,video:false});}
      }
    }
    if(!id){result.assets=[];result.title='';result.author='';}
    return result;
  };
  window.__xhsReadCurrentImage = () => {
    const note=window.__xhsReadNote();
    if(!note.id||!note.assets.length||note.assets.some(a=>a.video))return {error:'请先打开一篇图片笔记'};
    const media=document.querySelector('#noteContainer .media-container,.note-detail-mask .media-container');
    if(!media||!media.getClientRects().length||getComputedStyle(media).visibility==='hidden')return {error:'请先打开一篇图片笔记'};
    if(note.assets.length===1&&note.image_count===1)return {note,index:0};
    const active=media?.querySelector('.swiper-slide-active');
    const raw=active?.getAttribute('data-swiper-slide-index');
    if(raw!==null&&raw!==undefined&&/^\d+$/.test(raw)){const sequence=Number(raw)+1,index=note.assets.findIndex((asset,i)=>(asset.sequence||i+1)===sequence);if(index>=0)return {note,index};}
    function identity(raw){try{const u=new URL(raw),parts=u.pathname.split('/').filter(Boolean);return (/^sns-webpic-/.test(u.hostname)?parts.slice(2):parts).join('/').split('!')[0];}catch{return '';}}
    const img=active?.querySelector('img');
    const token=identity(img?.currentSrc||img?.src||'');
    if(token){const matches=note.assets.map((asset,index)=>({asset,index})).filter(({asset})=>[asset.url,asset.preview,asset.original_url].some(url=>url&&identity(url)===token));if(matches.length===1)return {note,index:matches[0].index};}
    const counter=media?.querySelector('.swiper-pagination-current');
    const number=Number(counter?.textContent?.trim());
    if(Number.isInteger(number)&&number>=1){const index=note.assets.findIndex((asset,i)=>(asset.sequence||i+1)===number);if(index>=0)return {note,index};}
    return {error:'暂时无法确认当前图片，请使用侧栏选图'};
  };
  function report(){
    searchRoute();
    try{const note=window.__xhsReadNote();const value=JSON.stringify({kind:'page',note});if(value!==last){last=value;send(JSON.parse(value));}}catch{}
  }
  function schedule(){if(!timer)timer=setTimeout(()=>{timer=null;report();},180);}
  const originalPush=history.pushState,originalReplace=history.replaceState;
  history.pushState=function(...a){const v=originalPush.apply(this,a);searchRoute();schedule();return v;};
  history.replaceState=function(...a){const v=originalReplace.apply(this,a);searchRoute();schedule();return v;};
  addEventListener('popstate',()=>{searchRoute();schedule();});
  function install(){
    searchRoute();
    try{window.__xhsInstallAdapters?.();}catch{window.__xhsReleaseBoot?.();}
    new MutationObserver(schedule).observe(document,{childList:true,subtree:true,characterData:true});
    report();
  }
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',install,{once:true});else install();
})();
