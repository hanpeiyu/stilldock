use crate::{
    model::{file_extension, media_allowed, safe_name, Asset, Note},
    store::Store,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
};

pub type SharedStore = Arc<Mutex<Store>>;
pub struct Work {
    pub id: String,
    pub note: Note,
    pub folder: PathBuf,
    pub control: watch::Receiver<u8>,
}

pub fn start(
    store: SharedStore,
    notify: impl Fn() + Send + 'static,
) -> std::sync::mpsc::Sender<Work> {
    let (tx, rx) = std::sync::mpsc::channel::<Work>();
    std::thread::Builder::new().name("downloads".into()).spawn(move|| {
        let rt=tokio::runtime::Builder::new_current_thread().enable_all().build().expect("download runtime");
        let client=reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).read_timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::custom(|a|if a.previous().len()<5&&media_allowed(a.url().as_str()){a.follow()}else{a.stop()}))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/146.0.0.0 Safari/537.36")
            .build().expect("HTTP client");
        for mut work in rx {
            if *work.control.borrow()!=0 {continue}
            if store.lock().unwrap().payload(&work.id).is_ok_and(|x|x.2!="pending") {continue}
            store.lock().unwrap().state(&work.id,"running","正在下载");notify();
            let result=rt.block_on(run(&client,&store,&mut work,&notify));
            let signal=*work.control.borrow();
            let (state,msg)=match (signal,result) {
                (1,_) => ("paused","已暂停，保留已下载进度".to_owned()),
                (2,_) => ("cancelled","已取消，已下载文件保留".to_owned()),
                (_,Ok(summary)) => ("done",summary),
                (_,Err(e)) => ("failed",e),
            };
            store.lock().unwrap().state(&work.id,state,&msg);notify();
        }
    }).expect("download thread");
    tx
}

#[derive(Debug)]
struct Transfer {
    part: PathBuf,
    bytes: u64,
    ext: &'static str,
    width: usize,
    height: usize,
}

