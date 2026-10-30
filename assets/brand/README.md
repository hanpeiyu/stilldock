# StillDock · 原帧应用图标

生成方式：内置 `image_gen` 生图工具，2026-10-30。主题是“相框中的图片落入收纳托盘”，采用与应用深色界面和珊瑚红操作色协调的配色。没有使用小红书商标或用户素材。

## 文件

- `stilldock-master.png`：原始生成结果，1254×1254 RGBA，保留透明外边角。
- `stilldock-16.png` 至 `stilldock-1024.png`：等比例缩放的 PNG 导出。
- `stilldock.ico`：Windows 多尺寸图标，包含 16、24、32、48、64、128、256 像素。
- `window-icon.rgba`：64×64 原始 RGBA，供 Tao 窗口/任务栏使用，避免为读取图标增加运行时图片解码依赖。
- `icon-64.data-uri.txt`：同一 64px PNG 的内嵌 Data URI，用于本地侧栏、favicon 和首屏加载提示，不产生额外网络请求。

源图未进行内容修改，仅缩放和转换导出格式。图标随本项目在 GPL-3.0-only 下提供。修改品牌图标时应同步更新这些派生文件；`build.rs` 将 ICO 嵌入 exe 的 Windows 资源，NSIS 安装/卸载程序使用同一 ICO。

## 最终生成提示词

```text
Use case: logo-brand. Asset type: final production Windows application icon for StillDock, a focused high-resolution image and Live media downloader. Create ONE icon, not a presentation or a sheet of variants. A centered rounded-square dark graphite tile (#1b1b20), containing a bold coral-red (#f34b65) photographic frame with a clean warm-white abstract landscape cutout; the lower edge resolves into a shallow docking tray, visually suggesting images saved into a dock. Keep the mark unified, simple, geometric, premium, and recognizable at 16px. Thick shapes, balanced negative space, carefully optically centered. Flat vector-like finish with only very subtle tonal depth. Front-facing, no perspective. The tile should occupy roughly 90% of the square canvas with consistent margins and clean rounded corners. Transparent outside the tile; no shadow outside the tile. 1024x1024 square. No text, letters, initials, numerals, watermark, brand logos, mockup devices, background scenery, borders around the canvas, or extra decoration. Deliver a finished icon ready for export to PNG and Windows ICO.
```
