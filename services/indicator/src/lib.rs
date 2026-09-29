#![cfg_attr(not(test), no_std)]

//! Portable device status indicator capability.
//!
//! [`Status`] is the device's semantic, functional state -- what the
//! firmware is doing, not what a specific display technology looks like.
//! [`StatusIndicator`] lets a caller express "I am now in this state"; the
//! platform implementation decides whether and how that becomes visible
//! (an LED, a display, a log line, or nothing at all). This crate
//! deliberately carries no colour, no GPIO/RMT/peripheral concept, no blink
//! timing, and no rendering loop -- those belong entirely to the platform
//! implementation (e.g. `iobewi-esp-indicator`'s WS2812/RMT renderer).

/// The device's functional state. Each variant is a fact about what the
/// firmware is doing, never a description of how it should be shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Booting,
    Ready,
    Scanning,
    Connecting,
    Online,
    Failed,
}

/// Platform capability: request a status change. The caller expresses only
/// the logical state; it never describes how that state is physically
/// rendered.
pub trait StatusIndicator {
    fn set(&self, status: Status);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct Recording(Cell<Option<Status>>);

    impl StatusIndicator for Recording {
        fn set(&self, status: Status) {
            self.0.set(Some(status));
        }
    }

    #[test]
    fn set_records_the_requested_status_verbatim() {
        let indicator = Recording(Cell::new(None));
        indicator.set(Status::Connecting);
        assert_eq!(indicator.0.get(), Some(Status::Connecting));
        indicator.set(Status::Failed);
        assert_eq!(indicator.0.get(), Some(Status::Failed));
    }
}
