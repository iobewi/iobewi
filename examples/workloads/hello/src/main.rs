//! Native Workload example. Compiled for the target, packed into an IWNI image by
//! `iobewi-workload-pack`, uploaded through the Workload OTA, loaded and run by the Agent.
//! Nothing here refers to the Agent, esp-hal, flash, partitions or OTA: only the safe
//! `iobewi-workload` API.
#![no_std]
#![no_main]

use iobewi_workload::{Context, workload_main};

#[cfg(not(feature = "variant-b"))]
const IDENT: &str = "native-workload-a";
#[cfg(feature = "variant-b")]
const IDENT: &str = "native-workload-b";

#[cfg(not(feature = "variant-b"))]
const PERIOD_MS: u32 = 250;
#[cfg(feature = "variant-b")]
const PERIOD_MS: u32 = 100;

workload_main!(main);

/// Decimal text of `n` into `buf` (no core::fmt: keeps the image small).
fn dec(mut n: u64, buf: &mut [u8; 20]) -> &[u8] {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    &buf[i..]
}

fn main(ctx: &Context) -> i32 {
    let log = ctx.log();
    let time = ctx.time();
    let control = ctx.control();

    log.info(IDENT);
    let t0 = time.now_us();
    let mut tick = 0u32;
    loop {
        #[cfg(not(feature = "fault-ignore-stop"))]
        if control.stop_requested() {
            break;
        }
        control.progress();
        tick += 1;

        if tick % 20 == 0 {
            let mut line = [0u8; 64];
            let mut n = 0;
            for part in [IDENT.as_bytes(), b" alive, elapsed_ms=".as_slice()] {
                line[n..n + part.len()].copy_from_slice(part);
                n += part.len();
            }
            let mut digits = [0u8; 20];
            let d = dec((time.now_us() - t0) / 1000, &mut digits);
            line[n..n + d.len()].copy_from_slice(d);
            n += d.len();
            log.write(iobewi_workload::level::INFO, &line[..n]);
        }

        #[cfg(feature = "fault-panic")]
        if tick == 8 {
            panic!("native-workload fault: panic");
        }
        #[cfg(feature = "fault-null-jump")]
        if tick == 8 {
            // SAFETY (deliberately violated): a jump to address 0 raises an
            // instruction-fetch exception, to observe how the Agent copes.
            unsafe { core::mem::transmute::<usize, extern "C" fn()>(0)() };
        }

        if !time.sleep_ms(PERIOD_MS) {
            #[cfg(not(feature = "fault-ignore-stop"))]
            break;
        }
    }
    log.info("stopping");
    0
}
