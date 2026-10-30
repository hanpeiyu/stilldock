(() => {
  const enabled = __ENABLED__;
  if (window.__xhsLocalAiFilter?.version === 2) {
    window.__xhsLocalAiFilter.setEnabled(enabled);
    return;
  }
  window.__xhsLocalAiFilter?.setEnabled(false);
  const style = document.createElement('style');
  style.id = 'xhs-local-ai-filter-style';
  style.textContent = `
    html[data-xhs-hide-ai] #ask_diandian,
    html[data-xhs-hide-ai] .ai-chat-section,
    html[data-xhs-hide-ai] .bottom-menu-ai { display:none !important; }
    html[data-xhs-hide-ai] .ai-feeds-page.with-ai-chat .search-layout-wrapper {
      width:100% !important; max-width:none !important; flex:1 1 100% !important;
    }
  `;
  (document.head || document.documentElement).append(style);
  const marked = new Map();
  const matches = el => /^(?:点点|问点点)(?:AI)?$/i.test((el?.textContent || '').replace(/\s/g, ''));
  let active = enabled, timer;
  function restore() {
    for (const [el, old] of marked) {
      if (old.value) el.style.setProperty('display', old.value, old.priority);
      else el.style.removeProperty('display');
    }
    marked.clear();
  }
  function scan() {
    for (const el of marked.keys()) if (!el.isConnected) marked.delete(el);
    if (!active) return;
    document.documentElement.setAttribute('data-xhs-hide-ai','');
    for (const leaf of document.querySelectorAll('a,button,[role="button"],nav span,aside span,.side-bar span,.side-bar-component span')) {
      if (!matches(leaf)) continue;
      const el = leaf.closest('a,button,[role="button"],li') || leaf;
      if (!matches(el) || marked.has(el)) continue;
      marked.set(el, {value:el.style.getPropertyValue('display'), priority:el.style.getPropertyPriority('display')});
      el.style.setProperty('display','none','important');
    }
  }
  document.addEventListener('click', event => {
    if (active && event.composedPath().some(el => el instanceof Element && matches(el))) {
      event.preventDefault();event.stopImmediatePropagation();
    }
  }, true);
  const observer = new MutationObserver(() => {
    if (active) {clearTimeout(timer);timer=setTimeout(scan,150);}
  });
  observer.observe(document,{childList:true,subtree:true,characterData:true});
  window.__xhsLocalAiFilter = {version:2,setEnabled(value) {
    if (active === value) return;
    active = value; clearTimeout(timer);
    document.documentElement.toggleAttribute('data-xhs-hide-ai',active);
    if (active) scan(); else restore();
    window.dispatchEvent(new Event('resize'));
  }};
  scan();
})();
