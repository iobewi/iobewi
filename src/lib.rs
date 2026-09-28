#![no_std]

//! ESP capabilities for IOBEWI. Enable the matching chip feature.

#[cfg(feature = "esp32s3")]
pub mod http;
