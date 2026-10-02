//! The S18 **validation** artifact format -- *not* a Workload ABI.
//!
//! The runtime backend of S18 is a minimal probe whose only job is to make the
//! Supervisor's state machine observable on hardware. Its artifacts carry a tiny
//! header the probe understands; everything after it is opaque payload. Nothing
//! here is, or should become, the final Workload format (Wasm, native or other):
//! the real runtime will replace the probe without touching OTM2 or the
//! Supervisor.
//!
//! ```text
//! offset size field
//! 0      8    magic "S18PROBE"
//! 8      1    format byte (1 = this validation header)
//! 9      1    fault code (see ProbeFault; 0 = none)
//! 10     2    counter period in milliseconds, u16 LE (0 = default 500)
//! 12     4    reserved (zero)
//! ```
//! Faults are only honoured by a probe built with the `workload-supervisor-probe`
//! feature; production images carry no probe at all.

pub const HEADER_LEN: usize = 16;
pub const MAGIC: [u8; 8] = *b"S18PROBE";
pub const DEFAULT_PERIOD_MS: u16 = 500;

/// What the probe does wrong on purpose, to exercise a Supervisor transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeFault {
    None,
    /// `start` fails (activation failure -> automatic rollback).
    FailStart,
    /// Starts, then the progress counter stops advancing (health becomes `Unhealthy`).
    FreezeAfterStart,
    /// Starts but reports `Unhealthy` at once (confirmation refused).
    HealthFail,
    /// Resets the device inside `start` (crash while `Activating`).
    ResetOnStart,
    /// Resets the device inside `stop` (crash while `RollingBack`).
    ResetOnStop,
}

impl ProbeFault {
    pub const fn code(self) -> u8 {
        match self {
            ProbeFault::None => 0,
            ProbeFault::FailStart => 1,
            ProbeFault::FreezeAfterStart => 2,
            ProbeFault::HealthFail => 3,
            ProbeFault::ResetOnStart => 4,
            ProbeFault::ResetOnStop => 5,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => ProbeFault::None,
            1 => ProbeFault::FailStart,
            2 => ProbeFault::FreezeAfterStart,
            3 => ProbeFault::HealthFail,
            4 => ProbeFault::ResetOnStart,
            5 => ProbeFault::ResetOnStop,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeHeader {
    pub fault: ProbeFault,
    pub period_ms: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    Short,
    BadMagic,
    BadFormat(u8),
    BadFault(u8),
}

impl ProbeHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() < HEADER_LEN {
            return Err(HeaderError::Short);
        }
        if bytes[..8] != MAGIC {
            return Err(HeaderError::BadMagic);
        }
        if bytes[8] != 1 {
            return Err(HeaderError::BadFormat(bytes[8]));
        }
        let fault = ProbeFault::from_code(bytes[9]).ok_or(HeaderError::BadFault(bytes[9]))?;
        let period = u16::from_le_bytes([bytes[10], bytes[11]]);
        Ok(Self { fault, period_ms: if period == 0 { DEFAULT_PERIOD_MS } else { period } })
    }

    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[..8].copy_from_slice(&MAGIC);
        out[8] = 1;
        out[9] = self.fault.code();
        out[10..12].copy_from_slice(&self.period_ms.to_le_bytes());
        out
    }
}
