# 来源与第三方说明

原帧 StillDock 是从 `hanpeiyu/XHS-Downloader` 工作区的 `rust-browser/` 实验目录独立整理的 Rust Windows 应用。原仓库上游为 JoeanAmier 及贡献者维护的 XHS-Downloader：

- 上游：https://github.com/JoeanAmier/XHS-Downloader
- 原工作区仓库：https://github.com/hanpeiyu/XHS-Downloader
- 独立整理前，原工作区 Git HEAD：`2ab2620`。Rust 目录当时属于未提交的新实现，因此此提交仅标识来源工作区基线，不代表包含全部提取源码。
- 媒体数据读取规则及专注/AI 过滤脚本延续了原项目及工作区开发成果；未将原 Python 应用作为运行依赖。

本次独立版本于 2026-10-30 整理，由 Han Peiyu 维护。保留 GPL-3.0-only 许可，完整条款见 LICENSE。上游作者及贡献者对其原有成果享有相应权利，本说明不将其贡献重新归属给本项目维护者。

依赖包含 Tao、Wry、Serde、Tokio、Reqwest、Rusqlite/SQLite、rfd、fs2、imagesize 等，精确版本与来源见 Cargo.lock，各依赖按其自身许可使用。WebView2 Runtime 是由 Microsoft 提供和许可的运行时；安装包可能包含从微软官方下载的安装引导程序，它不属于本项目的 GPL 源码。

“小红书”及相关标识属于其各自权利人。浏览页面中的原生品牌标识不表示官方授权或合作。

分发本项目修改版或二进制时，须保留许可及来源说明，并按 GPL 提供对应源码。发布 tag 对应的源码可从本仓库下载。

项目最初以“拾光 Shiguang”发布，v0.2.0 更名为“StillDock · 原帧”。更名不改变原有成果的来源或 GPL 许可。
