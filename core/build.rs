fn main() {
    println!("cargo:rerun-if-changed=icons/materialbox.ico");
    println!("cargo:rerun-if-changed=icons/materialbox.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("icon.o");
    let compiler = std::env::var_os("MATERIALBOX_WINDRES").unwrap_or_else(|| "windres.exe".into());
    let status = std::process::Command::new(compiler)
        .current_dir(manifest.join("icons"))
        .args(["--codepage=65001", "-i", "materialbox.rc", "-O", "coff", "-o"])
        .arg(&output)
        .status()
        .expect("Windows resource compiler is required");
    assert!(status.success(), "Application icon resource compilation failed");
    println!("cargo:rustc-link-arg-bin=materialbox={}", output.display());
}
