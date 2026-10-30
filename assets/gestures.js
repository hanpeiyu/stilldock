// Right-button gestures. Idle: no overlay, timers or animation loop.
(() => {
  if (window.top !== window || !/(^|\.)xiaohongshu\.com$/.test(location.hostname) || window.__stillDockGestures) return;
  let enabled = __GESTURES_ENABLED__, active, overlay, line, hud, svg, frame, hideTimer, suppressUntil = 0;
  const threshold = 56, previewThreshold = 12;
  const now = () => performance.now();
  const send = value => window.ipc.postMessage(JSON.stringify(value));
  function ensureOverlay() {
    if (overlay?.isConnected) return;
    if (!document.body) return;
    const style = document.createElement('style');
    style.textContent = '#stilldock-gesture-ui{position:fixed;inset:0;pointer-events:none!important;z-index:2147483647}#stilldock-gesture-ui[hidden]{display:none!important}#stilldock-gesture-ui svg{position:absolute;inset:0;width:100%;height:100%;overflow:hidden}#stilldock-gesture-ui .gesture-hud{position:absolute;bottom:28px;left:50%;transform:translateX(-50%);max-width:calc(100% - 32px);padding:10px 16px;border:1px solid #ff5b7280;border-radius:10px;background:#222225f2;color:#fff;box-shadow:0 4px 24px #0004;font:13px/1.7 "Segoe UI","Microsoft YaHei",sans-serif;text-align:center;white-space:pre-line}';
    (document.head || document.documentElement).append(style);
    overlay = document.createElement('div'); overlay.id = 'stilldock-gesture-ui'; overlay.hidden = true;
    svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    line = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
    for (const [key, value] of Object.entries({fill:'none',stroke:'#ff5b72','stroke-width':'3','stroke-linecap':'round','stroke-linejoin':'round'})) line.setAttribute(key, value);
    svg.append(line); hud = document.createElement('div'); hud.className = 'gesture-hud'; hud.setAttribute('role','status');
    overlay.append(svg, hud); document.body.append(overlay);
  }
  function text(value) { if (hud && hud.textContent !== value) hud.textContent = value; }
  function notify(message) {
    if (active) return;
    ensureOverlay(); if (!overlay) return;
    clearTimeout(hideTimer); overlay.hidden = false; svg.style.display = 'none'; text(message);
    hideTimer = setTimeout(() => { if (!active) overlay.hidden = true; }, 1800);
  }
  function classify(state) {
    const dx = state.x - state.startX, dy = state.y - state.startY;
    const x = Math.abs(dx), y = Math.abs(dy), displacement = Math.hypot(dx, dy);
    if (now() - state.started > 5000) return {label:'手势时间过长，松开取消'};
    if (Math.max(x,y) < threshold) return {label:'继续划动 · Esc 取消'};
    if (displacement / Math.max(state.length,1) < 0.72 || Math.max(x,y) < Math.min(x,y) * 1.55) return {label:'方向不明确，松开取消'};
    if (x > y) return dx < 0 ? {action:'back',label:'← 返回搜索 · 松开执行'} : {action:'toggle_sidebar',label:'→ 展开 / 收起下载栏 · 松开执行'};
    return dy > 0 ? {action:'download_current',label:'↓ 下载当前图片 · 松开执行'} : {label:'上划未绑定，松开取消'};
  }
  function draw() {
    frame = null; if (!active || active.maxDistance < previewThreshold) return;
    ensureOverlay(); if (!overlay) return;
    clearTimeout(hideTimer); overlay.hidden = false; svg.style.display = '';
    line.setAttribute('points', active.points.map(point => point.join(',')).join(' '));
    text(classify(active).label);
  }
  function point(event) {
    if (!active) return;
    const dx = event.clientX-active.x, dy = event.clientY-active.y;
    active.length += Math.hypot(dx,dy); active.x=event.clientX; active.y=event.clientY;
    active.maxDistance = Math.max(active.maxDistance,Math.hypot(active.x-active.startX,active.y-active.startY));
    if (active.maxDistance >= threshold && !active.consumed) {
      active.consumed = true;
      try { document.documentElement.setPointerCapture(active.pointerId); } catch {}
    }
    const last = active.points[active.points.length-1];
    if (Math.hypot(active.x-last[0],active.y-last[1]) >= 3) {active.points.push([active.x,active.y]);if(active.points.length>128)active.points.splice(1,1);}
    if (!frame) frame = requestAnimationFrame(draw);
  }
  function clean() {
    const previous = active; active = null;
    if (frame) {cancelAnimationFrame(frame);frame=null;}
    if (previous) try { document.documentElement.releasePointerCapture(previous.pointerId); } catch {}
    if (overlay) overlay.hidden = true;
    return previous;
  }
  function cancelActive() {
    if (!active) return false;
    suppressUntil = now()+1000; clean(); notify('手势已取消'); return true;
  }
  function setEnabled(value) {enabled=!!value;if(!enabled)cancelActive();}
  function execute(action,state) {
    if (state.href !== location.href) {notify('页面已变化，手势已取消');return;}
    if (action === 'back') {
      if (!window.__stillDockNavigation?.isOpen?.()) {notify('当前已在浏览列表');return;}
      window.__stillDockNavigation.back(); return;
    }
    if (action === 'toggle_sidebar') {send({kind:'gesture',action});return;}
    const capture = window.__xhsReadCurrentImage?.();
    if (!capture || capture.error) {notify(capture?.error || '当前图片尚未加载完成');return;}
    if (state.noteId && state.noteId !== capture.note.id) {notify('笔记已变化，手势已取消');return;}
    send({kind:'gesture',action,note:capture.note,index:capture.index});
    notify('正在加入下载队列…');
  }
  document.addEventListener('pointerdown', event => {
    if (event.button!==2) return;
    suppressUntil=0;
    if (!enabled || event.pointerType!=='mouse' || event.buttons!==2 || event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) return;
    if (event.target?.closest?.('input,textarea,select,[contenteditable="true"],[role="textbox"]')) return;
    if (active) clean();
    let noteId='';try {noteId=window.__xhsReadNote?.().id||'';}catch{}
    active={startX:event.clientX,startY:event.clientY,x:event.clientX,y:event.clientY,started:now(),pointerId:event.pointerId,length:0,maxDistance:0,consumed:false,points:[[event.clientX,event.clientY]],href:location.href,noteId};
  },true);
  document.addEventListener('pointermove', event => {
    if (!active || event.pointerId!==active.pointerId) return;
    if (event.buttons!==2) {cancelActive();return;}
    point(event); if(active?.consumed)event.preventDefault();
  },true);
  document.addEventListener('pointerup', event => {
    if (!active || event.pointerId!==active.pointerId || event.button!==2) return;
    point(event); const state=clean(); if (!state.consumed) return;
    suppressUntil=now()+1000;event.preventDefault();event.stopImmediatePropagation();
    if (event.buttons!==0 || event.shiftKey || event.ctrlKey || event.altKey || event.metaKey || event.clientX<0 || event.clientY<0 || event.clientX>innerWidth || event.clientY>innerHeight) {notify('手势已取消');return;}
    const result=classify(state); if(result.action)execute(result.action,state);else notify('手势已取消');
  },true);
  document.addEventListener('contextmenu', event => {
    if (event.button!==2 && !active?.consumed) return;
    if (active?.consumed || now()<suppressUntil) {event.preventDefault();event.stopImmediatePropagation();if(!active)suppressUntil=0;}
  },true);
  document.addEventListener('keydown', event => {
    if (event.key==='Escape' && !event.isComposing && cancelActive()) {event.preventDefault();event.stopImmediatePropagation();}
  },true);
  document.addEventListener('pointercancel',()=>cancelActive(),true);
  document.addEventListener('lostpointercapture',event=>{if(active?.pointerId===event.pointerId)cancelActive();},true);
  addEventListener('blur',()=>cancelActive());
  window.__stillDockGestures={setEnabled,cancelActive,notify};
  send({kind:'gesture_ready'});
})();
