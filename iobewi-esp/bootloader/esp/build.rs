fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let linker = if std::env::var_os("CARGO_FEATURE_ESP32C3").is_some() {
        "iobewi-esp-boot-esp32c3.x"
    } else if std::env::var_os("CARGO_FEATURE_ESP32S3").is_some() {
        "iobewi-esp-boot-esp32s3.x"
    } else {
        panic!("no supported ESP boot target feature selected");
    };

    println!("cargo:rustc-link-search={dir}/linker");
    println!("cargo:rustc-link-arg=-T{linker}");
    println!("cargo:rerun-if-changed=linker/{linker}");
}
