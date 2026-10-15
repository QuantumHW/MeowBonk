fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/platform/target.m");
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
        cc::Build::new()
            .file("src/platform/target.m")
            .flag("-fobjc-arc")
            .compile("meow_target");
        for framework in ["AppKit", "ApplicationServices"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    println!("cargo:rerun-if-changed=icons/icon.png");
    #[cfg(target_os = "windows")]
    {
        // Windows 的可执行文件资源需要 ICO；从项目 PNG 生成到构建目录。
        let source = image::open("icons/icon.png").expect("无法读取应用图标");
        let scaled = source
            .resize(256, 256, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let mut icon = image::RgbaImage::new(256, 256);
        let (x, y) = ((256 - scaled.width()) / 2, (256 - scaled.height()) / 2);
        image::imageops::overlay(&mut icon, &scaled, x.into(), y.into());
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("缺少构建目录"))
            .join("icon.ico");
        icon.save_with_format(&path, image::ImageFormat::Ico)
            .expect("无法生成 Windows 图标");
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path(path)),
        )
        .expect("Tauri 构建准备失败");
    }
    #[cfg(not(target_os = "windows"))]
    tauri_build::build();
}
