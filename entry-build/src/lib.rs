//! Host-side target/link policy for downstream products. Called from build.rs.
pub fn emit() {
    println!("cargo:rerun-if-env-changed=TARGET");
    let target = std::env::var("TARGET").expect("Cargo TARGET is required");
    if target != "xtensa-esp32s3-none-elf" {
        panic!("iobewi-entry-build: unsupported target {target}; expected xtensa-esp32s3-none-elf");
    }
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}
