#![no_std]

//! Local log capture: a bounded ring of formatted lines and the global
//! `log` logger that fills it. Knows nothing about any network, transport or
//! streaming policy -- a firmware without a network uses this crate alone.
//!
//! There is exactly one logger and one ring per image: both are
//! process-lifetime statics owned here; consumers (such as `iobewi-log-stream`)
//! read the same ring through [`pop_line`] / [`discard`].

extern crate alloc;

use alloc::string::String;
use core::{cell::RefCell, fmt::Write as _};
use critical_section::Mutex;
use heapless::{Deque, String as FixedString};
use log::{Level, LevelFilter, Metadata, Record};
use static_cell::StaticCell;

/// Maximum captured length of one log line (longer lines are dropped).
pub const LINE_MAX: usize = 160;
/// Number of lines the ring holds; when full, new lines are dropped.
pub const RING_CAPACITY: usize = 24;

/// One captured, formatted log line.
pub type Line = FixedString<LINE_MAX>;

static RING: Mutex<RefCell<Deque<Line, RING_CAPACITY>>> =
    Mutex::new(RefCell::new(Deque::new()));
static LOGGER: StaticCell<Logger> = StaticCell::new();

struct Logger {
    print: fn(&Record<'_>),
    application_target: &'static str,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        // `iobewi_log` is the namespace of this crate family (`iobewi_log`,
        // `iobewi_log_stream`): those targets log at Info like the application.
        if metadata.target().starts_with(self.application_target)
            || metadata.target().starts_with("iobewi_log") {
            metadata.level() <= Level::Info
        } else {
            metadata.level() <= Level::Warn
        }
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) { return; }
        (self.print)(record);
        let mut line: Line = FixedString::new();
        if write!(line, "{}", record.args()).is_err() { return; }
        critical_section::with(|cs| {
            let mut ring = RING.borrow(cs).borrow_mut();
            if !ring.is_full() { let _ = ring.push_back(line); }
        });
    }

    fn flush(&self) {}
}

/// Install once, during single-threaded startup, before other code logs.
/// The platform owns local console output and chooses the app log target.
pub fn install(print: fn(&Record<'_>), application_target: &'static str) {
    let logger = LOGGER.init(Logger { print, application_target });
    // SAFETY: the logger is process-lifetime storage and installation takes
    // place only once, before the executor and interrupt-driven loggers start.
    unsafe {
        let _ = log::set_logger_racy(logger);
        log::set_max_level_racy(LevelFilter::Info);
    }
}

/// Removes and returns the oldest captured line.
pub fn pop_line() -> Option<Line> {
    critical_section::with(|cs| RING.borrow(cs).borrow_mut().pop_front())
}

/// Drops every captured line.
pub fn discard() {
    critical_section::with(|cs| RING.borrow(cs).borrow_mut().clear());
}

/// Facts attached to every log line by whoever ships it (identity of the
/// node, which firmware is running, when). These are log metadata, not
/// transport settings: the application supplies them, `log/core` does not
/// know where they come from.
#[allow(async_fn_in_trait)]
pub trait LogMetadata {
    async fn node_id(&self) -> String;
    fn timestamp(&self) -> u64;
    fn workload(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use log::Log;

    fn noop(_: &Record<'_>) {}

    fn emit(logger: &Logger, level: Level, target: &str, text: &str) {
        logger.log(&Record::builder().level(level).target(target).args(format_args!("{text}")).build());
    }

    // One sequential test: the ring is a process-wide static shared by all tests.
    #[test]
    fn ring_capture_levels_overflow_and_discard() {
        let logger = Logger { print: noop, application_target: "app" };
        discard();

        // Application and the iobewi_log family log at Info, others only at Warn.
        emit(&logger, Level::Info, "app::sub", "app info");
        emit(&logger, Level::Info, "iobewi_log_stream", "stream info");
        emit(&logger, Level::Info, "smoltcp", "dropped info");
        emit(&logger, Level::Warn, "smoltcp", "kept warn");
        assert_eq!(pop_line().unwrap().as_str(), "app info");
        assert_eq!(pop_line().unwrap().as_str(), "stream info");
        assert_eq!(pop_line().unwrap().as_str(), "kept warn");
        assert!(pop_line().is_none());

        // A line longer than LINE_MAX is dropped, not truncated.
        let long = "x".repeat(LINE_MAX + 1);
        emit(&logger, Level::Warn, "app", &long);
        assert!(pop_line().is_none());

        // When the ring is full, new lines are dropped (oldest are kept).
        for i in 0..RING_CAPACITY + 5 {
            emit(&logger, Level::Warn, "app", &std::format!("line {i}"));
        }
        assert_eq!(pop_line().unwrap().as_str(), "line 0");
        let mut n = 1;
        while pop_line().is_some() { n += 1; }
        assert_eq!(n, RING_CAPACITY);

        emit(&logger, Level::Warn, "app", "again");
        discard();
        assert!(pop_line().is_none());
    }
}