async fn transfer_file(
    client: &reqwest::Client,
    store: &SharedStore,
    work: &mut Work,
    asset: &Asset,
    index: usize,
    completed: u64,
    notify: &impl Fn(),
    url: &str,
    high_quality: bool,
) -> Result<Transfer, String> {
    let part = work.folder.join(format!(
        "{:03}{}.part",
        index + 1,
        if high_quality { "-hq" } else { "" }
    ));
    let mut offset = tokio::fs::metadata(&part)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let mut response = None;
    for attempt in 0..2 {
        let mut request = client
            .get(url)
            .header("Referer", "https://www.xiaohongshu.com/");
        if offset > 0 {
            request = request.header("Range", format!("bytes={offset}-"));
        }
        let r = tokio::select! { biased;
            _=work.control.changed()=>return Err("任务已停止".into()),
            r=request.send()=>r.map_err(|_|"网络连接失败或超时，请重试")?,
        };
        if r.status().as_u16() == 416 && attempt == 0 {
            offset = 0;
            continue;
        }
        response = Some(r);
        break;
    }
    let mut response = response.ok_or("服务器未返回文件")?;
    let code = response.status().as_u16();
    if !response.status().is_success() {
        return Err(match code {
            401 | 403 => "媒体地址不可访问，请重新打开笔记再下载",
            404 | 410 => "媒体链接已失效，请重新打开笔记",
            429 => "请求过于频繁，请稍后再试",
            _ => "媒体服务器返回错误，请重试",
        }
        .into());
    }
    if code == 200 {
        offset = 0
    }
    if code == 206 {
        let range = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let start = range
            .strip_prefix("bytes ")
            .and_then(|v| v.split('-').next())
            .and_then(|v| v.parse::<u64>().ok());
        if start != Some(offset) {
            return Err("服务器续传位置不匹配，已保留临时文件".into());
        }
    }
    if response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/") || v.contains("json"))
    {
        return Err("媒体服务器返回了网页内容，请重新打开笔记".into());
    }
    let total = response.content_length().map(|n| n + offset);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .append(offset > 0)
        .open(&part)
        .await
        .map_err(|_| "无法写入临时文件")?;
    let mut bytes = offset;
    let mut tick = Instant::now();
    loop {
        let chunk = tokio::select! {biased;
            _=work.control.changed()=>{file.flush().await.map_err(|_|"保存下载进度失败")?;return Err("任务已停止".into())},
            c=response.chunk()=>c.map_err(|_|"下载连接中断，临时文件已保留，可重试")?,
        };
        let Some(chunk) = chunk else { break };
        file.write_all(&chunk)
            .await
            .map_err(|_| "写入失败，请检查磁盘空间与目录权限")?;
        bytes += chunk.len() as u64;
        if tick.elapsed() >= Duration::from_millis(300) {
            store.lock().unwrap().progress(
                &work.id,
                completed + bytes,
                completed + total.unwrap_or(bytes),
            );
            notify();
            tick = Instant::now();
        }
    }
    file.flush().await.map_err(|_| "保存文件失败")?;
    file.sync_all().await.map_err(|_| "文件落盘失败")?;
    drop(file);
    if bytes == 0 || total.is_some_and(|t| t != bytes) {
        return Err("文件长度校验失败，可继续下载".into());
    }
    let mut header = [0u8; 32];
    let mut input = tokio::fs::File::open(&part)
        .await
        .map_err(|_| "无法校验文件")?;
    let len = input
        .read(&mut header)
        .await
        .map_err(|_| "无法读取文件头")?;
    drop(input);
    let ext = file_extension(&header[..len], asset.video);
    if ext == "bin" {
        return Err("文件格式无法识别，已保留临时文件供重试".into());
    }
    let (width, height) = if asset.video {
        (0, 0)
    } else {
        let size = imagesize::size(&part).map_err(|_| "无法读取图片分辨率，文件可能无效")?;
        if high_quality
            && asset.width > 0
            && asset.height > 0
            && (size.width as u64 * size.height as u64) < (asset.width as u64 * asset.height as u64)
        {
            return Err("高清候选图片分辨率低于笔记标注尺寸".into());
        }
        (size.width, size.height)
    };
    Ok(Transfer {
        part,
        bytes,
        ext,
        width,
        height,
    })
}

