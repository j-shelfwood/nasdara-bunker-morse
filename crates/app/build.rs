fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    println!("cargo:rerun-if-env-changed=CI");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon.ico");
        res.set("ProductName", "Bunker Morse");
        res.set("FileDescription", "Nasdara bunker Morse decoder");
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 Joris Schelfhout, MIT License",
        );
        if let Err(e) = res.compile() {
            // CI builds the release exe, which must carry its icon and version info
            if std::env::var_os("CI").is_some() {
                panic!("could not embed the Windows resources: {e}");
            }
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
