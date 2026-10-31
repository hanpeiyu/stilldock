//! Hidden WebView2 fixture: real shadow DOM, layout, virtualization, gestures and native IPC.
use serde_json::Value;
use std::time::{Duration, Instant};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::{WebContext, WebViewBuilder};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut events = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = events.create_proxy();
    let window = WindowBuilder::new()
        .with_title("StillDock material fixture")
        .with_inner_size(tao::dpi::LogicalSize::new(1100., 800.))
        .with_visible(false)
        .build(&events)?;
    let profile = std::env::current_dir()?.join("target/material-smoke-profile");
    std::fs::create_dir_all(&profile)?;
    let mut context = WebContext::new(Some(profile));
    let html = format!(
        r##"<!doctype html><html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:"><body>
<div class="note-detail-mask"><div id="noteContainer"><div class="media-container" style="width:600px;height:400px"></div><div class="interaction-container">fixture</div></div><button class="close-circle" onclick="document.querySelector('.note-detail-mask').remove()">close</button></div>
<script>(function(location){{
const id='abcdef0123456789abcdef01';
window.__INITIAL_STATE__={{note:{{currentNoteId:id,noteDetailMap:{{[id]:{{note:{{title:'素材网格测试',user:{{nickname:'fixture'}},imageList:Array.from({{length:120}},(_,i)=>({{urlDefault:'https://sns-img-bd.xhscdn.com/full'+i,urlPre:'https://sns-img-bd.xhscdn.com/thumb'+i,width:480,height:640,stream:i===2?{{h264:[{{masterUrl:'https://sns-video-bd.xhscdn.com/live',width:720,height:1280}}]}}:{{}}}}))}}}}}}}}}};
{}
{}
{}
{}
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));let checks=0;
function assert(value,label){{if(!value)throw Error(label);checks++;}}
function root(){{return document.getElementById('stilldock-material')?.shadowRoot;}}
function button(text){{return [...root().querySelectorAll('button')].find(n=>n.textContent===text);}}
function pointer(target,type,x,y){{target.dispatchEvent(new PointerEvent(type,{{bubbles:true,composed:true,cancelable:true,pointerId:8,pointerType:'mouse',button:2,buttons:type==='pointerup'?0:2,clientX:x,clientY:y}}));}}
setTimeout(async()=>{{try{{
await delay(100);let r=root();assert(r,'grid mounted');
assert(r.querySelectorAll('.card').length>0&&r.querySelectorAll('.card').length<40,'bounded card DOM');
assert(r.querySelector('.viewport').getBoundingClientRect().height>300,'real viewport layout');
assert(r.querySelector('.card').getBoundingClientRect().width>160,'card width');
assert(getComputedStyle(document.querySelector('.note-detail-mask')).visibility==='hidden','native detail suspended');
assert(document.querySelector('.note-detail-mask').inert,'native interactions disabled');
button('清空').click();r.querySelectorAll('.media')[2].click();assert(r.querySelector('.count').textContent==='已选 1 / 120','selection count');
const live=r.querySelector('.live-option input');live.checked=true;live.dispatchEvent(new Event('change'));
button('下载选中 1 张').click();window.__stillDockMaterial.result(true,'queued');assert(r.querySelector('.feedback').textContent==='queued','native acknowledgement');
r.querySelectorAll('.zoom')[2].click();assert(r.querySelector('dialog').open,'dialog opens');assert(window.__xhsReadCurrentImage().index===2,'preview gesture image identity');
const previewImage=r.querySelector('.preview-image');document.dispatchEvent(new KeyboardEvent('keydown',{{key:'Escape',bubbles:true,cancelable:true}}));assert(!r.querySelector('dialog').open&&!previewImage.hasAttribute('src'),'Escape releases preview');assert(document.getElementById('noteContainer'),'Escape retains note');
r.querySelectorAll('.zoom')[1].click();await delay(0);assert(r.querySelector('dialog').open&&window.__xhsReadCurrentImage().index===1,'queued old close cannot clear new preview');window.__stillDockMaterial.closePreview();await delay(0);
const media=r.querySelectorAll('.media')[2];pointer(media,'pointerdown',200,160);pointer(document.body,'pointermove',200,260);pointer(document.body,'pointerup',200,260);
const oldImages=[...r.querySelectorAll('.card img')];const viewport=r.querySelector('.viewport');viewport.scrollTop=10000;viewport.dispatchEvent(new Event('scroll'));await delay(120);
assert(r.querySelectorAll('.card').length<40,'scroll stays bounded');assert(oldImages.every(img=>!img.hasAttribute('src')),'recycled image sources released');
viewport.scrollTop=0;viewport.dispatchEvent(new Event('scroll'));await delay(120);assert(r.querySelectorAll('.card input')[2].checked,'selection survives recycling');
window.__stillDockMaterial.setStates({{note_id:id,expected_urls:window.__xhsReadNote().assets.map(a=>a.url),assets:Array.from({{length:120}},(_,index)=>({{index,state:index===2?'missing':'present',live_state:'new'}}))}});
const filter=r.querySelector('select');filter.value='needed';filter.dispatchEvent(new Event('change'));await delay(60);assert(r.querySelectorAll('.card').length===1&&r.querySelector('.card').dataset.index==='2','filtered card keeps source index');assert(r.querySelector('.file-state').textContent==='文件缺失','per-card local state');filter.value='all';filter.dispatchEvent(new Event('change'));await delay(60);

button('轮播视图').click();assert(!root(),'native carousel switch removes workspace');assert(!document.querySelector('.note-detail-mask').inert,'native inert restored');assert(getComputedStyle(document.querySelector('.note-detail-mask')).visibility==='visible','native visibility restored');
document.getElementById('stilldock-material-launcher').click();assert(root(),'grid can reopen');
window.__stillDockNavigation.back();await delay(380);assert(!document.getElementById('noteContainer')&&!root(),'return uses native close without remount');
window.ipc.postMessage(JSON.stringify({{kind:'fixture_done',checks}}));
}}catch(error){{window.ipc.postMessage(JSON.stringify({{kind:'fixture_error',message:String(error)}}));}}}},100);
}})(new URL('https://www.xiaohongshu.com/search_result?keyword=fixture'));</script></body></html>"##,
        include_str!("../assets/navigation.js"),
        include_str!("../assets/site.js"),
        include_str!("../assets/materials.js")
            .replace("__MATERIAL_CONFIG__", "{enabled:true,includeLive:false}"),
        include_str!("../assets/gestures.js").replace("__GESTURES_ENABLED__", "true")
    );
    let view = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(html)
        .with_ipc_handler(move |r| {
            let _ = proxy.send_event(r.body().clone());
        })
        .build(&window)?;
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut checks = None;
    let mut error = None;
    let mut downloads = Vec::new();
    let mut gestures = Vec::new();
    events.run_return(|event, _, flow| {
        *flow = ControlFlow::WaitUntil(deadline);
        if let Event::UserEvent(raw) = event {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                match v["kind"].as_str() {
                    Some("material_download") => downloads.push(v),
                    Some("gesture") => gestures.push(v),
                    Some("fixture_done") => {
                        checks = v["checks"].as_u64();
                        *flow = ControlFlow::Exit;
                    }
                    Some("fixture_error") => {
                        error = Some(v["message"].to_string());
                        *flow = ControlFlow::Exit;
                    }
                    _ => {}
                }
            }
        }
        if Instant::now() >= deadline {
            *flow = ControlFlow::Exit;
        }
    });
    drop(view);
    if let Some(error) = error {
        return Err(error.into());
    }
    let checks = checks.ok_or("material fixture timed out")?;
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0]["indices"], serde_json::json!([2]));
    assert_eq!(downloads[0]["include_live"], true);
    assert_eq!(
        downloads[0]["note"]["assets"].as_array().unwrap().len(),
        120
    );
    assert_eq!(gestures.len(), 1);
    assert_eq!(gestures[0]["action"], "download_current");
    assert_eq!(gestures[0]["index"], 2);
    println!("Hidden WebView2 material smoke: {checks} real DOM/layout checks and native selection/gesture IPC passed");
    Ok(())
}
