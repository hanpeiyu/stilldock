use crate::{
    model::{file_extension, media_allowed, safe_name, Asset, Note},
    store::{DownloadOptions, SavedFile, Store},
};
use sha2::{Digest, Sha256};
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
    pub options: DownloadOptions,
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
    sha256: String,
}

#[derive(Debug)]
struct TransferFailure {
    message: String,
    retryable: bool,
}
impl TransferFailure {
    fn retry(message: &str) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}
impl From<&str> for TransferFailure {
    fn from(message: &str) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
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
    for attempt in 0..3 {
        if *work.control.borrow() != 0 {
            return Err("任务已停止".into());
        }
        match transfer_once(
            client,
            store,
            work,
            asset,
            index,
            completed,
            notify,
            url,
            high_quality,
        )
        .await
        {
            Ok(transfer) => return Ok(transfer),
            Err(error) if error.retryable && attempt < 2 => {
                store.lock().unwrap().state(
                    &work.id,
                    "running",
                    &format!("网络异常，正在第 {}/2 次重试", attempt + 1),
                );
                notify();
                tokio::select! {biased;_=work.control.changed()=>return Err("任务已停止".into()),_=tokio::time::sleep(Duration::from_millis(if attempt==0{500}else{1500}))=>{}}
            }
            Err(error) => return Err(error.message),
        }
    }
    unreachable!()
}

