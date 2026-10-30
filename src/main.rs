#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod download;
mod model;
mod store;

use download::{SharedStore, Work};
use fs2::FileExt;
use model::{select_assets, site_allowed, valid_note, Note};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::WindowBuilderExtWindows,
    window::{Icon, Window, WindowBuilder},
};
use tokio::sync::watch;
use wry::{
    dpi::{PhysicalPosition, PhysicalSize},
    NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder,
};

#[derive(Debug)]
enum Message {
    Ui(String),
    Site(String),
    Capture(String, Option<Vec<usize>>, bool, String, Vec<String>),
    Notify,
    Navigate(String),
    Open(String),
    Loaded,
}

fn layout(window: &Window, browser: &WebView, panel: &WebView, collapsed: bool) {
    let size = window.inner_size();
    let scale = window.scale_factor();
    let sidebar = ((if collapsed { 44.0 } else { 350.0 }) * scale) as u32;
    let sidebar = sidebar.min(size.width.saturating_sub(240)).max(1);
    let left = size.width.saturating_sub(sidebar);
    let _ = browser.set_bounds(Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: PhysicalSize::new(left, size.height).into(),
    });
    let _ = panel.set_bounds(Rect {
        position: PhysicalPosition::new(left as i32, 0).into(),
        size: PhysicalSize::new(sidebar, size.height).into(),
    });
}

