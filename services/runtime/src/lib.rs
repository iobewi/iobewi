#![cfg_attr(not(test), no_std)]

//! Portable runtime diagnostics capability.
//!
//! The current need is deliberately tiny: how much stack headroom remains
//! before an overflow (contrat §5's `task_hwm_min`). How that is measured --
//! linker symbols, stack painting, architecture-specific stack-pointer
//! reads -- is entirely the platform's concern; this crate only names the
//! fact a caller needs, not how to obtain it.

/// Platform capability: report runtime facts about the executing firmware.
pub trait RuntimeDiagnostics {
    /// Bytes of stack never touched since measurement began -- the
    /// remaining headroom before an overflow.
    fn stack_headroom_bytes(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(u32);

    impl RuntimeDiagnostics for Fixed {
        fn stack_headroom_bytes(&self) -> u32 {
            self.0
        }
    }

    #[test]
    fn stack_headroom_bytes_reports_the_implementation_value() {
        assert_eq!(Fixed(12_345).stack_headroom_bytes(), 12_345);
    }
}
