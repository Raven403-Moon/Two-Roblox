fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Two-Roblox");
        res.set("FileDescription", "Two-Roblox Client Launcher");
        res.set("LegalCopyright", "Copyright (c) 2026 Two-Roblox");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }
    }
}
