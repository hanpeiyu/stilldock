/* Conservative site adaptation: unknown layouts remain usable and untouched. */
(() => {
  const config = __CONFIG__;
  if (!/(^|\.)xiaohongshu\.com$/.test(location.hostname)) return;
  if (window.__xhsFocusMode) { window.__xhsFocusMode.update(config); return; }
  let options = config, timer;
  const marked = new Set(), comments = new Map(), homeFeeds = new Set();
  let nativeSearch, focusedInput, localLogo, suppressSearch=false;
  const background=getComputedStyle(document.body).backgroundColor;
  document.documentElement.style.setProperty('--xhs-focus-background',background&&background!=='rgba(0, 0, 0, 0)'?background:'#18181b');
  const style = document.createElement('style');
  style.textContent = `
    html[data-xhs-focus] [data-xhs-focus-hidden] {display:none !important}
    html[data-xhs-focus] [data-xhs-comments-folded] {display:none !important}
    html[data-xhs-focus] .side-bar {display:none !important}
    html[data-xhs-focus] .main-content.with-side-bar {margin-left:0 !important;width:100% !important;max-width:none !important}
    html[data-xhs-focus-home] .search-layout__top,
    html[data-xhs-focus-home] .channel-container {display:none !important}
    html[data-xhs-focus-home],html[data-xhs-focus-home] body,
    html[data-xhs-focus-home] #app,html[data-xhs-focus-home] #global,
    html[data-xhs-focus-home] .main-container,html[data-xhs-focus-home] .main-content,
    html[data-xhs-focus-home] .header-dual,html[data-xhs-focus-home] .header-container,
    html[data-xhs-focus-home] .mask-paper {background:var(--xhs-focus-background) !important}
    .xhs-local-logo{position:fixed;top:24px;left:28px;z-index:10001;display:flex;align-items:center;justify-content:center;text-decoration:none}
    .xhs-local-logo svg,.xhs-local-logo img{display:block;width:72px;height:36px}
    .xhs-local-logo-fallback{background:#ff2442;color:white;border-radius:18px;padding:5px 11px;font:bold 18px/1.4 sans-serif}
    .xhs-local-comments-toggle {display:block;margin:16px 0;padding:9px 14px;border:1px solid #8884;border-radius:8px;background:transparent;color:inherit;cursor:pointer;font:inherit;font-size:13px}
    html[data-xhs-focus-home] [data-xhs-native-search] {
      position:fixed !important;left:50% !important;top:calc(50vh - 30px) !important;
      transform:translateX(-50%) !important;width:min(800px,calc(100vw - 120px)) !important;
      max-width:none !important;margin:0 !important;z-index:10000 !important;background:transparent !important;
    }
    html[data-xhs-focus-home] [data-xhs-native-search] .mask-paper {background:transparent !important}
    html[data-xhs-focus-home] [data-xhs-native-search] .input-box {
      position:relative !important;inset:auto !important;transform:none !important;
      margin:0 auto !important;width:min(800px,calc(100vw - 120px)) !important;
    }
    .xhs-local-downloaded-badge {position:absolute;left:9px;top:9px;z-index:5;border-radius:5px;background:#304d3ee8;color:white;padding:3px 7px;font:11px/1.5 sans-serif;pointer-events:none}
  `;
  (document.head || document.documentElement).append(style);
  function restore() {
    for (const node of marked) node.removeAttribute('data-xhs-focus-hidden');
    marked.clear();
    localLogo?.remove();localLogo=null;
    for(const node of homeFeeds)node.removeAttribute('data-xhs-focus-hidden');
    homeFeeds.clear();nativeSearch?.removeAttribute('data-xhs-native-search');nativeSearch=null;
    document.documentElement.removeAttribute('data-xhs-focus-home');
    for (const [node, button] of comments) {node.removeAttribute('data-xhs-comments-folded');button.remove();}
    comments.clear();
    document.querySelectorAll('.xhs-local-downloaded-badge').forEach(el=>el.remove());
    document.querySelectorAll('[data-xhs-local-downloaded]').forEach(el=>el.removeAttribute('data-xhs-local-downloaded'));
  }
  function scan() {
    if (!options.enabled) return;
    if(!localLogo?.isConnected){
      const original=document.querySelector('.side-bar .logo svg,svg.logo,.logo-wrapper svg,.side-bar .logo img,.logo svg');
      localLogo=document.createElement('a');localLogo.className='xhs-local-logo';localLogo.href='/explore';localLogo.setAttribute('aria-label','小红书首页');
      if(original){const copy=original.cloneNode(true);copy.removeAttribute('id');localLogo.append(copy);}
      else{localLogo.textContent='小红书';localLogo.classList.add('xhs-local-logo-fallback');}
      document.body.append(localLogo);
    }
    const home=location.pathname==='/explore'||location.pathname==='/';
    if(!home){
      window.__xhsReleaseBoot?.();
      for(const node of homeFeeds)node.removeAttribute('data-xhs-focus-hidden');
      homeFeeds.clear();nativeSearch?.removeAttribute('data-xhs-native-search');nativeSearch=null;
      document.documentElement.removeAttribute('data-xhs-focus-home');
    }
    if(home){
      const feed=document.querySelector('.feeds-container');
      const visibleBox=[...document.querySelectorAll('.input-box.search-box-in-content,.input-box,.search-container')].find(el=>el.getBoundingClientRect().width>100 && el.getBoundingClientRect().height>0);
      const input=visibleBox?.querySelector('input,textarea,[contenteditable="true"]')||document.querySelector('#search-input');
      if(visibleBox){
        // Preserve the native component and its listeners/history. Only position its wrapper.
        const wrapper=visibleBox;
        if(wrapper && wrapper!==document.body){
          if(nativeSearch!==wrapper){nativeSearch?.removeAttribute('data-xhs-native-search');nativeSearch=wrapper;}
          wrapper.setAttribute('data-xhs-native-search','');
          document.documentElement.setAttribute('data-xhs-focus-home','');
          if(feed){feed.setAttribute('data-xhs-focus-hidden','');homeFeeds.add(feed);}
          // Hide only the native quick prompts, not search history or suggestions.
          for(const el of wrapper.querySelectorAll('button,a,span,div')){
            const label=(el.textContent||'').replace(/\s/g,'');
            if(el.children.length<3&&/^(做旅游攻略|答疑解惑)$/.test(label)){
              const target=el.closest('button,a')||el;target.setAttribute('data-xhs-focus-hidden','');marked.add(target);
            }
          }
          window.__xhsReleaseBoot?.();
          if(input&&focusedInput!==input){focusedInput=input;if(!/INPUT|TEXTAREA/.test(document.activeElement?.tagName||'')&&!document.activeElement?.isContentEditable){suppressSearch=true;try{input.focus({preventScroll:true});}finally{suppressSearch=false;}}}
        }
      }
    }
    for (const node of marked) if (!node.isConnected) marked.delete(node);
    // Only exact navigation labels within known navigation elements are hidden.
    for (const node of document.querySelectorAll('.side-bar-component, .side-bar a, .side-bar li, aside a')) {
      const label=(node.textContent||'').replace(/\s/g,'');
      if (/^(?:直播|发布|通知|消息|RED)(?:\d+\+?)?$/.test(label)) {
        node.setAttribute('data-xhs-focus-hidden','');marked.add(node);
      }
    }
    for (const [node, button] of comments) if (!node.isConnected) {button.remove();comments.delete(node);}
    if(options.foldComments) for(const node of document.querySelectorAll('.note-detail-mask .comments-container, .note-container .comments-container')) {
      if(comments.has(node))continue;
      const button=document.createElement('button');button.className='xhs-local-comments-toggle';button.textContent='展开评论';
      node.setAttribute('data-xhs-comments-folded','');button.setAttribute('aria-expanded','false');
      button.onclick=()=>{const folded=node.toggleAttribute('data-xhs-comments-folded');button.textContent=folded?'展开评论':'收起评论';button.setAttribute('aria-expanded',String(!folded));};
      node.before(button);comments.set(node,button);
    }
    // Mark only known note cards; never change note links or synthesize clicks.
    const done=new Set(options.downloaded||[]);
    for(const card of document.querySelectorAll('section.note-item, .feeds-container .note-item')) {
      const link=card.querySelector('a[href*="/explore/"],a[href*="/discovery/item/"]');
      const id=(link?.getAttribute('href')?.match(/\/(?:explore|discovery\/item)\/([a-f0-9]{24})/i)||[])[1];
      if(!id||!done.has(id.toLowerCase())||card.hasAttribute('data-xhs-local-downloaded'))continue;
      card.setAttribute('data-xhs-local-downloaded','');
      const badge=document.createElement('span');badge.className='xhs-local-downloaded-badge';badge.textContent='已下载';card.append(badge);
    }
  }
  const observer=new MutationObserver(()=>{if(options.enabled&&!timer)timer=setTimeout(()=>{timer=null;scan();},document.documentElement.hasAttribute('data-xhs-boot')?0:120);});
  observer.observe(document,{childList:true,subtree:true,characterData:true});
  function update(next) {
    const changed=options.enabled!==next.enabled||options.foldComments!==next.foldComments;
    options=next;document.documentElement.toggleAttribute('data-xhs-focus',!!options.enabled);
    if(changed)restore();
    if(options.enabled)scan();else{restore();window.__xhsReleaseBoot?.();}
  }
  window.__xhsFocusMode={update};update(config);
})();