fn send(panel: &WebView, value: Value) {
    let _ = panel.evaluate_script(&format!("window.receive&&window.receive({value});"));
}
fn toast(panel: &WebView, message: impl Into<String>) {
    send(panel, json!({"kind":"toast","message":message.into()}));
}
fn gesture_notice(browser: &WebView, message: &str) {
    let encoded = serde_json::to_string(message).unwrap_or_default();
    let _ = browser.evaluate_script(&format!("window.__stillDockGestures?.notify({encoded});"));
}
fn setting(store: &SharedStore, key: &str, default: bool) -> bool {
    store
        .lock()
        .unwrap()
        .setting(key, if default { "true" } else { "false" })
        == "true"
}
fn adapters(store: &SharedStore) -> String {
    let original = setting(store, "original", false);
    let focus = json!({"enabled":setting(store,"focus",true)&&!original,"foldComments":setting(store,"comments",true),"downloaded":[]});
    let focus = include_str!("../assets/focus.js").replace("__CONFIG__", &focus.to_string());
    let ai = include_str!("../assets/ai-filter.js").replace(
        "__ENABLED__",
        if setting(store, "ai", true) && !original {
            "true"
        } else {
            "false"
        },
    );
    format!("{ai}\n{focus}")
}
fn snapshot(panel: &WebView, store: &SharedStore, note: &Note, collapsed: bool, started: Instant) {
    let db = store.lock().unwrap();
    send(
        panel,
        json!({"kind":"snapshot","jobs":db.jobs().unwrap_or_default(),"note":note,"collapsed":collapsed,
        "folder":db.setting("folder",""),"focus":db.setting("focus","true")=="true","ai":db.setting("ai","true")=="true",
        "gestures":db.setting("gestures","true")=="true","include_live":db.setting("include_live","false")=="true","comments":db.setting("comments","true")=="true","original":db.setting("original","false")=="true",
        "about":format!("Windows 10/11 · WebView2 · v{} · 本次运行 {:.0} 秒",env!("CARGO_PKG_VERSION"),started.elapsed().as_secs_f64())}),
    );
}
fn open_folder(path: &Path) {
    if path.is_dir() {
        let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
    }
}
fn enqueue(
    store: &SharedStore,
    tx: &std::sync::mpsc::Sender<Work>,
    controls: &mut HashMap<String, watch::Sender<u8>>,
    id: String,
) -> Result<(), String> {
    let (note, folder, state) = store.lock().unwrap().payload(&id)?;
    if controls
        .get(&id)
        .is_some_and(|signal| *signal.borrow() == 0)
    {
        return Err("任务仍在队列中，请等待状态更新".into());
    }
    if !valid_note(&note) {
        return Err("任务中的媒体地址无效，请重新打开笔记".into());
    }
    if !["pending", "paused", "failed"].contains(&state.as_str()) {
        return Err("任务已在进行或已经结束".into());
    }
    store.lock().unwrap().ensure_available(&note, &id)?;
    let (signal, receiver) = watch::channel(0);
    controls.insert(id.clone(), signal);
    store.lock().unwrap().state(&id, "pending", "等待下载");
    tx.send(Work {
        id: id.clone(),
        note,
        folder,
        control: receiver,
    })
    .map_err(|_| {
        store
            .lock()
            .unwrap()
            .state(&id, "failed", "下载服务不可用，请重启程序");
        "下载服务不可用".to_owned()
    })
}
fn control(
    store: &SharedStore,
    controls: &HashMap<String, watch::Sender<u8>>,
    id: &str,
    cancel: bool,
) {
    let state = store
        .lock()
        .unwrap()
        .payload(id)
        .map(|v| v.2)
        .unwrap_or_default();
    if !["pending", "running", "paused", "failed"].contains(&state.as_str()) {
        return;
    }
    if let Some(tx) = controls.get(id) {
        let _ = tx.send(if cancel { 2 } else { 1 });
    }
    let state = if state == "running" {
        if cancel {
            "cancelling"
        } else {
            "pausing"
        }
    } else if cancel {
        "cancelled"
    } else {
        "paused"
    };
    store.lock().unwrap().state(
        id,
        state,
        if cancel {
            "已请求取消，文件保留"
        } else {
            "已请求暂停，进度保留"
        },
    );
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("未找到本机数据目录")?)
        .join("XhsBrowserRust");
    std::fs::create_dir_all(&base)?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(base.join("instance.lock"))?;
    if lock.try_lock_exclusive().is_err() {
        rfd::MessageDialog::new()
            .set_title("原帧")
            .set_description("程序已经运行，请使用现有窗口。")
            .show();
        return Ok(());
    }
    let store = Arc::new(Mutex::new(store::Store::open(
        &base.join("library.sqlite3"),
    )?));
    if store.lock().unwrap().setting("folder", "").is_empty() {
        let path = PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default())
            .join("Downloads")
            .join("XHS");
        std::fs::create_dir_all(&path)?;
        store
            .lock()
            .unwrap()
            .set_setting("folder", &path.to_string_lossy())?;
    }
    let event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let app_icon = Icon::from_rgba(
        include_bytes!("../assets/brand/window-icon.rgba").to_vec(),
        64,
        64,
    )?;
    let window = WindowBuilder::new()
        .with_window_icon(Some(app_icon.clone()))
        .with_taskbar_icon(Some(app_icon))
        .with_title("StillDock · 原帧")
        .with_inner_size(LogicalSize::new(1380.0, 860.0))
        .with_min_inner_size(LogicalSize::new(900.0, 600.0))
        .build(&event_loop)?;
    let mut context = WebContext::new(Some(base.join("browser-profile")));
    let p = proxy.clone();
    let p2 = proxy.clone();
    let boot_options = json!({"focus":setting(&store,"focus",true)&&!setting(&store,"original",false),"ai":setting(&store,"ai",true)&&!setting(&store,"original",false)});
    let bootstrap = include_str!("../assets/bootstrap.js")
        .replace("__BOOT_CONFIG__", &boot_options.to_string())
        .replace(
            "__BRAND_ICON__",
            include_str!("../assets/brand/icon-64.data-uri.txt").trim(),
        );
    let initial = format!(
        "{}\nwindow.__xhsInstallAdapters=()=>{{{}}};\n{}",
        bootstrap,
        adapters(&store),
        format!(
            "{}\n{}",
            include_str!("../assets/navigation.js"),
            include_str!("../assets/site.js")
        )
    );
    let gestures = include_str!("../assets/gestures.js").replace(
        "__GESTURES_ENABLED__",
        if setting(&store, "gestures", true) {
            "true"
        } else {
            "false"
        },
    );
    let initial = format!("{initial}\n{gestures}");
    let browser = WebViewBuilder::new_with_web_context(&mut context)
        .with_url("https://www.xiaohongshu.com/explore")
        .with_bounds(Rect {
            position: PhysicalPosition::new(0, 0).into(),
            size: PhysicalSize::new(1200, 860).into(),
        })
        .with_background_color((24, 24, 27, 255))
        .with_autoplay(false)
        .with_initialization_script_for_main_only(initial, true)
        .with_ipc_handler(move |request| {
            if site_allowed(&request.uri().to_string()) && request.body().len() < 1_048_576 {
                let _ = p.send_event(Message::Site(request.body().clone()));
            }
        })
        .with_navigation_handler(move |url| {
            let allowed = site_allowed(&url);
            if allowed {
                let _ = p2.send_event(Message::Navigate(url));
            }
            allowed
        })
        .with_new_window_req_handler({
            let p = proxy.clone();
            move |url, _| {
                if site_allowed(&url) {
                    let _ = p.send_event(Message::Open(url));
                }
                NewWindowResponse::Deny
            }
        })
        .build_as_child(&window)?;
    let p = proxy.clone();
    let p2 = proxy.clone();
    let panel = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(include_str!("../assets/panel.html").replace(
            "__BRAND_ICON__",
            include_str!("../assets/brand/icon-64.data-uri.txt").trim(),
        ))
        .with_autoplay(false)
        .with_background_color((24, 24, 27, 255))
        .with_focused(false)
        .with_bounds(Rect {
            position: PhysicalPosition::new(1200, 0).into(),
            size: PhysicalSize::new(44, 860).into(),
        })
        .with_navigation_handler(|url| {
            url == "about:blank" || url.starts_with("data:text/html") || url.is_empty()
        })
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_ipc_handler(move |r| {
            if r.body().len() < 262_144 {
                let _ = p.send_event(Message::Ui(r.body().clone()));
            }
        })
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = p2.send_event(Message::Loaded);
            }
        })
        .build_as_child(&window)?;
    let p = proxy.clone();
    let tx = download::start(store.clone(), move || {
        let _ = p.send_event(Message::Notify);
    });
    let mut controls: HashMap<String, watch::Sender<u8>> = HashMap::new();
    let mut gesture_jobs: HashSet<String> = HashSet::new();
    let mut collapsed = true;
    let mut current = Note::default();
    let mut finish_exit = false;
    let mut pause_exit = false;
    layout(&window, &browser, &panel, collapsed);
    std::fs::write(base.join("startup.json"),json!({"browser_created_ms":started.elapsed().as_millis(),"version":env!("CARGO_PKG_VERSION")}).to_string())?;
    event_loop.run(move|event,_,flow|{
        let _keep_context=&context;let _keep_lock=&lock;
        *flow=ControlFlow::Wait;
        let mut update=false;
        match event{
            Event::WindowEvent{event:WindowEvent::Resized(_),..}=>layout(&window,&browser,&panel,collapsed),
            Event::WindowEvent{event:WindowEvent::ScaleFactorChanged{..},..}=>layout(&window,&browser,&panel,collapsed),
            Event::WindowEvent{event:WindowEvent::CloseRequested,..}=>{
                if store.lock().unwrap().busy(){collapsed=false;layout(&window,&browser,&panel,collapsed);snapshot(&panel,&store,&current,collapsed,started);send(&panel,json!({"kind":"close"}));}else{*flow=ControlFlow::Exit;}
            }
            Event::UserEvent(Message::Loaded)=>{
                update=true;
            },
            Event::UserEvent(Message::Open(url))=>{let _=browser.load_url(&url);},
            Event::UserEvent(Message::Navigate(url))=>{
                if url::Url::parse(&url).is_ok_and(|u|u.path().trim_end_matches('/')=="/search_result"||u.path().starts_with("/search_result/")){collapsed=false;layout(&window,&browser,&panel,collapsed);update=true;}
            }
            Event::UserEvent(Message::Site(raw))=>{
                if let Ok(value)=serde_json::from_str::<Value>(&raw){match value["kind"].as_str(){
                    Some("search_start")=>{collapsed=false;layout(&window,&browser,&panel,collapsed);update=true;}
                    Some("page")=>{if let Ok(note)=serde_json::from_value::<Note>(value["note"].clone()){if site_allowed(&note.url){let newly_opened=!note.id.is_empty()&&note.id!=current.id;current=note;if newly_opened{send(&panel,json!({"kind":"current_note"}));}update=true;}}}
                    Some("gesture_ready")=>{let _=browser.evaluate_script(&format!("window.__stillDockGestures?.setEnabled({});",setting(&store,"gestures",true)));}
                    Some("gesture")=>{if setting(&store,"gestures",true){match value["action"].as_str(){
                        Some("toggle_sidebar")=>{collapsed=!collapsed;layout(&window,&browser,&panel,collapsed);gesture_notice(&browser,if collapsed{"下载栏已收起"}else{"下载栏已展开"});update=true;}
                        Some("download_current")=>{
                            if pause_exit||finish_exit{gesture_notice(&browser,"正在退出，请稍后重试");}else if let (Ok(mut note),Some(index))=(serde_json::from_value::<Note>(value["note"].clone()),value["index"].as_u64()){
                                if (index as usize)<note.assets.len()&&!note.assets[index as usize].video{
                                    select_assets(&mut note,Some(&[index as usize]),setting(&store,"include_live",false));
                                    if valid_note(&note){let root=PathBuf::from(store.lock().unwrap().setting("folder",""));let result=store.lock().unwrap().add(&note,&root);
                                        match result{Ok(id)=>match enqueue(&store,&tx,&mut controls,id.clone()){Ok(())=>{gesture_jobs.insert(id);gesture_notice(&browser,"当前图片已加入下载队列");},Err(error)=>gesture_notice(&browser,&error)},Err(error)=>gesture_notice(&browser,&error)}
                                    }else{gesture_notice(&browser,"当前图片尚未加载完成，请重试");}
                                }else{gesture_notice(&browser,"无法确认当前图片，请使用侧栏选图");}
                            }else{gesture_notice(&browser,"当前图片尚未加载完成，请重试");}update=true;
                        }
                        _=>{}
                    }}}
                    _=>{}
                }}
            }
            Event::UserEvent(Message::Notify)=>{
                let finished:Vec<_>=store.lock().unwrap().jobs().unwrap_or_default().into_iter().filter(|j|!["pending","running","pausing","cancelling"].contains(&j.state.as_str())).collect();
                for job in finished{controls.remove(&job.id);if gesture_jobs.remove(&job.id){gesture_notice(&browser,match job.state.as_str(){"done"=>"当前图片已保存","failed"=>"下载失败，可在下载栏重试","paused"=>"下载已暂停",_=>"下载已取消"});}}
                if (finish_exit||pause_exit)&&!store.lock().unwrap().busy(){*flow=ControlFlow::Exit;}update=true;
            }
            Event::UserEvent(Message::Capture(raw,indices,include_live,expected_id,expected_urls))=>{
                let parsed=serde_json::from_str::<Value>(&raw).ok().and_then(|v|if let Some(s)=v.as_str(){serde_json::from_str::<Note>(s).ok()}else{serde_json::from_value::<Note>(v).ok()});
                if let Some(mut note)=parsed{
                    if note.id!=expected_id || (indices.is_some() && note.assets.iter().map(|a|a.url.as_str()).collect::<Vec<_>>()!=expected_urls.iter().map(String::as_str).collect::<Vec<_>>()) {toast(&panel,"笔记或图片列表已经变化，请重新选择图片");} else {
                    select_assets(&mut note,indices.as_deref(),include_live);
                    if valid_note(&note){let root=PathBuf::from(store.lock().unwrap().setting("folder",""));let result=store.lock().unwrap().add(&note,&root);match result{Ok(id)=>match enqueue(&store,&tx,&mut controls,id){Ok(())=>toast(&panel,"已加入下载队列"),Err(e)=>toast(&panel,e)},Err(e)=>toast(&panel,e)}}else{toast(&panel,"未能读取媒体地址，请等待笔记加载完成后重试");}}
                }else{toast(&panel,"笔记数据暂不可用，请重新打开笔记");}update=true;
            }
            Event::UserEvent(Message::Ui(raw))=>{
                if let Ok(v)=serde_json::from_str::<Value>(&raw){let id=v["id"].as_str().unwrap_or("");match v["action"].as_str().unwrap_or(""){
                    "ready"=>update=true,
                    "toggle"=>{collapsed=!collapsed;layout(&window,&browser,&panel,collapsed);update=true;},
                    "home"=>{let _=browser.load_url("https://www.xiaohongshu.com/explore");},
                    "back"=>{let _=browser.go_back();},"forward"=>{let _=browser.go_forward();},"reload"=>{let _=browser.reload();},
                    "download"=>{if pause_exit{toast(&panel,"正在退出，请稍后重试");}else{let indices=v["indices"].as_array().map(|a|a.iter().filter_map(|i|i.as_u64().map(|n|n as usize)).collect());let expected_urls:Vec<String>=v["expected_urls"].as_array().map(|items|items.iter().filter_map(|s|s.as_str().map(str::to_owned)).collect()).unwrap_or_default();let include_live=v["include_live"].as_bool().unwrap_or(false);let expected_id=v["note_id"].as_str().unwrap_or("").to_owned();let p=proxy.clone();if browser.evaluate_script_with_callback("JSON.stringify(window.__xhsReadNote?window.__xhsReadNote():null)",move|s|{let _=p.send_event(Message::Capture(s,indices.clone(),include_live,expected_id.clone(),expected_urls.clone()));}).is_err(){toast(&panel,"页面尚未就绪，请稍后重试");update=true;}}},
                    "pause"|"cancel"=>{control(&store,&controls,id,v["action"]=="cancel");update=true;},
                    "resume"=>{if !pause_exit{if let Err(e)=enqueue(&store,&tx,&mut controls,id.to_owned()){toast(&panel,e)}}update=true;},
                    "pause_all"|"resume_all"=>{let jobs=store.lock().unwrap().jobs().unwrap_or_default();for job in jobs{if v["action"]=="pause_all"{control(&store,&controls,&job.id,false);}else if ["paused","failed"].contains(&job.state.as_str()){let _=enqueue(&store,&tx,&mut controls,job.id);}}update=true;},
                    "open_folder"=>{let path=PathBuf::from(store.lock().unwrap().setting("folder",""));open_folder(&path);},
                    "open_job"=>{if let Ok((_,p,_))=store.lock().unwrap().payload(id){open_folder(&p);}},
                    "open_note"=>{if let Ok((n,_,_))=store.lock().unwrap().payload(id){if site_allowed(&n.url){let _=browser.load_url(&n.url);}}},
                    "link"=>{let url=v["url"].as_str().unwrap_or("").trim();if site_allowed(url){let _=browser.load_url(url);}else{toast(&panel,"只支持完整的 HTTPS 小红书链接");}},
                    "choose_folder"=>{if let Some(path)=rfd::FileDialog::new().set_title("选择下载文件夹").pick_folder(){let test=path.join(".xhs-write-check");match OpenOptions::new().create_new(true).write(true).open(&test){Ok(f)=>{drop(f);let _=std::fs::remove_file(test);let _=store.lock().unwrap().set_setting("folder",&path.to_string_lossy());toast(&panel,"已保存，新任务使用新目录");},Err(_)=>toast(&panel,"该目录不可写，请更换目录")}}update=true;},
                    "setting"=>{if let Some(key)=v["key"].as_str(){if key=="gestures"{let enabled=v["value"].as_bool().unwrap_or(true);let _=store.lock().unwrap().set_setting(key,if enabled{"true"}else{"false"});let _=browser.evaluate_script(&format!("window.__stillDockGestures?.setEnabled({enabled});"));}else if key=="include_live"{let _=store.lock().unwrap().set_setting(key,if v["value"].as_bool().unwrap_or(false){"true"}else{"false"});}else if ["focus","ai","comments","original"].contains(&key){let _=store.lock().unwrap().set_setting(key,if v["value"].as_bool().unwrap_or(false){"true"}else{"false"});let _=browser.evaluate_script(&adapters(&store));let initial=format!("window.__xhsInstallAdapters=()=>{{{}}}",adapters(&store));let _=browser.evaluate_script(&initial);toast(&panel,"已更新浏览模式");}}update=true;},
                    "pause_exit"=>{pause_exit=true;let jobs=store.lock().unwrap().jobs().unwrap_or_default();for job in jobs{control(&store,&controls,&job.id,false);}if !store.lock().unwrap().busy(){*flow=ControlFlow::Exit;}update=true;},
                    "finish_exit"=>{finish_exit=true;toast(&panel,"队列处理结束后退出");if !store.lock().unwrap().busy(){*flow=ControlFlow::Exit;}},
                    "stay"=>{finish_exit=false;},_=>{}
                }}
            }
            _=>{}
        }
        if update{snapshot(&panel,&store,&current,collapsed,started);}
    });
}

fn main() {
    if let Err(error) = run() {
        let message =
            format!("启动失败：{error}\n\n请确认已安装 Microsoft Edge WebView2 Runtime。");
        eprintln!("{message}");
        rfd::MessageDialog::new()
            .set_title("原帧 · 启动失败")
            .set_description(message)
            .show();
    }
}