async fn transfer_once(
    client: &reqwest::Client,
    store: &SharedStore,
    work: &mut Work,
    asset: &Asset,
    index: usize,
    completed: u64,
    notify: &impl Fn(),
    url: &str,
    high_quality: bool,
) -> Result<Transfer, TransferFailure> {
    let part = work.options.temp_folder.join(format!(
        "{:03}{}.part",
        index + 1,
        if high_quality { "-hq" } else { "" }
    ));
    let mut offset = tokio::fs::metadata(&part)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let remembered = store
        .lock()
        .unwrap()
        .partial_source(&work.id, index, high_quality);
    if remembered.as_ref().is_some_and(|(source, _)| source != url) {
        offset = 0;
    }
    let mut response = None;
    for attempt in 0..2 {
        let mut request = client
            .get(url)
            .header("Referer", "https://www.xiaohongshu.com/");
        if offset > 0 {
            request = request.header("Range", format!("bytes={offset}-"));
            if let Some((source, etag)) = &remembered {
                if source == url && !etag.is_empty() && !etag.starts_with("W/") {
                    request = request.header("If-Range", etag);
                }
            }
        }
        let r = tokio::select! { biased;
            _=work.control.changed()=>return Err("任务已停止".into()),
            r=request.send()=>r.map_err(|_|TransferFailure::retry("网络连接失败或超时，请重试"))?,
        };
        if r.status().as_u16() == 416 && attempt == 0 {
            offset = 0;
            continue;
        }
        if offset > 0 && r.status().as_u16() == 206 && attempt == 0 {
            if let Some((source, etag)) = &remembered {
                if source == url
                    && !etag.is_empty()
                    && r.headers()
                        .get("etag")
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|new| new != etag)
                {
                    offset = 0;
                    continue;
                }
            }
        }
        response = Some(r);
        break;
    }
    let mut response = response.ok_or("服务器未返回文件")?;
    let code = response.status().as_u16();
    if !response.status().is_success() {
        let message = match code {
            401 | 403 => "媒体地址不可访问，请重新打开笔记再下载",
            404 | 410 => "媒体链接已失效，请重新打开笔记",
            429 => "请求过于频繁，请稍后再试",
            _ => "媒体服务器返回错误，请重试",
        };
        return Err(TransferFailure {
            message: message.into(),
            retryable: matches!(code, 500 | 502 | 503 | 504),
        });
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
    let mut etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if offset > 0 && etag.is_empty() {
        if let Some((source, previous)) = &remembered {
            if source == url {
                etag = previous.clone();
            }
        }
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .append(offset > 0)
        .open(&part)
        .await
        .map_err(|_| "无法写入临时文件")?;
    store
        .lock()
        .unwrap()
        .set_partial_source(&work.id, index, high_quality, url, &etag)
        .map_err(|message| TransferFailure {
            message,
            retryable: false,
        })?;
    let mut hasher = Sha256::new();
    if offset > 0 {
        let mut prefix = tokio::fs::File::open(&part)
            .await
            .map_err(|_| "无法读取续传文件")?;
        let mut remaining = offset;
        let mut buffer = [0u8; 65536];
        while remaining > 0 {
            if *work.control.borrow() != 0 {
                return Err("任务已停止".into());
            }
            let limit = (remaining as usize).min(buffer.len());
            let n = prefix
                .read(&mut buffer[..limit])
                .await
                .map_err(|_| "无法读取续传文件")?;
            if n == 0 {
                return Err("续传文件长度变化".into());
            }
            hasher.update(&buffer[..n]);
            remaining -= n as u64;
        }
    }
    let mut bytes = offset;
    let mut tick = Instant::now();
    loop {
        let chunk = tokio::select! {biased;
            _=work.control.changed()=>{file.flush().await.map_err(|_|"保存下载进度失败")?;return Err("任务已停止".into())},
            c=response.chunk()=>match c{Ok(chunk)=>chunk,Err(_)=>{file.flush().await.map_err(|_|"保存下载进度失败")?;return Err(TransferFailure::retry("下载连接中断，临时文件已保留，可重试"));}},
        };
        let Some(chunk) = chunk else { break };
        file.write_all(&chunk)
            .await
            .map_err(|_| "写入失败，请检查磁盘空间与目录权限")?;
        hasher.update(&chunk);
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
        if asset.width > 0
            && asset.height > 0
            && (size.width as u64 * size.height as u64) < (asset.width as u64 * asset.height as u64)
        {
            return Err("图片分辨率不符合要求，请重新打开笔记后重试".into());
        }
        (size.width, size.height)
    };
    Ok(Transfer {
        part,
        bytes,
        ext,
        width,
        height,
        sha256: format!("{:x}", hasher.finalize()),
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
    tokio::fs::create_dir_all(&work.options.temp_folder)
        .await
        .map_err(|_| "无法创建下载缓存目录")?;
    if work.options.layout != "legacy" {
        if let Some(parent) = work.options.temp_folder.parent() {
            crate::file_ops::hide_cache(parent);
        }
    }
    let mut report = store.lock().unwrap().saved_files(&work.id)?;
    let mut completed = 0;
    let mut reused = 0;
    let archive = if work.options.force {
        None
    } else {
        let loaded = { crate::library::Archive::load(&store.lock().unwrap(), &work.note.id)? };
        Some(loaded.indexed())
    };
    for (index, asset) in work.note.assets.clone().iter().enumerate() {
        if *work.control.borrow() != 0 {
            return Err("任务已停止".into());
        }
        let saved = { store.lock().unwrap().file(&work.id, index) };
        if let Some((path, size)) = saved {
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => Some(m),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => return Err("原文件暂时无法访问，请检查磁盘或目录权限后重试".into()),
            };
            if metadata.is_some_and(|m| m.is_file() && m.len() == size && size > 0) {
                if !report
                    .iter()
                    .any(|record| record.index == index && !record.format.is_empty())
                {
                    let (width, height) = if asset.video {
                        (0, 0)
                    } else {
                        let dimensions =
                            imagesize::size(&path).map_err(|_| "已保存图片无法读取，请重新下载")?;
                        (dimensions.width, dimensions.height)
                    };
                    let record = SavedFile {
                        index,
                        sequence: if asset.sequence > 0 {
                            asset.sequence
                        } else {
                            index + 1
                        },
                        live: asset.live,
                        video: asset.video,
                        filename: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        bytes: size,
                        width,
                        height,
                        format: path
                            .extension()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        source: "legacy".into(),
                        sha256: String::new(),
                    };
                    store
                        .lock()
                        .unwrap()
                        .file_done(&work.id, index, &path, &record)?;
                    report.retain(|r| r.index != index);
                    report.push(record);
                }
                completed += size;
                continue;
            }
        }
        if let Some(existing) = archive.as_ref().and_then(|a| a.reusable(asset)) {
            let mut record = existing.record;
            record.index = index;
            record.sequence = if asset.sequence > 0 {
                asset.sequence
            } else {
                index + 1
            };
            completed += record.bytes;
            store
                .lock()
                .unwrap()
                .file_done(&work.id, index, &existing.path, &record)?;
            report.retain(|r| r.index != index);
            report.push(record);
            reused += 1;
            store
                .lock()
                .unwrap()
                .progress(&work.id, completed, completed);
            notify();
            continue;
        }
        store.lock().unwrap().state(
            &work.id,
            "running",
            &format!("正在处理第 {}/{} 项", index + 1, work.note.assets.len()),
        );
        notify();
        let has_original =
            !asset.video && !asset.original_url.is_empty() && asset.original_url != asset.url;
        let source = if asset.video {
            "video"
        } else if has_original {
            "high_resolution"
        } else {
            "web"
        };
        let transfer = transfer_file(
            client,
            store,
            work,
            asset,
            index,
            completed,
            notify,
            if has_original {
                &asset.original_url
            } else {
                &asset.url
            },
            has_original,
        )
        .await?;
        let sequence = if asset.sequence > 0 {
            asset.sequence
        } else {
            index + 1
        };
        let stem = if work.options.layout == "flat" {
            format!(
                "{}_{}_{:03}{}",
                safe_name(&work.note.title),
                work.note.id,
                sequence,
                if asset.live { "_live" } else { "" }
            )
        } else {
            format!(
                "{:03}{}_{}",
                sequence,
                if asset.live { "_live" } else { "" },
                safe_name(&work.note.title)
            )
        };
        let destination = crate::file_ops::publish(
            &transfer.part,
            &work.folder,
            &stem,
            transfer.ext,
            work.id.rsplit('-').next().unwrap_or("new"),
        )
        .map_err(|_| "保存最终文件失败，请检查权限或文件占用")?;
        // After a source succeeds, discard only this item's unused candidate partial.
        let unused_part = if source == "high_resolution" {
            Some(
                work.options
                    .temp_folder
                    .join(format!("{:03}.part", index + 1)),
            )
        } else {
            None
        };
        if let Some(path) = unused_part {
            let _ = tokio::fs::remove_file(path).await;
        }
        let record = SavedFile {
            index,
            sequence,
            live: asset.live,
            video: asset.video,
            filename: destination
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            bytes: transfer.bytes,
            width: transfer.width,
            height: transfer.height,
            format: transfer.ext.into(),
            source: source.into(),
            sha256: transfer.sha256,
        };
        store
            .lock()
            .unwrap()
            .file_done(&work.id, index, &destination, &record)?;
        report.retain(|r| r.index != index);
        report.push(record);
        completed += transfer.bytes;
        store.lock().unwrap().state(
            &work.id,
            "running",
            &format!("已保存第 {}/{} 项", index + 1, work.note.assets.len()),
        );
        store
            .lock()
            .unwrap()
            .progress(&work.id, completed, completed);
        notify();
    }
    report.sort_by_key(|record| record.index);
    let export_failed = if work.options.export_json {
        export_metadata(work, &report).await.is_err()
    } else {
        false
    };
    if work.options.layout != "legacy" {
        let _ = tokio::fs::remove_dir(&work.options.temp_folder).await;
        if let Some(parent) = work.options.temp_folder.parent() {
            let _ = tokio::fs::remove_dir(parent).await;
        }
    }
    let live = report.iter().filter(|r| r.live).count();
    let mut summary = format!(
        "完成 {} 项 · {:.1} MB",
        work.note.assets.len(),
        completed as f64 / 1048576.0
    );
    if reused > 0 {
        summary.push_str(&format!(" · 跳过已有 {reused} 项（保留原位置）"));
    }
    if live > 0 {
        summary.push_str(&format!(" · {live} 个 LIVE 片段"));
    }
    if let Some(image) = report.iter().find(|r| r.width > 0) {
        summary.push_str(&format!(" · {}×{}", image.width, image.height));
    }
    if export_failed {
        summary.push_str(" · JSON 说明导出失败");
    }
    Ok(summary)
}

async fn export_metadata(work: &Work, report: &[SavedFile]) -> Result<(), String> {
    let metadata =
        serde_json::to_vec_pretty(&serde_json::json!({"note":&work.note,"files":report}))
            .map_err(|_| "说明文件编码失败")?;
    tokio::fs::write(
        work.folder.join(format!("{}-metadata.json", work.id)),
        metadata,
    )
    .await
    .map_err(|_| "说明文件导出失败")?;
    Ok(())
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
    fn tiny_png() -> Vec<u8> {
        [b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x02\x00\x00\x00\x03\x08\x06\x00\x00\x00".as_slice(),&[0;48]].concat()
    }
    #[test]
    fn duplicate_is_reused_across_jobs_unless_force_is_set_and_hash_is_recorded() {
        let root = std::env::temp_dir().join(format!("stilldock-dedupe-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = reqwest::Client::new();
        let (url, thread) = server(tiny_png(), false);
        let mut note = Note {
            id: "abcdef0123456789abcdef05".into(),
            assets: vec![Asset {
                url: "https://sns-img-bd.xhscdn.com/dedup-image".into(),
                original_url: url,
                sequence: 3,
                ..Default::default()
            }],
            ..Default::default()
        };
        let run_job = |force: bool, folder: &std::path::Path, note: &Note| {
            let id = store
                .lock()
                .unwrap()
                .add_with_policy(note, folder, force)
                .unwrap();
            let (_, folder, _) = store.lock().unwrap().payload(&id).unwrap();
            let options = store.lock().unwrap().download_options(&id).unwrap();
            let (_signal, rx) = watch::channel(0);
            let mut work = Work {
                id: id.clone(),
                note: note.clone(),
                folder,
                options,
                control: rx,
            };
            let message = rt
                .block_on(run(&client, &store, &mut work, &|| {}))
                .unwrap();
            store.lock().unwrap().state(&id, "done", &message);
            (id, message)
        };
        let (first, _) = run_job(false, &root, &note);
        thread.join().unwrap();
        let original = store.lock().unwrap().file(&first, 0).unwrap().0;
        let mut wrong = note.assets[0].clone();
        wrong.url = "https://sns-img-bd.xhscdn.com/different-image".into();
        let archive =
            { crate::library::Archive::load(&store.lock().unwrap(), &note.id).unwrap() }.indexed();
        assert!(archive.reusable(&wrong).is_none());
        let hash = store.lock().unwrap().saved_files(&first).unwrap()[0]
            .sha256
            .clone();
        assert_eq!(hash, format!("{:x}", Sha256::digest(tiny_png())));
        let (second, message) = run_job(false, &root.join("new-root"), &note);
        assert!(message.contains("跳过已有 1 项"));
        assert_eq!(store.lock().unwrap().file(&second, 0).unwrap().0, original);
        let (url, thread) = server(tiny_png(), false);
        note.assets[0].original_url = url;
        let (forced, _) = run_job(true, &root, &note);
        thread.join().unwrap();
        let other = store.lock().unwrap().file(&forced, 0).unwrap().0;
        assert_ne!(other, original);
        assert_eq!(std::fs::read(&original).unwrap(), tiny_png());
        let snapshot = crate::library::inspect(&store, &note);
        assert_eq!(snapshot.assets[0].state, "present");
        std::fs::remove_file(&original).unwrap();
        assert_eq!(
            crate::library::inspect(&store, &note).assets[0].state,
            "present"
        );
        std::fs::remove_file(other).unwrap();
        assert_eq!(
            crate::library::inspect(&store, &note).assets[0].state,
            "missing"
        );
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_body_retries_with_range_and_hashes_the_complete_file() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image", listener.local_addr().unwrap());
        let data = tiny_png();
        let expected = data.clone();
        let split = data.len() / 2;
        let thread = std::thread::spawn(move || {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let n = stream.read(&mut request).unwrap();
                let header = String::from_utf8_lossy(&request[..n]);
                if attempt == 0 {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nETag: version-one\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        data.len()
                    )
                    .unwrap();
                    stream.write_all(&data[..split]).unwrap();
                } else {
                    assert!(header
                        .to_ascii_lowercase()
                        .contains(&format!("range: bytes={split}-")));
                    assert!(header
                        .to_ascii_lowercase()
                        .contains("if-range: version-one"));
                    write!(stream,"HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",data.len()-split,split,data.len()-1,data.len()).unwrap();
                    stream.write_all(&data[split..]).unwrap();
                }
            }
        });
        let root = std::env::temp_dir().join(format!("stilldock-retry-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let note = Note {
            id: "abcdef0123456789abcdef06".into(),
            assets: vec![Asset {
                url,
                ..Default::default()
            }],
            ..Default::default()
        };
        let id = store.lock().unwrap().add(&note, &root).unwrap();
        let (_, folder, _) = store.lock().unwrap().payload(&id).unwrap();
        let options = store.lock().unwrap().download_options(&id).unwrap();
        let (_signal, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note,
            folder,
            options,
            control: rx,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap();
        thread.join().unwrap();
        let record = store.lock().unwrap().saved_files(&id).unwrap().remove(0);
        assert_eq!(record.sha256, format!("{:x}", Sha256::digest(&expected)));
        assert_eq!(
            std::fs::read(store.lock().unwrap().file(&id, 0).unwrap().0).unwrap(),
            expected
        );
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn server_retries_are_bounded_and_pause_interrupts_backoff_without_retrying_429() {
        fn response(status: u16) -> (String, std::thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/image", listener.local_addr().unwrap());
            let thread = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf);
                write!(
                    stream,
                    "HTTP/1.1 {status} Failure\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            });
            (url, thread)
        }
        let root = std::env::temp_dir().join(format!("stilldock-backoff-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for status in [429u16, 503] {
            let (url, thread) = response(status);
            let note = Note {
                id: format!("abcdef0123456789abcd{status:04}"),
                assets: vec![Asset {
                    url,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let id = store.lock().unwrap().add(&note, &root).unwrap();
            let (_, folder, _) = store.lock().unwrap().payload(&id).unwrap();
            let options = store.lock().unwrap().download_options(&id).unwrap();
            let (signal, rx) = watch::channel(0);
            let mut work = Work {
                id: id.clone(),
                note,
                folder,
                options,
                control: rx,
            };
            let result = rt.block_on(async {
                if status == 503 {
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(75)).await;
                        let _ = signal.send(1);
                    });
                }
                run(&reqwest::Client::new(), &store, &mut work, &|| {}).await
            });
            thread.join().unwrap();
            let error = result.unwrap_err();
            if status == 429 {
                assert!(error.contains("请求过于频繁"));
            } else {
                assert_eq!(error, "任务已停止");
            }
        }
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn changed_source_never_appends_to_the_previous_source_partial() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/new", listener.local_addr().unwrap());
        let data = tiny_png();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 4096];
            let n = stream.read(&mut buffer).unwrap();
            assert!(!String::from_utf8_lossy(&buffer[..n])
                .to_ascii_lowercase()
                .contains("range:"));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                data.len()
            )
            .unwrap();
            stream.write_all(&data).unwrap();
        });
        let root =
            std::env::temp_dir().join(format!("stilldock-source-part-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let note = Note {
            id: "abcdef0123456789abcdef07".into(),
            assets: vec![Asset {
                url,
                ..Default::default()
            }],
            ..Default::default()
        };
        let id = store.lock().unwrap().add(&note, &root).unwrap();
        let (_, folder, _) = store.lock().unwrap().payload(&id).unwrap();
        let options = store.lock().unwrap().download_options(&id).unwrap();
        std::fs::create_dir_all(&options.temp_folder).unwrap();
        std::fs::write(
            options.temp_folder.join("001.part"),
            b"old bytes must never be appended",
        )
        .unwrap();
        store
            .lock()
            .unwrap()
            .set_partial_source(
                &id,
                0,
                false,
                "https://sns-img-bd.xhscdn.com/previous",
                "old-etag",
            )
            .unwrap();
        let (_signal, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note,
            folder,
            options,
            control: rx,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap();
        thread.join().unwrap();
        assert_eq!(
            std::fs::read(store.lock().unwrap().file(&id, 0).unwrap().0).unwrap(),
            tiny_png()
        );
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn flat_output_uses_sqlite_and_exports_only_when_enabled() {
        let root = std::env::temp_dir().join(format!("stilldock-flat-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let folder = root.join("media");
        std::fs::create_dir_all(&folder).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        store
            .lock()
            .unwrap()
            .set_setting("output_layout", "flat")
            .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = reqwest::Client::new();
        let (url, thread) = server(tiny_png(), false);
        let mut note = Note {
            id: "abcdef0123456789abcdef01".into(),
            title: "same".into(),
            assets: vec![Asset {
                url,
                sequence: 2,
                ..Default::default()
            }],
            ..Default::default()
        };
        let id = store.lock().unwrap().add(&note, &folder).unwrap();
        let options = store.lock().unwrap().download_options(&id).unwrap();
        let cache = options.temp_folder.clone();
        let existing = folder.join(format!("same_{}_002.png", note.id));
        std::fs::write(&existing, b"do not overwrite").unwrap();
        let (_tx, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note: note.clone(),
            folder: folder.clone(),
            options,
            control: rx,
        };
        runtime
            .block_on(run(&client, &store, &mut work, &|| {}))
            .unwrap();
        thread.join().unwrap();
        assert_eq!(std::fs::read(&existing).unwrap(), b"do not overwrite");
        assert!(!cache.exists());
        assert!(!folder.join(".stilldock-cache").exists());
        let records = store.lock().unwrap().saved_files(&id).unwrap();
        assert_eq!(records[0].width, 2);
        assert_eq!(records[0].height, 3);
        assert_eq!(records[0].sequence, 2);
        assert_ne!(
            records[0].filename,
            existing.file_name().unwrap().to_string_lossy()
        );
        assert!(std::fs::read_dir(&folder).unwrap().all(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|e| e != "json")));
        store.lock().unwrap().state(&id, "done", "");
        store
            .lock()
            .unwrap()
            .set_setting("export_json", "true")
            .unwrap();
        let (url, thread) = server(tiny_png(), false);
        note.id = "abcdef0123456789abcdef02".into();
        note.assets[0].url = url;
        let next = store.lock().unwrap().add(&note, &folder).unwrap();
        let options = store.lock().unwrap().download_options(&next).unwrap();
        let (_tx, rx) = watch::channel(0);
        let mut work = Work {
            id: next.clone(),
            note,
            folder: folder.clone(),
            options,
            control: rx,
        };
        runtime
            .block_on(run(&client, &store, &mut work, &|| {}))
            .unwrap();
        thread.join().unwrap();
        let json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join(format!("{next}-metadata.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(json["files"][0]["width"], 2);
        assert_eq!(json["note"]["id"], "abcdef0123456789abcdef02");
        assert!(std::fs::read_dir(&folder)
            .unwrap()
            .all(|entry| !entry.unwrap().path().is_dir()));
        drop(work);
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn repair_only_downloads_missing_file_and_updates_existing_history() {
        let root = std::env::temp_dir().join(format!("stilldock-repair-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&root.join("jobs.db")).unwrap()));
        let (url, thread) = server(tiny_png(), false);
        let note = Note {
            id: "abcdef0123456789abcdef03".into(),
            title: "repair".into(),
            assets: vec![
                Asset {
                    url: "https://sns-img-bd.xhscdn.com/must-not-request".into(),
                    sequence: 1,
                    ..Default::default()
                },
                Asset {
                    url,
                    sequence: 2,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let id = store.lock().unwrap().add(&note, &root).unwrap();
        let (_, folder, _) = store.lock().unwrap().payload(&id).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        let data = tiny_png();
        let good = folder.join("keep.png");
        let gone = folder.join("gone.png");
        std::fs::write(&good, &data).unwrap();
        std::fs::write(&gone, &data).unwrap();
        for (index, path) in [&good, &gone].into_iter().enumerate() {
            store
                .lock()
                .unwrap()
                .file_done(
                    &id,
                    index,
                    path,
                    &SavedFile {
                        index,
                        sequence: index + 1,
                        live: false,
                        video: false,
                        filename: path.file_name().unwrap().to_string_lossy().into(),
                        bytes: data.len() as u64,
                        width: 2,
                        height: 3,
                        format: "png".into(),
                        source: "high_resolution".into(),
                        sha256: String::new(),
                    },
                )
                .unwrap();
        }
        store.lock().unwrap().state(&id, "done", "下载完成");
        std::fs::remove_file(&gone).unwrap();
        let target = store
            .lock()
            .unwrap()
            .local_targets(&note.id)
            .unwrap()
            .remove(0);
        let status = crate::local_files::check(&target);
        assert_eq!((status.present, status.missing), (1, 1));
        let options = store.lock().unwrap().download_options(&id).unwrap();
        let (_tx, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note,
            folder: folder.clone(),
            options,
            control: rx,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap();
        thread.join().unwrap();
        store.lock().unwrap().state(&id, "done", "下载完成");
        let targets = store.lock().unwrap().local_targets("").unwrap();
        assert_eq!(targets.len(), 1);
        let status = crate::local_files::check(&targets[0]);
        assert_eq!((status.present, status.missing, status.changed), (2, 0, 0));
        assert_eq!(store.lock().unwrap().file(&id, 0).unwrap().0, good);
        assert_eq!(std::fs::read(&good).unwrap(), data);
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 2);
        let relocated = root.join("organized");
        std::fs::create_dir_all(&relocated).unwrap();
        let target = &targets[0];
        for (old, _) in &target.files {
            std::fs::rename(old, relocated.join(old.file_name().unwrap())).unwrap();
        }
        let paths = crate::local_files::relocated(target, &relocated).unwrap();
        std::fs::write(&paths[1], b"wrong size").unwrap();
        assert!(crate::local_files::relocated(target, &relocated).is_err());
        assert_eq!(store.lock().unwrap().payload(&id).unwrap().1, folder);
        std::fs::write(&paths[1], tiny_png()).unwrap();
        store.lock().unwrap().state(&id, "pending", "");
        assert!(store
            .lock()
            .unwrap()
            .relink(target, &relocated, &paths)
            .is_err());
        assert_eq!(store.lock().unwrap().file(&id, 0).unwrap().0, good);
        store.lock().unwrap().state(&id, "done", "");
        store
            .lock()
            .unwrap()
            .relink(target, &relocated, &paths)
            .unwrap();
        assert_eq!(store.lock().unwrap().payload(&id).unwrap().1, relocated);
        assert_eq!(
            store
                .lock()
                .unwrap()
                .download_options(&id)
                .unwrap()
                .temp_folder,
            relocated.join(".stilldock-cache").join(&id)
        );
        let target = store.lock().unwrap().local_targets("").unwrap().remove(0);
        assert_eq!(crate::local_files::check(&target).present, 2);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), data);
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn legacy_resume_rebuilds_file_metadata_without_reading_or_overwriting_json() {
        let root =
            std::env::temp_dir().join(format!("stilldock-legacy-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let database = root.join("jobs.db");
        let folder = root.join("old-task");
        std::fs::create_dir_all(&folder).unwrap();
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            title: "old".into(),
            assets: vec![Asset {
                url: "https://sns-img-bd.xhscdn.com/no-network-needed".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let id;
        {
            let store = Store::open(&database).unwrap();
            id = store.add(&note, &root).unwrap();
        }
        let image = folder.join("old.png");
        let data = tiny_png();
        std::fs::write(&image, &data).unwrap();
        std::fs::write(folder.join("note.json"), b"keep old note").unwrap();
        std::fs::write(folder.join("download-report.json"), b"not even valid JSON").unwrap();
        {
            let db = rusqlite::Connection::open(&database).unwrap();
            db.execute(
                "UPDATE jobs SET layout='legacy',temp_folder='',folder=? WHERE id=?",
                rusqlite::params![folder.to_string_lossy(), id],
            )
            .unwrap();
            db.execute(
                "INSERT INTO files(job_id,idx,path,size) VALUES(?,0,?,?)",
                rusqlite::params![id, image.to_string_lossy(), data.len() as i64],
            )
            .unwrap();
        }
        let store = Arc::new(Mutex::new(Store::open(&database).unwrap()));
        let options = store.lock().unwrap().download_options(&id).unwrap();
        let (_tx, rx) = watch::channel(0);
        let mut work = Work {
            id: id.clone(),
            note,
            folder: folder.clone(),
            options,
            control: rx,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap();
        let records = store.lock().unwrap().saved_files(&id).unwrap();
        assert_eq!(records[0].width, 2);
        assert_eq!(records[0].height, 3);
        assert_eq!(records[0].source, "legacy");
        assert_eq!(
            std::fs::read(folder.join("note.json")).unwrap(),
            b"keep old note"
        );
        assert_eq!(
            std::fs::read(folder.join("download-report.json")).unwrap(),
            b"not even valid JSON"
        );
        assert_eq!(std::fs::read(image).unwrap(), data);
        drop(work);
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn smaller_hd_is_rejected_and_live_pairs_share_sequence() {
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
        let web = "http://127.0.0.1:1/must-not-fallback".to_owned();
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
            options: store.lock().unwrap().download_options(&id).unwrap(),
            control: rx,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = rt
            .block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap_err();
        hq_thread.join().unwrap();
        assert!(error.contains("分辨率"));
        assert!(!folder.join("002_pair.png").exists());
        let (valid_hq, valid_thread) = server(png(2, 3), false);
        work.note.assets[0].original_url = valid_hq;
        let summary = rt
            .block_on(run(&reqwest::Client::new(), &store, &mut work, &|| {}))
            .unwrap();
        valid_thread.join().unwrap();
        live_thread.join().unwrap();
        assert!(summary.contains("1 个 LIVE"));
        assert!(folder.join("002_pair.png").is_file());
        assert!(folder.join("002_live_pair.mp4").is_file());
        let report = store.lock().unwrap().saved_files(&id).unwrap();
        assert_eq!(report[0].source, "high_resolution");
        assert_eq!(report[0].width, 2);
        assert_eq!(report[0].height, 3);
        assert!(!folder.join("note.json").exists());
        assert!(!folder.join("download-report.json").exists());
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
            options: store.lock().unwrap().download_options(&id).unwrap(),
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
        assert_eq!(
            std::fs::read(work.options.temp_folder.join("001.part")).unwrap(),
            data[..8]
        );
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
