// Keep related searches useful by opening an ordinary, same-origin note search.
(() => {
  if(window.top!==window||!/(^|\.)xiaohongshu\.com$/.test(location.hostname)||window.__stillDockRelatedSearch)return;
  const headings=new Set(['大家都在搜','相关搜索']);
  const clean=text=>(text||'').replace(/\s+/g,' ').trim();
  function keywordFrom(target){
    const link=target.closest?.('a[href]');let raw=link?.getAttribute('href')||'',hrefWord='';
    if(raw){try{const url=new URL(raw,location.href);if(url.origin===location.origin&&url.pathname.replace(/\/$/,'')==='/search_result'&&!url.searchParams.get('noteId')&&!url.searchParams.get('note_id')){const word=clean(url.searchParams.get('keyword')||url.searchParams.get('query'));if(word)hrefWord=word;}}catch{}}
    if(target.closest?.('a[href*="/explore/"],a[href*="/discovery/item/"],a[href*="/search_result/"]'))return '';
    let card=target;
    for(let depth=0;card&&depth<7;depth++,card=card.parentElement){
      const text=clean(card.textContent);
      if(text.length>1200)break;
      if(!card.querySelectorAll||card.querySelectorAll('a[href*="/explore/"],a[href*="/discovery/item/"],a[href*="/search_result/"]').length)continue;
      if(![...card.querySelectorAll('span,p,h3,h4,div')].some(node=>headings.has(clean(node.textContent))))continue;
      if(hrefWord)return hrefWord;
      for(let row=target;row&&row!==card;row=row.parentElement){const word=clean(row.getAttribute?.('data-keyword')||row.textContent);if(word&&!headings.has(word)&&![...headings].some(h=>word.includes(h))&&word.length<=120)return word;}
      return '';
    }
    return '';
  }
  addEventListener('click',event=>{
    if(event.button!==0||event.ctrlKey||event.shiftKey||event.altKey||event.metaKey||event.defaultPrevented)return;
    const target=event.target;if(!target?.closest||target.closest('input,textarea,[contenteditable="true"]'))return;
    // Never hijack an ordinary result card, arbitrary link, or another application overlay.
    const word=keywordFrom(target);if(!word||word.length>120||/[\u0000-\u001f\u007f]/.test(word))return;
    event.preventDefault();event.stopImmediatePropagation();window.ipc.postMessage(JSON.stringify({kind:'keyword_search',keyword:word}));
  },true);
  window.__stillDockRelatedSearch={keywordFrom};
})();
