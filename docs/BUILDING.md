# 构建、测试和打包

## 环境

在 Windows x64 上安装：

1. Rust stable，MSVC 工具链（`x86_64-pc-windows-msvc`）。
2. Visual Studio C++ Build Tools 及 Windows SDK。
3. Microsoft Edge WebView2 Evergreen Runtime（运行需要）。
4. Node.js 22+（仅页面测试需要）。
5. NSIS 3（仅安装包需要）。

不需要 Python、.NET、前端包管理器、外部 Chrome 或 Playwright。`assets/` 中的文件通过 `include_str!` 编译进 exe。

## 测试与构建

```powershell
cargo fmt --check
cargo test --locked
node --test tests/site.test.cjs tests/panel.test.cjs tests/search.test.cjs tests/bootstrap.test.cjs
cargo build --release --locked
.\target\release\xhs-browser.exe
```

建议使用 Cargo.lock，不在发布前自动升级依赖。Rust 测试包含本地回环 HTTP 服务器，用于测试暂停与服务器忽略 Range 的情况；这个服务器不会编译进发布程序。

## 生成分发包

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

脚本会执行 release 构建，寻找 NSIS，下载并验证微软 WebView2 安装引导程序的数字签名，然后生成：

- `dist/Shiguang-<版本>-windows-x64-portable.zip`
- `dist/Shiguang-<版本>-windows-x64-setup.exe`
- `dist/SHA256SUMS.txt`

可指定已有工具与引导程序：

```powershell
.\scripts\package.ps1 -NsisPath 'C:\Program Files (x86)\NSIS\makensis.exe' -WebView2Setup 'C:\Tools\MicrosoftEdgeWebview2Setup.exe'
```

也支持 `-SkipBuild` 使用已构建 exe，以及 `-TargetDirectory` / `CARGO_TARGET_DIR` 指定 Cargo 输出目录。引导程序默认缓存到被 Git 忽略的 `vendor/`；不提交二进制依赖、用户 Cookie、数据库或下载目录。

安装包沿用此前预览版的安装位置 `%LOCALAPPDATA%\Programs\XhsBrowser` 与卸载键 `XhsBrowserRust`，升级保留现有资料。程序数据目录同样保持兼容。

## GitHub Actions

- `CI`：推送/PR 时，在 Windows 上进行格式检查、Rust 测试、页面脚本测试和 release 构建。
- `Build packages`：在 Actions 中手动运行，生成安装包、便携包和校验和工件，不自动公开发布。
- 正式发布：确认对应提交检查通过，创建 `v<版本>` tag，再以预览发布或稳定发布的形式上传匹配该 tag 的包。务必保证 Cargo.toml、tag 与二进制版本一致；目前不使用自动更新。

首次仓库发布使用预览版，不能据此推断真实网站或所有 Win10 机器均已验证。
