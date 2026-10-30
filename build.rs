fn main() {
    println!("cargo:rerun-if-changed=assets/brand/stilldock.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/brand/stilldock.ico")
            .set_language(0x0804)
            .set("ProductName", "StillDock · 原帧")
            .set("FileDescription", "StillDock · 原帧素材下载工具")
            .set("OriginalFilename", "xhs-browser.exe")
            .set("LegalCopyright", "StillDock contributors · GPL-3.0-only")
            .compile()
            .expect("failed to compile Windows application icon and version resources");
    }
}
