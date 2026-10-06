fn main() {
    println!("cargo:rustc-link-arg=-Tlinkall.x");
    for key in ["AP31_STA_SSID", "AP31_STA_PASSWORD", "AP31_AP_PASSWORD"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
}
