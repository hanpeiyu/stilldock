// Native playback stays untouched; a small isolated toolbar joins the existing queue.
(() => {
  if(window.top!==window||!/(^|\.)xiaohongshu\.com$/.test(location.hostname)||window.__stillDockVideo)return;
  let note,host,root,download,force,feedback,pending=false,timer,previous='';
  const send=value=>window.ipc.postMessage(JSON.stringify(value));
  const active=()=>!!host?.isConnected;
  const video=value=>!!value?.id&&(value.media_type==='video'||value.assets?.some(a=>a.video&&!a.live));
  function destroy(){clearTimeout(timer);pending=false;if(active())send({kind:'workspace',mode:'native',note_id:note?.id||''});host?.remove();host=root=download=force=feedback=null;document.documentElement?.removeAttribute('data-stilldock-video');}
  function result(ok,message,expectedId){if(!active()||(expectedId&&expectedId!==note.id))return;clearTimeout(timer);pending=false;feedback.textContent=message;feedback.setAttribute('role',ok?'status':'alert');refresh();}
  function refresh(){if(!active())return;const ready=note.assets?.some(a=>a.video&&!a.live);download.disabled=pending||!ready;download.textContent=pending?'正在加入队列…':ready?'下载视频':'正在读取视频…';}
  function submit(){
    if(!active()||pending)return;
    const fresh=window.__xhsReadNote?.(),index=fresh?.assets?.findIndex(a=>a.video&&!a.live);
    if(!fresh||fresh.id!==note.id||!video(fresh)||index<0||index===undefined){result(false,'视频尚未就绪，请刷新或使用右侧备用下载');return;}
    pending=true;feedback.textContent='';refresh();send({kind:'material_download',note:fresh,indices:[index],include_live:false,force:force.checked,expected_urls:fresh.assets.map(a=>a.url)});
    const id=note.id;timer=setTimeout(()=>result(false,'尚未收到确认，请查看右侧任务队列',id),6000);
  }
  function update(value){
    const blocked=[...(document.querySelectorAll?.('.login-container,.login-modal,.captcha-container,[role="dialog"][aria-modal="true"],dialog[open]')||[])].some(node=>!node.closest('#noteContainer,.note-detail-mask')&&node.getClientRects().length&&getComputedStyle(node).display!=='none'&&getComputedStyle(node).visibility!=='hidden');
    if(!video(value)||blocked){destroy();note=value;previous='';return;}
    if(value.id!==previous){destroy();previous=value.id;}
    note=value;if(!document.body)return;
    if(!active()){
      host=document.createElement('div');host.id='stilldock-video-actions';root=host.attachShadow({mode:'open'});
      const style=document.createElement('style');style.textContent=`:host{position:fixed!important;right:18px!important;top:16px!important;z-index:2147483001!important;display:block!important;max-width:calc(100vw - 190px);color:#eee;font:12px/1.6 'Segoe UI','Microsoft YaHei',sans-serif;color-scheme:dark}*{box-sizing:border-box}.bar{display:flex;gap:12px;align-items:center;flex-wrap:wrap;justify-content:flex-end;padding:8px 10px;border:1px solid #45454f;border-radius:10px;background:#222228f5;box-shadow:0 3px 16px #0004}label{display:flex;gap:5px;align-items:center;font-size:11px;cursor:pointer}input{accent-color:#ed3255}button{font:inherit;border:1px solid #ed3255;border-radius:7px;padding:8px 18px;background:#dd3254;color:white;cursor:pointer;font-weight:600}button:disabled{opacity:.55;cursor:default}button:focus-visible,input:focus-visible{outline:2px solid #ff5c7c;outline-offset:2px}.feedback{font-size:11px;text-align:right;max-width:360px;padding:3px 6px;border-radius:5px;background:#18181bf5}.feedback:empty{display:none}@media(max-width:520px){:host{top:64px!important;max-width:calc(100vw - 36px)}}`;
      const bar=document.createElement('div'),label=document.createElement('label');bar.className='bar';force=document.createElement('input');force.type='checkbox';label.append(force,document.createTextNode('重新下载已有文件'));
      download=document.createElement('button');download.type='button';download.onclick=submit;feedback=document.createElement('div');feedback.className='feedback';feedback.setAttribute('role','status');bar.append(label,download);root.append(style,bar,feedback);document.body.append(host);
      for(const type of ['click','pointerdown','pointerup','keydown'])root.addEventListener(type,event=>event.stopPropagation());
      document.documentElement.setAttribute('data-stilldock-video','');send({kind:'page',note,workspace:'video'});send({kind:'workspace',mode:'video',note_id:note.id});
    }
    refresh();
  }
  const style=document.createElement('style');style.textContent='html[data-stilldock-video] .xhs-material-toolbar{top:64px!important}';
  function install(){(document.head||document.documentElement).append(style);update(window.__xhsReadNote?.());}
  window.__stillDockVideo={update,result,hide:destroy,isOpen:active};
  addEventListener('pagehide',()=>{destroy();note=null;previous='';});
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',install,{once:true});else install();
})();
