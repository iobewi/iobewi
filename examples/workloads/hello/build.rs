fn main() {
    // The fixed-link layout of the target, shared by every ESP32-S3 Workload.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workload/sdk/ld");
    std::fs::copy(dir.join("esp32s3-v1.ld"), std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("workload.ld")).unwrap();
    println!("cargo:rustc-link-search={}", std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=../../../workload/sdk/ld/esp32s3-v1.ld");
}
