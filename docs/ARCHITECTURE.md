# 架构

## 运行结构

```text
Tao 原生窗口 / 事件循环
├─ Wry WebView2：小红书页面
│  ├─ bootstrap.js：首屏基础样式与有限时长遮罩
│  ├─ site.js：笔记读取、路由变化和搜索意图 → IPC
│  ├─ focus.js：原生搜索框定位、专注布局与评论折叠
│  └─ ai-filter.js：已知 AI 元素过滤
├─ Wry WebView2：本地下载侧栏（panel.html，编译进 exe）
│  └─ 操作 → 独立 IPC → Rust 命令处理
└─ 下载线程 / Tokio 单线程运行时
   ├─ 复用 Reqwest HTTP Client，当前串行处理媒体
   ├─ 高清/网页候选独立 .part，验证 Range/长度/分辨率后重命名
   └─ SQLite：任务、文件进度、设置 → 通知 UI
```

两个 WebView 是原生子视图，窗口/DPI 变化时由 Rust 更新边界，没有外部浏览器窗口嵌入或窗口重挂接。网页登录会话由 WebView2 的持久化配置目录管理；媒体下载不导出 Cookie。

## 源码入口

| 文件 | 职责 |
| --- | --- |
| `src/main.rs` | 窗口、双 WebView、IPC、设置、任务控制与快照 |
| `src/file_ops.rs` | Windows 原子无覆盖保存、长路径与缓存隐藏 |
| `src/model.rs` | 笔记/媒体模型、URL/目录边界与文件类型识别 |
| `src/store.rs` | SQLite WAL、数据库迁移、任务设置与文件元数据 |
| `src/download.rs` | 流式下载、暂停、续传与网络超时 |
| `assets/panel.html` | 侧栏布局、选图、缩略图与历史 |
| `assets/site.js` | 读取当前打开笔记的页面状态，必要时读取笔记内轮播 |
| `tests/*.test.cjs` | Node VM 中的页面脚本和 UI 行为单元测试 |

页面通过 MutationObserver 合并更新，不使用固定频率的整页轮询。下载进度通知约每 300ms 合并一次。当前的页面 UI 测试使用模拟 DOM，不是浏览器像素测试。

## 边界与恢复

- 顶层导航限制在 HTTPS 小红书域名。侧栏使用内嵌 HTML，不开放本地 HTTP 端口。
- 下载媒体及重定向限制在明确的平台 CDN 域名范围内。
- 数据库在主/下载线程间由互斥锁保护；网络及主要媒体写入不占用 UI 线程。
- 启动时将未完成任务恢复为暂停，避免未经操作自动重启整队列。
- 每个任务拥有独立缓存目录和临时文件，最终素材可按笔记分目录或平铺。服务器不接受 Range 时重新写入临时文件。
- 专注首屏遮罩最长保留 1.5 秒；未知布局不能无限隐藏网页。

## 当前技术债

跨任务去重、历史分页/全库搜索、设置切换后新文档的完整一致性、真实 WebView2 端到端测试、独立 Win10/DPI 与长时间内存测试尚待完善。Rust 本身不保证网页渲染速度，页面网络和 WebView2 占用仍是主要变量。

## 高清与 Live 数据流

Asset 包含网页 URL、高清候选、缩略图、Live 视频地址及笔记标注尺寸。点击下载时重新读取当前笔记，核对笔记 ID 后按原始序号选取；选中 Live 且开启动态下载时展开为同编号的图片与视频两个任务项。旧任务缺失的新字段均有默认值。

高清与网页资源使用独立临时文件，暂停/恢复时不混用两个来源的字节。当前高清候选失败时直接报错重试，不自动降级。imagesize 仅探测文件尺寸，不完整解码图片。文件元数据逐项写入 SQLite，JSON 默认关闭，可在完成后导出一份合并说明；它不声称能检测作者嵌入水印。

`navigation.js` 独立于专注模式，提供固定返回按钮和 Esc，优先点击原生关闭或遮罩，缺失时才退回已知列表历史/同源列表地址。LIVE 开关在下载按钮上方始终显示，设置存入本地数据库。

## 保存策略与迁移

`jobs.layout/temp_folder/export_json` 固定每次任务的保存方式、缓存位置与导出选项。旧任务通过加列迁移标记为 `legacy`，保留原目录与 `.part` 路径。`files` 增加尺寸、格式、资源来源、原图序号和 Live 标记，续传不再依赖目录中的 JSON。旧文件缺少元数据时从实际文件恢复尺寸，不覆盖旧 JSON。

新任务缓存位于最终目录内的隐藏 `.stilldock-cache/<任务ID>`，使缓存和素材保持同卷，避免跨磁盘复制。Windows 使用不替换目标的 MoveFileW 发布文件，遇到重名另选后缀；不同笔记平铺也不会覆盖用户文件。完成后仅清理空缓存目录，不递归删除用户内容。可选 JSON 汇总导出失败不会抹去已保存素材和数据库记录。

## Local availability and relocation

`local_files.rs` inspects only recorded paths and byte sizes on one background worker. A bounded channel allows at most one pending request; local status is separate from persisted job completion and bounded to visible recent history. Database locks are released before filesystem checks. Startup/history checks are bounded to 200 completed tasks; opening a note requests a targeted check.

Repair reuses the existing job and fixed storage options. The transfer loop rechecks each saved file before skipping; non-NotFound IO errors stop the task. Relocation is explicit, requires all filenames/sizes to match, and updates job folder, cache path and file paths in one SQLite transaction guarded by completed state and original folder. No watcher, recursive disk scan, automatic deletion, or content-hash guarantee is introduced.
