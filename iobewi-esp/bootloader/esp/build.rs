fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    // The memory map is a SoC fact: the linker scripts live with the arch code
    // (`arch/esp32/<soc>/linker`), the bootloader only selects one.
    let (soc, linker) = if std::env::var_os("CARGO_FEATURE_ESP32C3").is_some() {
        ("c3", "iobewi-esp-boot-esp32c3.x")
    } else if std::env::var_os("CARGO_FEATURE_ESP32S3").is_some() {
        ("s3", "iobewi-esp-boot-esp32s3.x")
    } else {
        panic!("no supported ESP boot target feature selected");
    };

    let search = format!("{dir}/../../../arch/esp32/{soc}/linker");
    println!("cargo:rustc-link-search={search}");
    println!("cargo:rustc-link-arg=-T{linker}");
    println!("cargo:rerun-if-changed={search}/{linker}");
}
