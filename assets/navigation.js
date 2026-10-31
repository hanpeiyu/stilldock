// Persistent in-page return controls; independent of the focus-mode layout.
(() => {
  if (window.top !== window || !/(^|\.)xiaohongshu\.com$/.test(location.hostname) || window.__stillDockNavigation) return;
  let button, busy = false, cameFromList = false, scheduled, escapeHeld=false;
  const key = 'stilldock.returnUrl';
  const noteRoute = raw => {
    try {
      const u = new URL(raw, location.href);
      return /\/(?:explore|discovery\/item|search_result)\/[a-f0-9]{24}(?:\/|$)/i.test(u.pathname)
        || /^[a-f0-9]{24}$/i.test(u.searchParams.get('noteId') || u.searchParams.get('note_id') || '');
    } catch { return false; }
  };
  const listUrl = raw => {
    try {
      const u = new URL(raw, location.href);
      return u.origin === location.origin && !noteRoute(u.href) && /^\/(?:search_result|explore)?\/?$/.test(u.pathname) ? u.href : '';
    } catch { return ''; }
  };
  let destination = '';
  try { destination = listUrl(sessionStorage.getItem(key)); } catch {}
  destination ||= listUrl(document.referrer) || new URL('/explore', location.href).href;
  function visible(node) {
    if (!node || !node.getClientRects().length || node.getAttribute('aria-hidden') === 'true') return false;
    const style = getComputedStyle(node);
    return style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0';
  }
  function noteOpen() {
    return window.__stillDockMaterial?.isOpen?.() || noteRoute(location.href) || [...document.querySelectorAll('#noteContainer,.note-detail-mask')].some(visible);
  }
  function remember(raw) {
    const url = listUrl(raw);
    if (!url || url === destination) return;
    destination = url;
    try { sessionStorage.setItem(key, url); } catch {}
  }
  function sync() {
    const open = noteOpen();
    if (!open) { remember(location.href); busy = false; cameFromList = false; }
    document.documentElement?.toggleAttribute('data-stilldock-note-open', open);
    if (button) { button.hidden = !open; button.disabled = busy; }
  }
  function fallback() {
    if (!noteOpen()) { sync(); return; }
    if (cameFromList) {
      cameFromList = false;
      history.back();
      setTimeout(() => { if (noteOpen()) location.assign(destination); }, 400);
    } else location.assign(destination);
  }
  function back() {
    if (busy || !noteOpen()) return;
    if(window.__stillDockMaterial?.closePreview?.())return;
    window.__stillDockMaterial?.prepareBack?.();
    busy = true;
    if (button) button.disabled = true;
    const close = [...document.querySelectorAll('.note-detail-mask .close-circle,.note-detail-mask .close-icon,.close-circle,[aria-label="关闭笔记"]')]
      .find(node => visible(node) && typeof node.click === 'function');
    const mask = [...document.querySelectorAll('.note-detail-mask')].find(node => visible(node) && typeof node.click === 'function');
    // Native close preserves the loaded feed and its scroll position.
    if (close || mask) { (close || mask).click(); setTimeout(() => { sync(); if (noteOpen()) fallback(); }, 300); }
    else fallback();
  }
  function mount() {
    if (!document.body || button) return;
    const style = document.createElement('style');
    style.textContent = `#stilldock-back{position:fixed;top:16px;left:18px;z-index:2147483646;display:flex;align-items:center;gap:12px;padding:9px 13px;border:1px solid #73737b;border-radius:9px;background:#222225ed;color:#fff;box-shadow:0 3px 16px #0004;cursor:pointer;font:13px/1.5 'Segoe UI','Microsoft YaHei',sans-serif;backdrop-filter:blur(8px)}#stilldock-back[hidden]{display:none!important}#stilldock-back:hover{background:#39393e}#stilldock-back:focus-visible{outline:2px solid #ff4466;outline-offset:2px}#stilldock-back small{color:#b8b8c1;font-size:10px}html[data-stilldock-note-open] .xhs-local-logo{visibility:hidden!important}`;
    (document.head || document.documentElement).append(style);
    button = document.createElement('button');
    button.id = 'stilldock-back'; button.type = 'button'; button.hidden = true;
    button.setAttribute('aria-label', '返回搜索结果（Esc）');
    button.title = '返回搜索结果 · Esc';
    const label = document.createElement('span'), shortcut = document.createElement('small');
    label.textContent = '← 返回搜索'; shortcut.textContent = 'Esc';
    button.append(label, shortcut); button.onclick = back;
    document.body.append(button);
    sync();
    new MutationObserver(() => {
      if (!scheduled) scheduled = setTimeout(() => { scheduled = null; sync(); }, 60);
    }).observe(document, {childList:true,subtree:true,attributes:true,attributeFilter:['class','style','aria-hidden']});
  }
  for (const name of ['pushState', 'replaceState']) {
    const original = history[name];
    history[name] = function (...args) {
      const before = location.href, result = original.apply(this, args);
      if (listUrl(before) && noteRoute(location.href)) { remember(before); cameFromList = name === 'pushState'; }
      sync(); return result;
    };
  }
  addEventListener('popstate', sync);
  // Capture at window before site handlers; one physical press must dismiss only one layer.
  function stop(event){event.preventDefault();event.stopImmediatePropagation();}
  addEventListener('keydown', event => {
    if(event.key!=='Escape'||event.isComposing)return;
    if(escapeHeld){stop(event);return;}
    if(!window.__stillDockMaterial?.isPreviewOpen?.()&&!noteOpen())return;
    escapeHeld=true;stop(event);
    if(event.repeat)return;
    if(window.__stillDockMaterial?.closePreview?.())return;
    back();
  },true);
  addEventListener('keyup',event=>{if(event.key==='Escape'&&escapeHeld){stop(event);escapeHeld=false;}},true);
  addEventListener('blur',()=>{escapeHeld=false;});
  window.__stillDockNavigation = {back, sync, isOpen:noteOpen,suppressEscapeRelease(){escapeHeld=true;}};
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', mount, {once:true});
  else mount();
})();
