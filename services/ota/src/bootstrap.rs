//! Persistent first-install bootstrap lifecycle for OTA-capable devices.
//!
//! This state machine is deliberately separate from the normal OTA
//! transaction state. It describes only the one-time path from a fresh
//! device to its first confirmed production agent:
//!
//! Factory -> Provisioning -> ReadyForAgent -> Production.
//!
//! Persistence is supplied through IOBEWI ConfigSpace, so this module has no
//! knowledge of NVS, flash layout, ESP, or any other platform mechanism.

use iobewi_config_space::{Budget, ConfigBackend, ConfigSpace};

const MAGIC: &[u8; 4] = b"LFC2";
const ENCODED_LEN: usize = 5;

/// Durable space required by the bootstrap lifecycle.
pub const CONFIG_BUDGET: Budget = Budget::new(ENCODED_LEN);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BootstrapState {
    Factory = 0,
    Provisioning = 1,
    ReadyForAgent = 2,
    Production = 3,
}

impl BootstrapState {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Factory),
            1 => Some(Self::Provisioning),
            2 => Some(Self::ReadyForAgent),
            3 => Some(Self::Production),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Factory => "factory",
            Self::Provisioning => "provisioning",
            Self::ReadyForAgent => "ready_for_agent",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapError {
    Persistence,
    Corrupt,
    InvalidTransition {
        from: BootstrapState,
        to: BootstrapState,
    },
}

fn decode(raw: &[u8]) -> Option<BootstrapState> {
    if raw.len() != ENCODED_LEN || &raw[..4] != MAGIC {
        return None;
    }
    BootstrapState::from_u8(raw[4])
}

fn encode(state: BootstrapState) -> [u8; ENCODED_LEN] {
    let mut encoded = [0u8; ENCODED_LEN];
    encoded[..4].copy_from_slice(MAGIC);
    encoded[4] = state as u8;
    encoded
}

/// Load the bootstrap lifecycle. Absence means a genuinely fresh device.
/// A present but malformed record is never treated as Factory.
pub async fn state<B: ConfigBackend>(
    space: &ConfigSpace<B>,
) -> Result<BootstrapState, BootstrapError> {
    match space.load().await {
        Ok(None) => Ok(BootstrapState::Factory),
        Ok(Some(snapshot)) => decode(&snapshot.data).ok_or(BootstrapError::Corrupt),
        Err(_) => Err(BootstrapError::Persistence),
    }
}

async fn commit<B: ConfigBackend>(
    space: &ConfigSpace<B>,
    next: BootstrapState,
) -> Result<(), BootstrapError> {
    space
        .commit(&encode(next))
        .await
        .map(|_| ())
        .map_err(|_| BootstrapError::Persistence)
}

async fn transition<B: ConfigBackend>(
    space: &ConfigSpace<B>,
    expected: BootstrapState,
    next: BootstrapState,
) -> Result<(), BootstrapError> {
    let current = state(space).await?;
    if current == next {
        return Ok(());
    }
    if current != expected {
        return Err(BootstrapError::InvalidTransition {
            from: current,
            to: next,
        });
    }
    commit(space, next).await
}

/// Start or resume the one-time provisioning phase.
pub async fn begin_provisioning<B: ConfigBackend>(
    space: &ConfigSpace<B>,
) -> Result<(), BootstrapError> {
    transition(
        space,
        BootstrapState::Factory,
        BootstrapState::Provisioning,
    )
    .await
}

/// Mark all durable prerequisites ready so the first production agent may be
/// activated through the normal OTA transaction path.
pub async fn ready_for_agent<B: ConfigBackend>(
    space: &ConfigSpace<B>,
) -> Result<(), BootstrapError> {
    transition(
        space,
        BootstrapState::Provisioning,
        BootstrapState::ReadyForAgent,
    )
    .await
}

/// Complete first-install bootstrap after the first agent image has been
/// confirmed by the normal OTA validation path.
pub async fn production<B: ConfigBackend>(
    space: &ConfigSpace<B>,
) -> Result<(), BootstrapError> {
    transition(
        space,
        BootstrapState::ReadyForAgent,
        BootstrapState::Production,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::cell::RefCell;
    use core::future::Future;
    use core::task::{Context, Poll, Waker};
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use iobewi_config_space::{ConfigManager, Snapshot};

    fn ready<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut future = core::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut Context::from_waker(waker)) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Default)]
    struct MemoryState {
        values: BTreeMap<String, Snapshot>,
        generations: BTreeMap<String, u64>,
    }

    #[derive(Clone)]
    struct MemoryBackend {
        state: Rc<RefCell<MemoryState>>,
    }

    impl MemoryBackend {
        fn new() -> Self {
            Self {
                state: Rc::new(RefCell::new(MemoryState::default())),
            }
        }
    }

    impl ConfigBackend for MemoryBackend {
        type Error = ();

        fn capacity_units(&self) -> usize {
            1024
        }

        fn reservation_units(&self, _space: &str, budget: Budget) -> Option<usize> {
            Some(budget.max_bytes())
        }

        async fn load(&self, space: &str) -> Result<Option<Snapshot>, Self::Error> {
            Ok(self.state.borrow().values.get(space).cloned())
        }

        async fn commit(&self, space: &str, data: &[u8]) -> Result<u64, Self::Error> {
            let mut state = self.state.borrow_mut();
            let generation = state
                .generations
                .get(space)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .unwrap();
            state.generations.insert(String::from(space), generation);
            state.values.insert(
                String::from(space),
                Snapshot {
                    generation,
                    data: Vec::from(data),
                },
            );
            Ok(generation)
        }

        async fn clear(&self, space: &str) -> Result<u64, Self::Error> {
            let mut state = self.state.borrow_mut();
            let generation = state
                .generations
                .get(space)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .unwrap();
            state.generations.insert(String::from(space), generation);
            state.values.remove(space);
            Ok(generation)
        }
    }

    fn space() -> ConfigSpace<MemoryBackend> {
        let mut manager = ConfigManager::new(MemoryBackend::new());
        manager.claim("bootstrap", CONFIG_BUDGET).unwrap()
    }

    #[test]
    fn fresh_device_is_factory() {
        let space = space();
        assert_eq!(ready(state(&space)), Ok(BootstrapState::Factory));
    }

    #[test]
    fn normal_first_install_path_reaches_production() {
        let space = space();
        ready(begin_provisioning(&space)).unwrap();
        assert_eq!(ready(state(&space)), Ok(BootstrapState::Provisioning));

        ready(ready_for_agent(&space)).unwrap();
        assert_eq!(ready(state(&space)), Ok(BootstrapState::ReadyForAgent));

        ready(production(&space)).unwrap();
        assert_eq!(ready(state(&space)), Ok(BootstrapState::Production));
    }

    #[test]
    fn transitions_are_idempotent_but_cannot_skip_states() {
        let space = space();
        ready(begin_provisioning(&space)).unwrap();
        ready(begin_provisioning(&space)).unwrap();

        assert_eq!(
            ready(production(&space)),
            Err(BootstrapError::InvalidTransition {
                from: BootstrapState::Provisioning,
                to: BootstrapState::Production,
            })
        );
    }
}
