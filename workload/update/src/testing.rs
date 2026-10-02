//! Test support (feature `test-util`, and unit tests): a fake NOR flash that behaves
//! like the real thing where it matters -- 4 KiB erase units, programming only
//! clears bits, a 4-byte write unit, and a crash injected at any erase/program
//! leaves that operation half done -- plus a [`FlashAccess`] over it.

use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use embedded_storage::nor_flash::{ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash};

use crate::flash::FlashAccess;

pub const ERASE: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeError {
    Crash,
    Misaligned,
    Range,
}

impl NorFlashError for FakeError {
    fn kind(&self) -> NorFlashErrorKind {
        NorFlashErrorKind::Other
    }
}

pub struct Fake {
    pub data: Vec<u8>,
    pub ops: usize,
    pub crash_at: Option<usize>,
}

impl Fake {
    pub fn new(size: u32) -> Self {
        Self { data: vec![0xFF; size as usize], ops: 0, crash_at: None }
    }

    pub fn reboot(&mut self) {
        self.crash_at = None;
    }

    fn crashing_now(&mut self) -> bool {
        let now = self.ops;
        self.ops += 1;
        self.crash_at == Some(now)
    }
}

impl ErrorType for Fake {
    type Error = FakeError;
}

impl ReadNorFlash for Fake {
    const READ_SIZE: usize = 1;
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), FakeError> {
        let end = offset as usize + bytes.len();
        if end > self.data.len() {
            return Err(FakeError::Range);
        }
        bytes.copy_from_slice(&self.data[offset as usize..end]);
        Ok(())
    }
    fn capacity(&self) -> usize {
        self.data.len()
    }
}

impl NorFlash for Fake {
    const WRITE_SIZE: usize = 4;
    const ERASE_SIZE: usize = ERASE as usize;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), FakeError> {
        if from % ERASE != 0 || to % ERASE != 0 || to < from || to as usize > self.data.len() {
            return Err(FakeError::Misaligned);
        }
        if self.crashing_now() {
            // A sector erase cut short leaves the first half erased, the rest as it was.
            let half = from + (to - from) / 2;
            self.data[from as usize..half as usize].fill(0xFF);
            return Err(FakeError::Crash);
        }
        self.data[from as usize..to as usize].fill(0xFF);
        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), FakeError> {
        if offset % 4 != 0 || bytes.len() % 4 != 0 || offset as usize + bytes.len() > self.data.len() {
            return Err(FakeError::Misaligned);
        }
        let crash = self.crashing_now();
        let upto = if crash { bytes.len() / 2 / 4 * 4 } else { bytes.len() };
        for (i, b) in bytes[..upto].iter().enumerate() {
            self.data[offset as usize + i] &= *b; // programming only clears bits
        }
        if crash { Err(FakeError::Crash) } else { Ok(()) }
    }
}


/// A [`FlashAccess`] over a shared fake flash: the "lock" is a `RefCell` borrow
/// held only for the closure, exactly like one `SharedFlash` operation.
pub struct FakeAccess(pub RefCell<Fake>);

impl FakeAccess {
    pub fn new(size: u32) -> Self {
        Self(RefCell::new(Fake::new(size)))
    }
}

impl FlashAccess for FakeAccess {
    type Flash = Fake;

    async fn with<R>(&self, f: impl FnOnce(&mut Fake) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }
}

// ---------------------------------------------------------------------------
// A fake Workload runtime for the Supervisor's host tests.
// ---------------------------------------------------------------------------

use alloc::vec::Vec as StdVec;

use crate::supervisor::{ArtifactReader, Health, Identity, RuntimeError, WorkloadRuntime};

#[derive(Default)]
struct FakeRuntimeState {
    running: Option<Identity>,
    health: Option<Health>,
    report_as: Option<Identity>,
    starts: StdVec<Identity>,
    stops: usize,
    /// Digests whose `start` fails.
    fail_digests: StdVec<[u8; 32]>,
    /// `start` called while something already ran: the Supervisor must stop first.
    overlap: bool,
}

/// Records every call; faults are set per test.
#[derive(Default)]
pub struct FakeRuntime(RefCell<FakeRuntimeState>);

impl FakeRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_start_of(&self, digest: [u8; 32]) {
        self.0.borrow_mut().fail_digests.push(digest);
    }

    pub fn clear_faults(&self) {
        let mut s = self.0.borrow_mut();
        s.fail_digests.clear();
        s.health = None;
        s.report_as = None;
    }

    /// Force the reported health (`None` = derived: Healthy while running).
    pub fn set_health(&self, health: Option<Health>) {
        self.0.borrow_mut().health = health;
    }

    /// Make `running()` claim another artifact than the one started (wrong artifact).
    pub fn report_running_as(&self, identity: Option<Identity>) {
        self.0.borrow_mut().report_as = identity;
    }

    pub fn starts(&self) -> StdVec<Identity> {
        self.0.borrow().starts.clone()
    }

    pub fn stops(&self) -> usize {
        self.0.borrow().stops
    }

    pub fn overlapped(&self) -> bool {
        self.0.borrow().overlap
    }

    pub fn running_now(&self) -> Option<Identity> {
        self.0.borrow().running.clone()
    }
}

impl WorkloadRuntime for FakeRuntime {
    async fn start<R: ArtifactReader>(&self, artifact: &Identity, reader: &R) -> Result<(), RuntimeError> {
        // Read a few bytes through the supervisor's reader, like a real loader would.
        let mut head = [0u8; 4];
        reader.read(0, &mut head).await?;
        let mut s = self.0.borrow_mut();
        if s.running.is_some() {
            s.overlap = true;
        }
        if s.fail_digests.contains(&artifact.digest) {
            return Err(RuntimeError { reason: "fault: start fails" });
        }
        s.running = Some(artifact.clone());
        s.starts.push(artifact.clone());
        Ok(())
    }

    async fn stop(&self) {
        let mut s = self.0.borrow_mut();
        s.stops += 1;
        s.running = None;
    }

    async fn health(&self) -> Health {
        let s = self.0.borrow();
        match (&s.running, s.health) {
            (None, _) => Health::Unknown,
            (Some(_), Some(h)) => h,
            (Some(_), None) => Health::Healthy,
        }
    }

    async fn running(&self) -> Option<Identity> {
        let s = self.0.borrow();
        s.report_as.clone().or_else(|| s.running.clone())
    }
}
