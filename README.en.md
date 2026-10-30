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

The downloader first tries a high-resolution JPEG candidate exposed/derived from the opened note's platform media URL, checks the saved dimensions, and falls back explicitly to the webpage image if needed. `download-report.json` records actual dimensions, format, size, source and fallback reason. Files are not re-encoded locally. Available sources may not be the original camera file, and embedded author watermarks are not removed.

Live images are identified in the selection grid. Still images are saved by default; an opt-in checkbox also saves the corresponding MP4 using the same original image number. This is an image/video pair, not a guaranteed importable iPhone Live Photo package. Note information is collapsed by default to leave room for the image, with an on-demand information button.

Previously named Shiguang; renamed StillDock at v0.2.0. Existing profiles, task databases and installation paths are retained.

## Run

Download the Windows x64 installer or portable ZIP from Releases. The installer runs per user and downloads Microsoft WebView2 Runtime if needed. The portable ZIP requires an existing WebView2 Runtime. Supported target: 64-bit Windows 10/11. Sign in inside the application; everyday Chrome cookies are not imported.

This is an **unsigned early preview**. A separate Windows 10 machine and long-running stress tests have not been completed. Website compatibility can change; automated tests do not establish live-site compatibility.

## Local data

Profiles, settings and SQLite tasks use `%LOCALAPPDATA%\XhsBrowserRust`. Downloads default to `%USERPROFILE%\Downloads\XHS\<note-id>\<job-id>`. These names are retained for compatibility with earlier Rust previews. Portable means installation-free, not self-contained profile storage. Uninstalling preserves user data.

No application telemetry or local HTTP server is configured. Cookies are not exported to the task database. Stored note metadata and media URLs may contain access parameters; do not attach your data directory to public issues.

## Build

On Windows x64, install Rust stable MSVC, Visual Studio C++ Build Tools and Windows SDK. Node.js 22+ is used only for JavaScript tests.

```powershell
cargo fmt --check
cargo test --locked
node --test tests/site.test.cjs tests/panel.test.cjs tests/search.test.cjs tests/bootstrap.test.cjs tests/focus.test.cjs
cargo build --release --locked
```

The output executable is `target/release/xhs-browser.exe`. See the [Chinese build/packaging guide](docs/BUILDING.md) for NSIS packaging and GitHub Actions artifacts.

## Limitations

Windows x64 only. No API/MCP/TUI, author scraping or automatic updater. Media is read from the currently opened, authorized note. Quality depends on the streams/URLs exposed by the website. Finished/cancelled history is limited to the latest 200 entries, with active jobs also retained. Cross-job image deduplication is not implemented. Expired media links may require reopening the note and creating a new task.

## Attribution and license

GPL-3.0-only; see [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md). Extracted from the Rust browser experiment in [hanpeiyu/XHS-Downloader](https://github.com/hanpeiyu/XHS-Downloader), with extraction and page-adaptation work based on [JoeanAmier/XHS-Downloader](https://github.com/JoeanAmier/XHS-Downloader) and its derivative workspace. This project is not affiliated with Xiaohongshu.
