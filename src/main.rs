#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod download;
mod file_ops;
mod library;
mod local_files;
mod model;
mod store;

use download::{SharedStore, Work};
use fs2::FileExt;
use model::{select_assets, select_current_assets, site_allowed, valid_note, Note};
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
    Capture(String, Option<Vec<usize>>, bool, bool, String, Vec<String>),
    AssetsChecked(library::Snapshot),
    Notify,
    LocalChecked(Vec<(String, local_files::Status)>),
    Relinked(Result<(), String>),
    Navigate(String),
    Open(String),
    Loaded,
}

enum LocalRequest {
    Check(String),
    Relink(String, PathBuf),
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
fn material_config(store: &SharedStore) -> Value {
    json!({"enabled":setting(store,"material_view",true)&&!setting(store,"original",false),"includeLive":setting(store,"include_live",false)})
}
fn material_feedback(browser: &WebView, ok: bool, message: &str, note_id: &str) {
    let message = serde_json::to_string(message).unwrap_or_default();
    let note_id = serde_json::to_string(note_id).unwrap_or_default();
    let _ = browser.evaluate_script(&format!(
        "window.__stillDockMaterial?.result({ok},{message},{note_id});"
    ));
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
    format!(
        "{ai}\n{focus}\nwindow.__stillDockMaterial?.setConfig({});",
        material_config(store)
    )
}
fn snapshot(
    panel: &WebView,
    store: &SharedStore,
    note: &Note,
    collapsed: bool,
    started: Instant,
    local: &HashMap<String, local_files::Status>,
    selection: &Option<Vec<usize>>,
    assets: &Option<library::Snapshot>,
) {
    let db = store.lock().unwrap();
    send(
        panel,
        json!({"kind":"snapshot","asset_states":assets,"selection":selection,"material_view":db.setting("material_view","true")=="true","local":local,"jobs":db.jobs().unwrap_or_default(),"note":note,"collapsed":collapsed,
        "output_layout":db.setting("output_layout","note"),"export_json":db.setting("export_json","false")=="true","folder":db.setting("folder",""),"focus":db.setting("focus","true")=="true","ai":db.setting("ai","true")=="true",
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
    let options = store.lock().unwrap().download_options(&id)?;
    if controls
        .get(&id)
        .is_some_and(|signal| *signal.borrow() == 0)
    {
        return Err("任务仍在队列中，请等待状态更新".into());
    }
    if !valid_note(&note) {
        return Err("任务中的媒体地址无效，请重新打开笔记".into());
    }
    if !["pending", "paused", "failed", "done"].contains(&state.as_str()) {
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
        options,
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
    let boot_options = json!({"materials":setting(&store,"material_view",true)&&!setting(&store,"original",false),"focus":setting(&store,"focus",true)&&!setting(&store,"original",false),"ai":setting(&store,"ai",true)&&!setting(&store,"original",false)});
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
    let materials = include_str!("../assets/materials.js")
        .replace("__MATERIAL_CONFIG__", &material_config(&store).to_string());
    let initial = format!("{initial}\n{materials}");
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
    let (check_tx, check_rx) = std::sync::mpsc::sync_channel::<LocalRequest>(1);
    let check_store = store.clone();
    let check_proxy = proxy.clone();
    std::thread::spawn(move || {
        while let Ok(request) = check_rx.recv() {
            let note_id = match request {
                LocalRequest::Check(note_id) => note_id,
                LocalRequest::Relink(id, folder) => {
                    let result = (|| {
                        let target = {
                            check_store
                                .lock()
                                .unwrap()
                                .local_targets("")?
                                .into_iter()
                                .find(|t| t.id == id)
                                .ok_or("任务不存在或正在下载")?
                        };
                        let paths = local_files::relocated(&target, &folder)?;
                        check_store.lock().unwrap().relink(&target, &folder, &paths)
                    })();
                    let _ = check_proxy.send_event(Message::Relinked(result));
                    String::new()
                }
            };
            let targets = { check_store.lock().unwrap().local_targets(&note_id) };
            if let Ok(targets) = targets {
                let results = targets
                    .iter()
                    .map(|target| (target.id.clone(), local_files::check(target)))
                    .collect();
                if check_proxy
                    .send_event(Message::LocalChecked(results))
                    .is_err()
                {
                    break;
                }
            }
        }
    });
    let asset_proxy = proxy.clone();
    let asset_checker = library::Checker::start(store.clone(), move |snapshot| {
        let _ = asset_proxy.send_event(Message::AssetsChecked(snapshot));
    });
    let mut asset_states: Option<library::Snapshot> = None;
    let mut last_asset_check = Instant::now();
    let mut last_asset_version = String::new();
    let mut local: HashMap<String, local_files::Status> = HashMap::new();
    let _ = check_tx.try_send(LocalRequest::Check(String::new()));
    let mut controls: HashMap<String, watch::Sender<u8>> = HashMap::new();
    let mut gesture_jobs: HashSet<String> = HashSet::new();
    let mut collapsed = true;
    let mut current = Note::default();
    let mut selection: Option<Vec<usize>> = None;
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
                if store.lock().unwrap().busy(){collapsed=false;layout(&window,&browser,&panel,collapsed);snapshot(&panel,&store,&current,collapsed,started,&local,&selection,&asset_states);send(&panel,json!({"kind":"close"}));}else{*flow=ControlFlow::Exit;}
            }
            Event::UserEvent(Message::Loaded)=>{
                update=true;
            },
            Event::UserEvent(Message::Open(url))=>{let _=browser.load_url(&url);},
            Event::UserEvent(Message::Navigate(url))=>{
                if url::Url::parse(&url).is_ok_and(|u|u.path().trim_end_matches('/')=="/search_result"){collapsed=false;layout(&window,&browser,&panel,collapsed);update=true;}
            }
            Event::UserEvent(Message::Site(raw))=>{
                if let Ok(value)=serde_json::from_str::<Value>(&raw){match value["kind"].as_str(){
                    Some("search_start")=>{collapsed=false;layout(&window,&browser,&panel,collapsed);update=true;}
                    Some("page")=>{if let Ok(note)=serde_json::from_value::<Note>(value["note"].clone()){if site_allowed(&note.url){let newly_opened=!note.id.is_empty()&&note.id!=current.id;let sources_changed=note.id!=current.id||note.assets.iter().map(|a|(&a.url,&a.original_url,&a.live_url)).ne(current.assets.iter().map(|a|(&a.url,&a.original_url,&a.live_url)));if sources_changed{selection=model::remap_selection(&current,&note,selection.as_deref());asset_states=None;asset_checker.request(&note);let _=store.lock().unwrap().refresh_sources(&note);}current=note;if newly_opened{send(&panel,json!({"kind":"current_note"}));let _=check_tx.try_send(LocalRequest::Check(current.id.clone()));}update=true;}}}
                    Some("material_open")=>{if let Some(index)=value["index"].as_u64(){if let Some(path)=asset_states.as_ref().filter(|s|s.note_id==current.id).and_then(|s|s.assets.iter().find(|a|a.index==index as usize)).and_then(|a|a.path.as_ref()).and_then(|p|p.parent()){open_folder(path);}}},
                    Some("material_selection")=>{if value["note_id"].as_str()==Some(current.id.as_str())&&value["expected_urls"].as_array().is_some_and(|urls|urls.len()==current.assets.len()&&urls.iter().zip(&current.assets).all(|(url,asset)|url.as_str()==Some(asset.url.as_str()))){selection=value["indices"].as_array().map(|a|a.iter().filter_map(|i|i.as_u64().map(|i|i as usize)).filter(|i|*i<current.assets.len()).collect());update=true;}},
                    Some("material_live")=>{let enabled=value["value"].as_bool().unwrap_or(false);let _=store.lock().unwrap().set_setting("include_live",if enabled{"true"}else{"false"});update=true;},
                    Some("material_download")=>{
                        let result=(||->Result<(),String>{
                            if pause_exit||finish_exit{return Err("正在退出，请稍后重试".into());}
                            let captured:Note=serde_json::from_value(value["note"].clone()).map_err(|_|"笔记数据暂不可用")?;
                            let indices:Vec<usize>=value["indices"].as_array().ok_or("请选择图片")?.iter().map(|v|v.as_u64().map(|i|i as usize).ok_or("选图序号无效")).collect::<Result<_,_>>()?;
                            let note=select_current_assets(&current,captured,&indices,setting(&store,"include_live",false))?;
                            let root=PathBuf::from(store.lock().unwrap().setting("folder",""));
                            let id={store.lock().unwrap().add_with_policy(&note,&root,value["force"].as_bool().unwrap_or(false))?};
                            enqueue(&store,&tx,&mut controls,id)
                        })();
                        match result{Ok(())=>{collapsed=false;layout(&window,&browser,&panel,collapsed);material_feedback(&browser,true,"已加入下载队列",value["note"]["id"].as_str().unwrap_or(""));},Err(error)=>material_feedback(&browser,false,&error,value["note"]["id"].as_str().unwrap_or(""))}update=true;
                    },
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
            Event::UserEvent(Message::AssetsChecked(snapshot))=>{if snapshot.note_id==current.id&&snapshot.expected_urls.iter().map(String::as_str).eq(current.assets.iter().map(|a|a.url.as_str())){let encoded=serde_json::to_string(&snapshot).unwrap_or_default();let _=browser.evaluate_script(&format!("window.__stillDockMaterial?.setStates({encoded});"));asset_states=Some(snapshot);update=true;}},
            Event::UserEvent(Message::Relinked(result))=>{asset_checker.request(&current);match result{Ok(())=>toast(&panel,"已按文件名和大小关联新目录，未移动文件"),Err(error)=>toast(&panel,error)}update=true;}
            Event::UserEvent(Message::LocalChecked(results))=>{let completed:HashSet<_>=store.lock().unwrap().jobs().unwrap_or_default().into_iter().filter(|j|j.state=="done").map(|j|j.id).collect();local.retain(|id,_|completed.contains(id));for (id,status) in results{if completed.contains(&id){local.insert(id,status);}}update=true;}
            Event::UserEvent(Message::Notify)=>{
                let all_jobs=store.lock().unwrap().jobs().unwrap_or_default();let version=all_jobs.iter().filter(|j|j.note_id==current.id).map(|j|format!("{}:{}:{}",j.id,j.state,j.files)).collect::<Vec<_>>().join("|");let finished:Vec<_>=all_jobs.into_iter().filter(|j|!["pending","running","pausing","cancelling"].contains(&j.state.as_str())).collect();
                let mut recheck=false;for job in finished{if controls.remove(&job.id).is_some(){local.remove(&job.id);recheck=true;}if gesture_jobs.remove(&job.id){gesture_notice(&browser,match job.state.as_str(){"done"=>"当前图片已保存","failed"=>"下载失败，可在下载栏重试","paused"=>"下载已暂停",_=>"下载已取消"});}}
                if recheck{let _=check_tx.try_send(LocalRequest::Check(String::new()));}if version!=last_asset_version&&(recheck||last_asset_check.elapsed().as_secs_f32()>=1.0){asset_checker.request(&current);last_asset_version=version;last_asset_check=Instant::now();}
                if (finish_exit||pause_exit)&&!store.lock().unwrap().busy(){*flow=ControlFlow::Exit;}update=true;
            }
            Event::UserEvent(Message::Capture(raw,indices,include_live,force,expected_id,expected_urls))=>{
                let parsed=serde_json::from_str::<Value>(&raw).ok().and_then(|v|if let Some(s)=v.as_str(){serde_json::from_str::<Note>(s).ok()}else{serde_json::from_value::<Note>(v).ok()});
                if let Some(mut note)=parsed{
                    if note.id!=expected_id || (indices.is_some() && note.assets.iter().map(|a|a.url.as_str()).collect::<Vec<_>>()!=expected_urls.iter().map(String::as_str).collect::<Vec<_>>()) {toast(&panel,"笔记或图片列表已经变化，请重新选择图片");} else {
                    select_assets(&mut note,indices.as_deref(),include_live);
                    if valid_note(&note){let root=PathBuf::from(store.lock().unwrap().setting("folder",""));let result=store.lock().unwrap().add_with_policy(&note,&root,force);match result{Ok(id)=>match enqueue(&store,&tx,&mut controls,id){Ok(())=>toast(&panel,"已加入下载队列"),Err(e)=>toast(&panel,e)},Err(e)=>toast(&panel,e)}}else{toast(&panel,"未能读取媒体地址，请等待笔记加载完成后重试");}}
                }else{toast(&panel,"笔记数据暂不可用，请重新打开笔记");}update=true;
            }
            Event::UserEvent(Message::Ui(raw))=>{
                if let Ok(v)=serde_json::from_str::<Value>(&raw){let id=v["id"].as_str().unwrap_or("");match v["action"].as_str().unwrap_or(""){
                    "ready"=>update=true,
                    "check_local"=>{asset_checker.request(&current);let _=check_tx.try_send(LocalRequest::Check(String::new()));toast(&panel,"正在后台核对本地文件…");},
                    "relink"=>{if let Some(folder)=rfd::FileDialog::new().set_title("选择移动后的素材文件夹（按原文件名和大小核对）").pick_folder(){if check_tx.try_send(LocalRequest::Relink(id.to_owned(),folder)).is_err(){toast(&panel,"正在核对文件，请稍后重新关联");}else{toast(&panel,"正在核对新目录，全部匹配后更新关联…");}}},
                    "repair"=>{if !pause_exit&&!finish_exit&&local.get(id).is_some_and(|s|s.repairable()){
                        match enqueue(&store,&tx,&mut controls,id.to_owned()){Ok(())=>{local.remove(id);toast(&panel,"已加入补下队列，现有文件将跳过");},Err(e)=>toast(&panel,e)}
                    }else{toast(&panel,"请先核对文件，并确认保存目录可以访问");}update=true;},
                    "selection"=>{if v["note_id"].as_str()==Some(current.id.as_str()){selection=v["indices"].as_array().map(|a|a.iter().filter_map(|i|i.as_u64().map(|i|i as usize)).filter(|i|*i<current.assets.len()).collect());let encoded=serde_json::to_string(&selection).unwrap_or_else(|_|"null".into());let _=browser.evaluate_script(&format!("window.__stillDockMaterial?.setSelection({encoded});"));update=true;}},
                    "toggle"=>{collapsed=!collapsed;layout(&window,&browser,&panel,collapsed);update=true;},
                    "home"=>{let _=browser.load_url("https://www.xiaohongshu.com/explore");},
                    "back"=>{let _=browser.go_back();},"forward"=>{let _=browser.go_forward();},"reload"=>{let _=browser.reload();},
                    "download"=>{if pause_exit{toast(&panel,"正在退出，请稍后重试");}else{let indices=v["indices"].as_array().map(|a|a.iter().filter_map(|i|i.as_u64().map(|n|n as usize)).collect());let expected_urls:Vec<String>=v["expected_urls"].as_array().map(|items|items.iter().filter_map(|s|s.as_str().map(str::to_owned)).collect()).unwrap_or_default();let include_live=v["include_live"].as_bool().unwrap_or(false);let force=v["force"].as_bool().unwrap_or(false);let expected_id=v["note_id"].as_str().unwrap_or("").to_owned();let p=proxy.clone();if browser.evaluate_script_with_callback("JSON.stringify(window.__xhsReadNote?window.__xhsReadNote():null)",move|s|{let _=p.send_event(Message::Capture(s,indices.clone(),include_live,force,expected_id.clone(),expected_urls.clone()));}).is_err(){toast(&panel,"页面尚未就绪，请稍后重试");update=true;}}},
                    "pause"|"cancel"=>{control(&store,&controls,id,v["action"]=="cancel");update=true;},
                    "resume"=>{if !pause_exit{if let Err(e)=enqueue(&store,&tx,&mut controls,id.to_owned()){toast(&panel,e)}}update=true;},
                    "pause_all"|"resume_all"=>{let jobs=store.lock().unwrap().jobs().unwrap_or_default();for job in jobs{if v["action"]=="pause_all"{control(&store,&controls,&job.id,false);}else if ["paused","failed"].contains(&job.state.as_str()){let _=enqueue(&store,&tx,&mut controls,job.id);}}update=true;},
                    "open_folder"=>{let path=PathBuf::from(store.lock().unwrap().setting("folder",""));open_folder(&path);},
                    "open_job"=>{let payload={store.lock().unwrap().payload(id)};if let Ok((_,p,_))=payload{let referenced=store.lock().unwrap().file_records(id).ok().and_then(|files|files.iter().find(|(path,_)|path.parent()==Some(p.as_path())).or_else(||files.first()).and_then(|(path,_)|path.parent().map(Path::to_path_buf)));if let Some(folder)=referenced.filter(|folder|folder.is_dir()){open_folder(&folder);}else if p.is_dir(){open_folder(&p);}else{toast(&panel,"原目录不存在或暂时无法访问，请检查磁盘或打开笔记补下");}}},
                    "open_note"=>{if let Ok((n,_,_))=store.lock().unwrap().payload(id){if site_allowed(&n.url){let _=browser.load_url(&n.url);}}},
                    "link"=>{let url=v["url"].as_str().unwrap_or("").trim();if site_allowed(url){let _=browser.load_url(url);}else{toast(&panel,"只支持完整的 HTTPS 小红书链接");}},
                    "choose_folder"=>{if let Some(path)=rfd::FileDialog::new().set_title("选择下载文件夹").pick_folder(){let test=path.join(".xhs-write-check");match OpenOptions::new().create_new(true).write(true).open(&test){Ok(f)=>{drop(f);let _=std::fs::remove_file(test);let _=store.lock().unwrap().set_setting("folder",&path.to_string_lossy());toast(&panel,"已保存，新任务使用新目录");},Err(_)=>toast(&panel,"该目录不可写，请更换目录")}}update=true;},
                    "setting"=>{if let Some(key)=v["key"].as_str(){if key=="output_layout"{if let Some(layout)=v["value"].as_str(){if ["note","flat"].contains(&layout){let _=store.lock().unwrap().set_setting(key,layout);}}}else if key=="export_json"{let _=store.lock().unwrap().set_setting(key,if v["value"].as_bool().unwrap_or(false){"true"}else{"false"});}else if key=="gestures"{let enabled=v["value"].as_bool().unwrap_or(true);let _=store.lock().unwrap().set_setting(key,if enabled{"true"}else{"false"});let _=browser.evaluate_script(&format!("window.__stillDockGestures?.setEnabled({enabled});"));}else if key=="include_live"{let _=store.lock().unwrap().set_setting(key,if v["value"].as_bool().unwrap_or(false){"true"}else{"false"});let _=browser.evaluate_script(&format!("window.__stillDockMaterial?.setConfig({});",material_config(&store)));}else if ["focus","ai","comments","original","material_view"].contains(&key){let _=store.lock().unwrap().set_setting(key,if v["value"].as_bool().unwrap_or(false){"true"}else{"false"});let _=browser.evaluate_script(&adapters(&store));let initial=format!("window.__xhsInstallAdapters=()=>{{{}}}",adapters(&store));let _=browser.evaluate_script(&initial);toast(&panel,"已更新浏览模式");}}update=true;},
                    "pause_exit"=>{pause_exit=true;let jobs=store.lock().unwrap().jobs().unwrap_or_default();for job in jobs{control(&store,&controls,&job.id,false);}if !store.lock().unwrap().busy(){*flow=ControlFlow::Exit;}update=true;},
                    "finish_exit"=>{finish_exit=true;toast(&panel,"队列处理结束后退出");if !store.lock().unwrap().busy(){*flow=ControlFlow::Exit;}},
                    "stay"=>{finish_exit=false;},_=>{}
                }}
            }
            _=>{}
        }
        if update{snapshot(&panel,&store,&current,collapsed,started,&local,&selection,&asset_states);}
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
