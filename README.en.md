<p align="center"><img src="assets/brand/stilldock-256.png" width="96" height="96" alt="StillDock icon"></p>

# StillDock · 原帧

A Windows tool focused on high-resolution, watermark-free Xiaohongshu material downloads. Search, browse, select images and download in one window, powered by Rust and embedded WebView2.

[中文](README.md) · [Downloads](https://github.com/hanpeiyu/stilldock/releases) · [Build guide](docs/BUILDING.md)

## Features

- Native Xiaohongshu search, history and sign-in.
- Focus mode with a centered native search field, optional AI filtering and collapsed comments.
- Collapsible download sidebar that opens when a search starts.
- On-demand album thumbnails, numbered selection, select/clear all and larger previews.
- Image/video downloads, a persistent queue, pause/resume/cancel, history and folder shortcuts.
- Rust + Tao + Wry/WebView2 + SQLite. No Python, Node.js, .NET or external Chrome required at runtime.

## High-resolution images and Live media

The downloader first tries a high-resolution JPEG candidate exposed/derived from the opened note's platform media URL, checks the saved dimensions, and reports failure instead of silently replacing a failed high-resolution source with a low-resolution preview. SQLite stores actual dimensions, format, size and source. JSON is off by default; optional export creates one `<job-id>-metadata.json` with both note and file information. Files are not re-encoded locally. Available sources may not be the original camera file, and embedded author watermarks are not removed.

Live images are identified in the selection grid. Still images are saved by default; an always-visible checkbox above the download button remembers its setting and also saves the corresponding MP4 using the same original image number. This is an image/video pair, not a guaranteed importable iPhone Live Photo package. An in-page Back to Search button and Escape key close the note, preferring native close to preserve the list and scroll position. Note information is collapsed by default to leave room for the image, with an on-demand information button.

Previously named Shiguang; renamed StillDock at v0.2.0. Existing profiles, task databases and installation paths are retained.

## Run

Download the Windows x64 installer or portable ZIP from Releases. The installer runs per user and downloads Microsoft WebView2 Runtime if needed. The portable ZIP requires an existing WebView2 Runtime. Supported target: 64-bit Windows 10/11. Sign in inside the application; everyday Chrome cookies are not imported.

This is an **unsigned early preview**. A separate Windows 10 machine and long-running stress tests have not been completed. Website compatibility can change; automated tests do not establish live-site compatibility.

## Page controls and task panel

The image grid is the primary selection/download surface. The right task panel keeps queue/history/settings, with duplicate controls folded under **备用下载与选图** and automatically expanded for native fallback. A layout-adapter failure does not block source data from reaching the fallback panel.

Escape closes a preview before returning to search; its repeat/release events are consumed to prevent native handlers closing another layer. Clicking outside the preview also returns to the grid. Right-button gestures are removed, preserving native carousel and context-menu behavior. Video posts keep their native player and gain an in-page download action. Recognized related-search cards open a plain keyword search without changing other search tabs or note links.

## Local data

Profiles, settings and SQLite tasks use `%LOCALAPPDATA%\XhsBrowserRust`. Downloads default to `%USERPROFILE%\Downloads\XHS\<note-id>` (or directly in the chosen directory). These names are retained for compatibility with earlier Rust previews. Portable means installation-free, not self-contained profile storage. Uninstalling preserves user data.

No application telemetry or local HTTP server is configured. Cookies are not exported to the task database. Stored note metadata and media URLs may contain access parameters; do not attach your data directory to public issues.

## Build

On Windows x64, install Rust stable MSVC, Visual Studio C++ Build Tools and Windows SDK. Node.js 22+ is used only for JavaScript tests.

```powershell
cargo fmt --check
cargo test --locked
node --test tests/site.test.cjs tests/panel.test.cjs tests/search.test.cjs tests/bootstrap.test.cjs tests/focus.test.cjs tests/navigation.test.cjs tests/materials.test.cjs tests/video.test.cjs tests/related-search.test.cjs
cargo build --release --locked
```

The output executable is `target/release/xhs-browser.exe`. See the [Chinese build/packaging guide](docs/BUILDING.md) for NSIS packaging and GitHub Actions artifacts.

## Limitations

Windows x64 only. No API/MCP/TUI, author scraping or automatic updater. Media is read from the currently opened, authorized note. Quality depends on the streams/URLs exposed by the website. Finished/cancelled history is limited to the latest 200 entries, with active jobs also retained. Cross-job image deduplication is not implemented. Expired media links may require reopening the note and creating a new task.

## Attribution and license

GPL-3.0-only; see [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md). Extracted from the Rust browser experiment in [hanpeiyu/XHS-Downloader](https://github.com/hanpeiyu/XHS-Downloader), with extraction and page-adaptation work based on [JoeanAmier/XHS-Downloader](https://github.com/JoeanAmier/XHS-Downloader) and its derivative workspace. This project is not affiliated with Xiaohongshu.

## Save layout and optional metadata export

Settings offers per-note subfolders or flat output. Per-note output has no extra job folder; flat filenames include the title, note ID and image sequence. Final files are published without overwriting existing names. Pause/resume caches remain isolated per job in a hidden `.stilldock-cache` and empty cache directories are removed after completion.

Save location, layout and JSON preference are frozen when a job is created. Existing jobs retain their paths and partial files. Old JSON files are not deleted or required for resume; legacy file dimensions can be rebuilt into SQLite. A combined metadata JSON is exported only when explicitly enabled.

## Local file availability

Download history and current file availability are separate. A bounded background worker checks recorded paths and byte sizes at startup, on opening a note, and on visiting history (up to 200 completed jobs). Missing or size-changed files can be repaired in the same job; intact files are skipped. Inaccessible storage is reported separately and never triggers automatic downloads.

After moving a complete task's files, use **关联新目录** in history. All original filenames and sizes must match in the selected folder before paths are updated atomically. No files are moved or copied. This is not a content hash check; partial moves, renames and same-size content modifications are not detected/relinked automatically.

## Material grid

Image posts open in an adaptive card grid with shared sidebar selection, select/invert/clear, one-image preview and selected downloads. LIVE uses the existing preference and original image numbering. Escape closes the preview first, then returns through native note closing. Switch to the carousel per post or disable the default grid in settings; video and unrecognized posts retain native rendering.

Cards are virtualized by visible rows, thumbnails load near the viewport, and preview image references/observers are released on leaving. Shadow DOM isolates website CSS. Native detail interactions and video playback are suspended while covered, but the underlying page DOM remains available for returning to the same search position. This does not guarantee immediate WebView2 process memory reclamation.

## Local states and reliable downloads

Cards show availability/queue states, can filter needed items, select missing resources, and open existing file locations. Within the same note, an existing matching media file is reused in its original location by default. Opt into a new copy using the force-redownload checkbox. Resume always preserves already complete files in the same job.

Connection/body failures and HTTP 500/502/503/504 retry at most twice with cancellable 0.5s/1.5s backoff. Permission, expiration, rate-limit, disk and format errors do not loop. Reopening a note refreshes matching signed sources for paused/failed jobs while retaining selected original indices and LIVE pairs; resume remains explicit. New downloads record full SHA-256, but cross-post deduplication and automatic deletion are not implemented. Lookup is bounded to 200 same-note jobs / 4000 file records.
