fn main() {
    // Embeds the icon shown for the executable in Windows Explorer and the
    // taskbar. The window icon itself is set at runtime in `main.rs`.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../assets/flixparty.ico");
        winresource::WindowsResource::new()
            .set_icon("../../assets/flixparty.ico")
            .compile()
            .expect("Failed to embed the Windows resources");
    }
}
