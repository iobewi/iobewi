fn main() {
    println!("cargo:rustc-link-arg=-Tlinkall.x");
    println!("cargo:rerun-if-env-changed=SETUP_AP_PASSWORD");
}