async fn run(
    client: &reqwest::Client,
    store: &SharedStore,
    work: &mut Work,
    notify: &impl Fn(),
) -> Result<String, String> {
    tokio::fs::create_dir_all(&work.folder)
        .await
        .map_err(|_| "无法创建下载目录，请检查路径与权限")?;
    let metadata = serde_json::to_vec_pretty(&work.note).map_err(|_| "笔记信息保存失败")?;
    tokio::fs::write(work.folder.join("note.json"), metadata)
        .await
        .map_err(|_| "笔记信息保存失败")?;
    let report_path = work.folder.join("download-report.json");
    let mut report: Vec<serde_json::Value> = tokio::fs::read(&report_path)
        .await
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default();
    let mut completed = 0;
    for (index, asset) in work.note.assets.clone().iter().enumerate() {
        if *work.control.borrow() != 0 {
            return Err("任务已停止".into());
        }
        if let Some((path, size)) = store.lock().unwrap().file(&work.id, index) {
            if path.parent() == Some(work.folder.as_path())
                && std::fs::metadata(&path).is_ok_and(|m| m.len() == size && size > 0)
            {
                completed += size;
                continue;
            }
        }
        store.lock().unwrap().state(
            &work.id,
            "running",
            &format!("正在处理第 {}/{} 项", index + 1, work.note.assets.len()),
        );
        notify();
        let mut fallback_reason = String::new();
        let mut source = if asset.video { "video" } else { "web" };
        let transfer =
            if !asset.video && !asset.original_url.is_empty() && asset.original_url != asset.url {
                match transfer_file(
                    client,
                    store,
                    work,
                    asset,
                    index,
                    completed,
                    notify,
                    &asset.original_url,
                    true,
                )
                .await
                {
                    Ok(file) => {
                        source = "high_resolution";
                        file
                    }
                    Err(error) => {
                        if *work.control.borrow() != 0 {
                            return Err(error);
                        }
                        fallback_reason = error;
                        source = "web_fallback";
                        store.lock().unwrap().state(
                            &work.id,
                            "running",
                            "高清候选不可用，正在回退网页图片",
                        );
                        notify();
                        transfer_file(
                            client, store, work, asset, index, completed, notify, &asset.url, false,
                        )
                        .await?
                    }
                }
            } else {
                transfer_file(
                    client, store, work, asset, index, completed, notify, &asset.url, false,
                )
                .await?
            };
        let sequence = if asset.sequence > 0 {
            asset.sequence
        } else {
            index + 1
        };
        let stem = format!(
            "{:03}{}_{}",
            sequence,
            if asset.live { "_live" } else { "" },
            safe_name(&work.note.title)
        );
        let mut destination = work.folder.join(format!("{stem}.{}", transfer.ext));
        if destination.exists() {
            destination = work.folder.join(format!(
                "{stem}_{}.{}",
                work.id.rsplit('-').next().unwrap_or("new"),
                transfer.ext
            ));
        }
        tokio::fs::rename(&transfer.part, &destination)
            .await
            .map_err(|_| "保存最终文件失败，请检查文件占用")?;
        // After a source succeeds, discard only this item's unused candidate partial.
        let unused_part = if source == "high_resolution" {
            Some(work.folder.join(format!("{:03}.part", index + 1)))
        } else if source == "web_fallback" {
            Some(work.folder.join(format!("{:03}-hq.part", index + 1)))
        } else {
            None
        };
        if let Some(path) = unused_part {
            let _ = tokio::fs::remove_file(path).await;
        }
        report.retain(|r| r["index"].as_u64() != Some(index as u64));
        report.push(serde_json::json!({"index":index,"sequence":sequence,"live":asset.live,"video":asset.video,
            "filename":destination.file_name().unwrap_or_default().to_string_lossy(),"bytes":transfer.bytes,
            "width":transfer.width,"height":transfer.height,"format":transfer.ext,"source":source,"fallback_reason":fallback_reason}));
        tokio::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&report).map_err(|_| "下载报告编码失败")?,
        )
        .await
        .map_err(|_| "下载报告保存失败")?;
        store
            .lock()
            .unwrap()
            .file_done(&work.id, index, &destination, transfer.bytes);
        completed += transfer.bytes;
        let detail = if asset.video {
            if asset.live {
                "LIVE 动态片段".to_owned()
            } else {
                "视频".to_owned()
            }
        } else {
            format!(
                "{} · {}×{}",
                if source == "high_resolution" {
                    "高清源"
                } else if source == "web_fallback" {
                    "已回退网页图片"
                } else {
                    "网页资源，未验证高清源"
                },
                transfer.width,
                transfer.height
            )
        };
        store.lock().unwrap().state(
            &work.id,
            "running",
            &format!(
                "第 {}/{} 项 · {} · {} · {:.1} MB",
                index + 1,
                work.note.assets.len(),
                detail,
                transfer.ext,
                transfer.bytes as f64 / 1048576.0
            ),
        );
        store
            .lock()
            .unwrap()
            .progress(&work.id, completed, completed);
        notify();
    }
    let fallback = report
        .iter()
        .filter(|r| r["source"] == "web_fallback")
        .count();
    let web = report.iter().filter(|r| r["source"] == "web").count();
    let live = report.iter().filter(|r| r["live"] == true).count();
    let mut summary = format!(
        "完成 {} 项 · {:.1} MB",
        work.note.assets.len(),
        completed as f64 / 1048576.0
    );
    if live > 0 {
        summary.push_str(&format!(" · {live} 个 LIVE 片段"));
    }
    if fallback > 0 {
        summary.push_str(&format!(" · {fallback} 张已回退网页图片"));
    }
    if web > 0 {
        summary.push_str(&format!(" · {web} 张未验证高清源"));
    }
    if let Some(image) = report.iter().find(|r| r["width"].as_u64().unwrap_or(0) > 0) {
        summary.push_str(&format!(" · {}×{}", image["width"], image["height"]));
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn server(data: Vec<u8>, stall: bool) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request);
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",data.len()).unwrap();
            if stall {
                stream.write_all(&data[..8]).unwrap();
                stream.flush().unwrap();
                std::thread::sleep(Duration::from_millis(250));
                let _ = stream.write_all(&data[8..]);
            } else {
                stream.write_all(&data).unwrap();
            }
        });
        (url, thread)
    }
    #[test]
    fn smaller_hd_falls_back_and_live_pairs_share_sequence() {
        let root =
            std::env::temp_dir().join(format!("stilldock-quality-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let png = |w: u32, h: u32| {
            let mut data = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
            data.extend(w.to_be_bytes());
            data.extend(h.to_be_bytes());
            data.extend([8, 6, 0, 0, 0]);
            data.extend([0; 48]);
            data
        };
        let (hq, hq_thread) = server(png(1, 1), false);
        let (web, web_thread) = server(png(2, 3), false);
        let (live, live_thread) = server(
            b"\x00\x00\x00\x18ftypisom\x00\x00\x00\x00isommp42".to_vec(),
            false,
        );
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            title: "pair".into(),
            assets: vec![
                Asset {
                    url: web,
                    original_url: hq,
                    width: 2,
                    height: 3,
                    sequence: 2,
                    ..Default::default()
                },
                Asset {
                    url: live,
                    video: true,
                    live: true,
                    sequence: 2,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let id = store.lock().unwrap().add(&note, &root).unwrap();
        let folder = store.lock().unwrap().payload(&id).unwrap().1;
        let (_signal, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note,
            folder: folder.clone(),
            control: rx,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let summary = rt
            .block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap();
        hq_thread.join().unwrap();
        web_thread.join().unwrap();
        live_thread.join().unwrap();
        assert!(summary.contains("1 张已回退"));
        assert!(summary.contains("1 个 LIVE"));
        assert!(folder.join("002_pair.png").is_file());
        assert!(folder.join("002_live_pair.mp4").is_file());
        let report: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(folder.join("download-report.json")).unwrap())
                .unwrap();
        assert_eq!(report[0]["source"], "web_fallback");
        assert_eq!(report[0]["width"], 2);
        assert_eq!(report[0]["height"], 3);
        assert!(report[0]["fallback_reason"]
            .as_str()
            .unwrap()
            .contains("分辨率"));
        drop(work);
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pause_flush_and_server_ignoring_range() {
        let root = std::env::temp_dir().join(format!("xhs-transfer-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let data = [b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x02\x00\x00\x00\x03\x08\x06\x00\x00\x00".as_slice(), &[0u8; 48]].concat();
        let (url, thread) = server(data.clone(), true);
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            title: "file".into(),
            assets: vec![crate::model::Asset {
                url,
                video: false,
                preview: String::new(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let id = store.lock().unwrap().add(&note, &root).unwrap();
        let folder = store.lock().unwrap().payload(&id).unwrap().1;
        let (signal, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note,
            folder: folder.clone(),
            control: rx,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = reqwest::Client::new();
        rt.block_on(async {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let _ = signal.send(1);
            });
            assert!(run(&client, &store, &mut work, &|| {}).await.is_err());
        });
        thread.join().unwrap();
        assert_eq!(std::fs::read(folder.join("001.part")).unwrap(), data[..8]);
        let (url, thread) = server(data.clone(), false);
        work.note.assets[0].url = url;
        let (_tx, rx) = watch::channel(0);
        work.control = rx;
        rt.block_on(run(&client, &store, &mut work, &|| {}))
            .unwrap();
        thread.join().unwrap();
        let (path, size) = store.lock().unwrap().file(&id, 0).unwrap();
        assert_eq!(size, data.len() as u64);
        assert_eq!(std::fs::read(path).unwrap(), data);
        drop(work);
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }
}
