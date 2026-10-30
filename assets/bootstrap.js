// Runs at document creation, before the site's first paint.
(() => {
  const options=__BOOT_CONFIG__;
  if(!/(^|\.)xiaohongshu\.com$/.test(location.hostname))return;
  let observer,deadline;
  function release(){document.documentElement?.removeAttribute('data-xhs-boot');clearTimeout(deadline);}
  window.__xhsReleaseBoot=release;
  function attach(){
    const root=document.documentElement;if(!root)return false;
    observer?.disconnect();
    if(options.focus)root.setAttribute('data-xhs-focus','');
    if(options.ai)root.setAttribute('data-xhs-hide-ai','');
    const style=document.createElement('style');style.id='xhs-early-style';
    style.textContent=`html[data-xhs-focus],html[data-xhs-focus] body{background:#18181b!important}
      html[data-xhs-focus] .side-bar,html[data-xhs-focus-home] .channel-container{display:none!important}
      html[data-xhs-hide-ai] #ask_diandian,html[data-xhs-hide-ai] .ai-chat-section,html[data-xhs-hide-ai] .bottom-menu-ai{display:none!important}
      html[data-xhs-boot] body{visibility:hidden!important}
      html[data-xhs-boot]::before{content:'正在加载搜索…';position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);padding-top:62px;background:url('__BRAND_ICON__') center top/48px 48px no-repeat;color:#92929c;font:13px \"Segoe UI\",sans-serif}`;
    root.append(style);
    if(options.focus&&/^\/(?:explore)?\/?$/.test(location.pathname)){
      root.setAttribute('data-xhs-boot','');
      // Unknown layouts or script errors must never leave the page hidden.
      deadline=setTimeout(release,1500);
      addEventListener('pagehide',release,{once:true});
    }
    return true;
  }
  if(!attach()){observer=new MutationObserver(attach);observer.observe(document,{childList:true,subtree:true});}
})();
