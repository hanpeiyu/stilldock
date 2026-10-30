//! Hidden WebView2 integration fixture. No Chrome, real account, or media downloads.
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
        .with_title("StillDock gesture integration fixture")
        .with_visible(false)
        .build(&events)?;
    let profile = std::env::current_dir()?.join("target/gesture-smoke-profile");
    std::fs::create_dir_all(&profile)?;
    let mut context = WebContext::new(Some(profile));
    let html = format!(
        r##"<!doctype html><html><meta charset="utf-8"><body>
<div id="noteContainer"><div class="media-container" style="width:600px;height:400px">
<div class="swiper-slide swiper-slide-active" data-swiper-slide-index="1"><img alt="fixture"></div>
</div></div><button class="close-circle" onclick="document.getElementById('noteContainer')?.remove()">close</button>
<script>
(function(location){{
window.__INITIAL_STATE__={{note:{{currentNoteId:'abcdef0123456789abcdef01',noteDetailMap:{{abcdef0123456789abcdef01:{{note:{{title:'fixture',imageList:[{{urlDefault:'https://sns-img-bd.xhscdn.com/first'}},{{urlDefault:'https://sns-img-bd.xhscdn.com/second',stream:{{h264:[{{width:720,height:1280,masterUrl:'https://sns-video-bd.xhscdn.com/live'}}]}}}}]}}}}}}}}}};
{}
{}
{}
function assert(value,label){{if(!value)throw Error(label);}}
function pointer(type,x,y,extra={{}}){{const event=new PointerEvent(type,{{bubbles:true,cancelable:true,pointerId:1,pointerType:'mouse',button:2,buttons:type==='pointerup'?0:2,clientX:x,clientY:y,...extra}});document.body.dispatchEvent(event);return event;}}
function menu(){{const event=new MouseEvent('contextmenu',{{bubbles:true,cancelable:true,button:2}});document.body.dispatchEvent(event);return event;}}
function stroke(x,y){{pointer('pointerdown',200,160);pointer('pointermove',x,y);pointer('pointerup',x,y);}}
setTimeout(()=>{{try{{
assert(window.__xhsReadCurrentImage().index===1,'visible image index');
stroke(300,160);assert(menu().defaultPrevented,'gesture menu suppressed');
stroke(200,260);assert(menu().defaultPrevented,'download menu suppressed');
pointer('pointerdown',200,160);pointer('pointermove',300,160);document.dispatchEvent(new KeyboardEvent('keydown',{{key:'Escape',bubbles:true,cancelable:true}}));pointer('pointerup',300,160);assert(menu().defaultPrevented,'Escape menu suppressed');assert(document.getElementById('noteContainer'),'Escape must not close note during a stroke');
stroke(210,165);assert(!menu().defaultPrevented,'normal right click preserved');
window.__stillDockGestures.setEnabled(false);stroke(300,160);assert(!menu().defaultPrevented,'disabled gestures preserve menu');
window.__stillDockGestures.setEnabled(true);stroke(110,160);assert(!document.getElementById('noteContainer'),'left gesture native close');
window.__stillDockGestures.notify('fixture complete');assert(document.querySelector('#stilldock-gesture-ui .gesture-hud').textContent==='fixture complete','real DOM feedback');assert(getComputedStyle(document.querySelector('#stilldock-gesture-ui svg')).display==='none','trail must be hidden after notification');
window.ipc.postMessage(JSON.stringify({{kind:'fixture_done',checks:10}}));
}}catch(error){{window.ipc.postMessage(JSON.stringify({{kind:'fixture_error',message:String(error)}}));}}}},100);
}})(new URL('https://www.xiaohongshu.com/search_result?keyword=fixture'));
</script></body></html>"##,
        include_str!("../assets/navigation.js"),
        include_str!("../assets/site.js"),
        include_str!("../assets/gestures.js").replace("__GESTURES_ENABLED__", "true")
    );
    let view = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(html)
        .with_ipc_handler(move |request| {
            let _ = proxy.send_event(request.body().clone());
        })
        .build(&window)?;
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut done = false;
    let mut error = None;
    let mut actions = Vec::new();
    events.run_return(|event, _, flow| {
        *flow = ControlFlow::WaitUntil(deadline);
        if let Event::UserEvent(raw) = event {
            if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                match value["kind"].as_str() {
                    Some("gesture") => actions.push(value),
                    Some("fixture_done") => {
                        done = true;
                        *flow = ControlFlow::Exit;
                    }
                    Some("fixture_error") => {
                        error = Some(value["message"].to_string());
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
    if !done {
        return Err("hidden WebView2 fixture timed out".into());
    }
    assert_eq!(actions.len(), 2, "exactly two native IPC gesture actions");
    assert_eq!(actions[0]["action"], "toggle_sidebar");
    assert_eq!(actions[1]["action"], "download_current");
    assert_eq!(actions[1]["index"], 1);
    assert_eq!(
        actions[1]["note"]["assets"][1]["live_url"],
        "https://sns-video-bd.xhscdn.com/live"
    );
    println!("Hidden WebView2 gesture smoke: 10 DOM checks and native IPC passed");
    Ok(())
}
