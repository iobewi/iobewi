#![no_std]

//! SNTP synchronization of a Unix epoch clock based on Embassy's monotonic time.
//! The clock is absent until the first valid response; network failures retain
//! the last synchronized value while the service retries.

use core::cell::RefCell;
use core::net::{IpAddr, SocketAddr};

use critical_section::Mutex;
use embassy_net::Stack;
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use log::{info, warn};
use sntpc::{NtpContext, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use sntpc_time_embassy::EmbassyTimestampGenerator;

/// Network endpoint and validation/retry policy owned by the caller.
#[derive(Clone, Copy)]
pub struct SyncOptions {
    pub server: &'static str,
    pub resync_period: Duration,
    pub retry_period: Duration,
    pub plausible_epoch_floor: u64,
}

/// Keep 64-bit clock state valid also on targets without 64-bit atomics.
#[derive(Clone, Copy)]
struct Sync {
    epoch_at_sync_s: u64,
    mono_at_sync_us: u64,
}

static SYNC: Mutex<RefCell<Option<Sync>>> = Mutex::new(RefCell::new(None));
static FIRST_SYNC: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Whether SNTP has converged at least once since boot.
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
    if is_set() {
        return true;
    }
    with_timeout(timeout, FIRST_SYNC.wait()).await.is_ok()
}

/// Resync forever; a network failure retains the last estimate.
#[embassy_executor::task]
pub async fn sync_task(stack: Stack<'static>, options: SyncOptions) -> ! {
    let mut first_sync_done = false;
    loop {
        match sync_once(stack, options).await {
            Ok(epoch) => {
                let sync = Sync { epoch_at_sync_s: epoch, mono_at_sync_us: Instant::now().as_micros() };
                critical_section::with(|cs| *SYNC.borrow(cs).borrow_mut() = Some(sync));
                if !first_sync_done {
                    first_sync_done = true;
                    FIRST_SYNC.signal(());
                }
                info!("SNTP: synced, ts={epoch}");
                Timer::after(options.resync_period).await;
            }
            Err(e) => {
                warn!("SNTP: sync failed: {e:?}");
                Timer::after(options.retry_period).await;
            }
        }
    }
}

// Fields are read via the derived `Debug` impl (`warn!("... {e:?}")`), which
// rustc's dead-code lint doesn't count as a use.
#[derive(Debug)]
#[allow(dead_code)]
enum SyncError {
    Dns(embassy_net::dns::Error),
    NoAddress,
    Bind(embassy_net::udp::BindError),
    Ntp(sntpc::Error),
    Implausible(u64),
}

async fn sync_once(stack: Stack<'static>, options: SyncOptions) -> Result<u64, SyncError> {
    let addrs = stack
        .dns_query(options.server, DnsQueryType::A)
        .await
        .map_err(SyncError::Dns)?;
    let addr: IpAddr = (*addrs.first().ok_or(SyncError::NoAddress)?).into();

    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buffer = [0u8; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buffer = [0u8; 128];
    let mut socket =
        UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);
    socket.bind(123).map_err(SyncError::Bind)?;
    let socket = UdpSocketWrapper::new(socket);

    let context = NtpContext::new(EmbassyTimestampGenerator::default());
    let result = get_time(SocketAddr::from((addr, 123)), &socket, context)
        .await
        .map_err(SyncError::Ntp)?;

    let epoch = result.sec();
    if epoch < options.plausible_epoch_floor {
        return Err(SyncError::Implausible(epoch));
    }
    Ok(epoch)
}
