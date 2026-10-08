#![no_std]
#![no_main]

// The whole firmware: the hardware is the `esp32s3` feature of the entry crate, chosen at
// compile time. Everything else is the portable product in the library.
entry_api::entry!(wifi_ap_setup_example::run);
