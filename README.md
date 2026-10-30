<div align="center">

<img src="assets/brand/stilldock-256.png" width="96" height="96" alt="StillDock 图标">

# 原帧 StillDock

**小红书高清无水印素材下载工具，专注大图与 Live 动态素材。**

为 Windows 打造，使用 Rust 与内嵌 WebView2，在同一个窗口完成搜索、大图浏览、图集挑选与下载。

[下载预览版](https://github.com/hanpeiyu/stilldock/releases) · [English](README.en.md) · [构建与打包](docs/BUILDING.md) · [问题反馈](https://github.com/hanpeiyu/stilldock/issues)

</div>

## 它能做什么

- **原生搜索**：保留小红书的搜索记录、建议与登录交互。
- **专注浏览**：首页居中原生搜索框，精简无关导航，支持屏蔽点点 AI、折叠评论；随时恢复原网页。
- **同窗下载**：开始搜索时自动展开侧栏，浏览区域随窗口大小调整。
- **看图挑选**：图集缩略图按需加载，支持编号、勾选、全选/清空和放大预览，也可保存整篇笔记。
- **可靠队列**：图片和视频下载、暂停/继续/取消、退出恢复、本地历史及按笔记打开目录。
- **轻量分发**：HTML/CSS/JS 编译进 exe；运行不需要 Python、Node.js、.NET 或外部 Chrome。

原名“拾光 Shiguang”，自 v0.2.0 更名为 **StillDock · 原帧**。这是独立的 Rust 项目，只服务于 Windows 本地素材浏览，不包含原项目的 API、MCP、TUI 或批量作者采集端。

## 高清与 Live：实际如何保存

- **优先高清源**：从已打开笔记的媒体地址读取可用的高清候选，优先平台提供的高清 JPEG，读取保存文件的实际分辨率；不在本地二次编码。
- **不静默降质**：有高清候选时直接下载该资源，失败就提示重试；图片分辨率低于笔记标注时拒绝保存为最终文件，不自动改下低清预览图。
- **下载报告**：每个任务的 `download-report.json` 记录实际像素尺寸、文件格式、字节数和资源来源。
- **Live 成组保存**：默认保存静态图片；下载按钮上方的 LIVE 开关始终可见并记忆选择，勾选后，以相同原图编号保存图片和 MP4。缩略图带 LIVE 标记，选图会同时选中对应的一组资源。
- **快捷返回**：笔记打开后左上角显示“← 返回搜索”，支持 Esc；优先使用原生关闭保留列表滚动位置，不依赖弹层周围留白。
- **大图优先**：原网页笔记信息区默认折叠，点“笔记信息”临时展开；打开笔记后侧栏回到当前笔记下载页。

“高清无水印”是下载目标，不能保证每篇笔记都暴露原始最高画质或无水印资源。平台返回的图片若已含作者嵌入水印，本程序不做抹除或修图。JPEG 资源也不等同于相机原始无损文件。Live 当前保存为图片＋MP4，不保证可直接回导为 iPhone Live Photo。

## 快速开始

1. 到 [Releases](https://github.com/hanpeiyu/stilldock/releases) 下载 Windows x64 预览版。
2. 安装包 `StillDock-<版本>-windows-x64-setup.exe` 按当前用户安装；便携包解压后运行 `xhs-browser.exe`。
3. 首次打开，在应用内完成小红书登录。搜索后打开笔记，在右侧保存整篇或勾选部分图片。

**环境**：64 位 Windows 10/11，以及 [Microsoft Edge WebView2 Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。已安装时复用；安装包在缺少 Runtime 时联网补装。便携版需要自行准备 Runtime。应用需要联网访问小红书。

> 当前为早期预览版：未代码签名，尚未在独立 Windows 10 设备完成验证，也未完成长时间压力测试。自动化测试不能代替真实网站验收。

## 数据与隐私

| 内容 | 默认位置/行为 |
| --- | --- |
| 登录会话、设置与任务库 | `%LOCALAPPDATA%\XhsBrowserRust` |
| 下载文件 | `%USERPROFILE%\Downloads\XHS\<笔记ID>\<任务ID>\` |
| 本地任务数据库 | 数据目录下 `library.sqlite3` |
| 笔记信息 | 每个任务目录中的 `note.json` 与 `download-report.json` |
| 卸载 | 保留登录数据和已下载文件 |

沿用此前 Rust 预览版的数据目录及可执行文件名，方便已有用户直接升级。便携版仅指无需安装，登录数据仍写入上述本机目录。

程序不读取日常 Chrome 的 Cookie，不导出 Cookie 到任务库，也不设置遥测或本地 HTTP 服务。登录由内嵌 WebView2 管理。`note.json` 与任务库包含标题、作者和带访问参数的链接，反馈问题时请勿上传整个数据目录。

每次下载任务使用独立子目录，避免补选图片时覆盖另一任务的文件；跨任务图片去重尚未实现。

## 已知边界

- 网站结构变化可能影响搜索精简和媒体解析，可先切换“显示原网页”排查。
- 仅保存当前账号有权查看的笔记；不会绕过登录、验证码或平台访问限制。
- 高清源和实际尺寸取决于平台提供的资源；不可用时提示失败，不静默降质。无法保证为相机原始文件或不存在作者嵌入水印。
- 历史界面保留所有进行中的任务，并展示最近 200 条已完成/取消记录；不是全库搜索。
- 已过期媒体链接可能需要重新打开笔记后创建下载任务。
- 仅 Windows x64；不包含 macOS、Linux、移动端和自动更新。

## 开发与发布

需要 Rust stable MSVC、Visual Studio C++ Build Tools、Windows SDK；页面单元测试还需要 Node.js 22 或更新版本。Node.js 仅用于开发。

```powershell
git clone https://github.com/hanpeiyu/stilldock.git
cd stilldock
cargo fmt --check
cargo test --locked
node --test tests/site.test.cjs tests/panel.test.cjs tests/search.test.cjs tests/bootstrap.test.cjs tests/focus.test.cjs tests/navigation.test.cjs
cargo build --release --locked
```

入口为 `src/main.rs`。详见 [构建与打包](docs/BUILDING.md)、[架构说明](docs/ARCHITECTURE.md)、[更新记录](CHANGELOG.md) 和 [后续计划](docs/ROADMAP.md)、[验证记录](docs/VALIDATION.md)。GitHub Actions 在 Windows 上运行格式检查、Rust/页面测试及发布构建；手动构建可下载安装包和便携包工件。

## 来源与许可

GPL-3.0-only，见 [LICENSE](LICENSE) 和 [NOTICE.md](NOTICE.md)。本项目从 [hanpeiyu/XHS-Downloader](https://github.com/hanpeiyu/XHS-Downloader) 工作区中的 Rust 浏览器实验独立而来；媒体提取与网页适配参考并延续 [JoeanAmier/XHS-Downloader](https://github.com/JoeanAmier/XHS-Downloader) 及其衍生工作区的成果。感谢原项目作者和贡献者。

本项目与小红书官方无隶属关系。请尊重内容作者权益，仅保存有权使用的内容。重新分发程序时应按 GPL 提供相应源码及许可说明。
