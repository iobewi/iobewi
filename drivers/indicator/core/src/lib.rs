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

/// Forwards only *changes* of status to an indicator, and reports the
/// previous status when one happened -- so the lifecycle owner can log
/// logical transitions (not every write) without keeping its own copy.
/// Starts at [`Status::Booting`], the state every image begins in.
pub struct StatusTracker<'a, I: StatusIndicator> {
    indicator: &'a I,
    current: core::cell::Cell<Status>,
}

impl<'a, I: StatusIndicator> StatusTracker<'a, I> {
    pub fn new(indicator: &'a I) -> Self {
        Self { indicator, current: core::cell::Cell::new(Status::Booting) }
    }

    pub fn current(&self) -> Status {
        self.current.get()
    }

    /// `Some(previous)` when the status changed, `None` when it already was `next`.
    pub fn set(&self, next: Status) -> Option<Status> {
        let previous = self.current.replace(next);
        if previous == next {
            return None;
        }
        self.indicator.set(next);
        Some(previous)
    }
}

/// Platform capability describing how a configurable status indicator may be
/// wired on this target.
///
/// The values are platform pin identifiers, interpreted by the concrete
/// adapter. Applications may expose them in configuration UIs but must not
/// assume any platform-specific electrical meaning.
pub trait StatusIndicatorCapabilities {
    fn configurable_pins(&self) -> &'static [u8];
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

    #[test]
    fn tracker_forwards_only_changes_and_reports_the_previous_status() {
        let indicator = Recording(Cell::new(None));
        let tracker = StatusTracker::new(&indicator);
        assert_eq!(tracker.set(Status::Booting), None);
        assert_eq!(indicator.0.get(), None);
        assert_eq!(tracker.set(Status::Connecting), Some(Status::Booting));
        assert_eq!(tracker.set(Status::Connecting), None);
        assert_eq!(tracker.set(Status::Online), Some(Status::Connecting));
        // Reconnection: Online -> Connecting -> Online.
        assert_eq!(tracker.set(Status::Connecting), Some(Status::Online));
        assert_eq!(tracker.set(Status::Online), Some(Status::Connecting));
        assert_eq!(tracker.current(), Status::Online);
        assert_eq!(indicator.0.get(), Some(Status::Online));
    }
}
