/// Собирает один и тот же декодер на ESP32 и host-стенде.
pub fn build(root: &std::path::Path) {
    let mut cc = cc::Build::new();
    cc.include(root).file(root.join("tjpgd.c")).file(root.join("watch_bridge.c")).opt_level(2).warnings(false);
    if std::env::var("TARGET").unwrap_or_default().starts_with("xtensa") { cc.compiler("xtensa-esp32s3-elf-gcc").flag("-mlongcalls"); }
    cc.compile("watch_jpeg");
    println!("cargo:rerun-if-changed={}", root.display());
}
