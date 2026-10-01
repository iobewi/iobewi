#![no_std]

//! Portable Unix epoch clock state, independent of any network stack.
//!
//! The clock is absent until a time source (for example `iobewi-ntp`'s SNTP
//! task) records a first valid epoch with [`set_synced`]; afterwards [`now`]
//! extrapolates from Embassy's monotonic time. A later network failure in the
//! time source retains the last synchronized value.

use core::cell::RefCell;

use critical_section::Mutex;
use embassy_time::{with_timeout, Duration, Instant, Timer};

/// Keep 64-bit clock state valid also on targets without 64-bit atomics.
#[derive(Clone, Copy)]
struct Sync {
    epoch_at_sync_s: u64,
    mono_at_sync_us: u64,
}

static SYNC: Mutex<RefCell<Option<Sync>>> = Mutex::new(RefCell::new(None));

/// Whether a time source has recorded a valid epoch at least once since boot.
pub fn is_set() -> bool {
    critical_section::with(|cs| SYNC.borrow(cs).borrow().is_some())
}

/// Current Unix epoch seconds UTC, or `None` before the first valid sync.
pub fn now() -> Option<u64> {
    let sync = critical_section::with(|cs| *SYNC.borrow(cs).borrow())?;
    let elapsed_us = Instant::now().as_micros().saturating_sub(sync.mono_at_sync_us);
    Some(sync.epoch_at_sync_s + elapsed_us / 1_000_000)
}

/// Blocks until the first sync completes or `timeout` elapses. Returns
/// `true` immediately if already synced from an earlier call.
pub async fn wait(timeout: Duration) -> bool {
    let observed = with_timeout(timeout, async {
        while !is_set() {
            Timer::after_millis(50).await;
        }
    }).await.is_ok();
    observed || is_set()
}


/// Records `epoch_s` (Unix seconds UTC) as the current time, anchored to the
/// monotonic clock at the instant of the call.
pub fn set_synced(epoch_s: u64) {
    let sync = Sync { epoch_at_sync_s: epoch_s, mono_at_sync_us: Instant::now().as_micros() };
    critical_section::with(|cs| *SYNC.borrow(cs).borrow_mut() = Some(sync));
}
