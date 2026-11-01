fn main() {
    // esp-hal 1.0.0's empty flash alignment reservation inherits SHF_WRITE.
    // Keep its addresses and NOBITS semantics, but declare the flash gap read-only
    // so GNU ld doesn't merge it with .text into an RWX program segment.
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let script =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("watch-linkall.x");
    // Use an absolute INCLUDE: dependency link-search paths precede this crate's.
    std::fs::write(&script, format!("INCLUDE \"memory.x\"\nINCLUDE \"alias.x\"\nINCLUDE \"{root}/ld/esp32s3.x\"\nINCLUDE \"hal-defaults.x\"\n")).unwrap();
    println!("cargo:rerun-if-changed=ld/esp32s3.x");
    println!("cargo:rustc-link-arg=-T{}", script.display());
}
