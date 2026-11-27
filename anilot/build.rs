// Встраивает иконку в .exe (файл создаётся в CI скриптом scripts/make_icons.py).
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        if std::path::Path::new("assets/icon.ico").exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            res.set("ProductName", "AniLot");
            res.set("FileDescription", "AniLot — неофициальный клиент Anixart");
            let _ = res.compile();
        }
    }
}
