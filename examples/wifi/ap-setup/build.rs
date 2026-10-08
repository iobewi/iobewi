// The firmware needs the entry crate's linker/descriptor emission; a host build needs nothing.
#[cfg(feature = "esp32s3")]
fn main() {
    iobewi_entry_build::emit();
}

#[cfg(not(feature = "esp32s3"))]
fn main() {}
