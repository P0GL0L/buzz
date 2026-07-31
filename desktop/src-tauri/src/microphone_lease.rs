use std::sync::atomic::{AtomicU16, Ordering};

/// Exclusive microphone lease value for a live huddle.
pub const MICROPHONE_OWNER_HUDDLE: u16 = 1;
/// Exclusive microphone lease value for composer dictation.
pub const MICROPHONE_OWNER_DICTATION: u16 = 2;

/// Process-local microphone ownership shared by huddles and dictation.
#[derive(Default)]
pub struct MicrophoneLeaseRuntime {
    owner: AtomicU16,
}

impl MicrophoneLeaseRuntime {
    /// Atomically reserve the microphone for one capture owner.
    pub fn claim(
        &self,
        owner: u16,
        owner_label: &'static str,
    ) -> Result<MicrophoneClaim<'_>, String> {
        self.owner
            .compare_exchange(0, owner, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|active| {
                let active_label = match active {
                    MICROPHONE_OWNER_HUDDLE => "a huddle",
                    MICROPHONE_OWNER_DICTATION => "composer dictation",
                    _ => "another audio session",
                };
                format!("cannot start {owner_label}: {active_label} is using the microphone")
            })?;
        Ok(MicrophoneClaim {
            runtime: self,
            owner,
            retained: false,
        })
    }

    /// Release the lease only if `owner` still holds it.
    pub fn release(&self, owner: u16) {
        let _ = self
            .owner
            .compare_exchange(owner, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

/// Rollback-safe microphone reservation retained after session setup succeeds.
pub struct MicrophoneClaim<'a> {
    runtime: &'a MicrophoneLeaseRuntime,
    owner: u16,
    retained: bool,
}

impl MicrophoneClaim<'_> {
    /// Keep the lease after this setup guard leaves scope.
    pub fn retain(&mut self) {
        self.retained = true;
    }
}

impl Drop for MicrophoneClaim<'_> {
    fn drop(&mut self) {
        if !self.retained {
            self.runtime.release(self.owner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MicrophoneLeaseRuntime, MICROPHONE_OWNER_DICTATION, MICROPHONE_OWNER_HUDDLE};

    #[test]
    fn claims_are_exclusive_and_rollback_safe() {
        let runtime = MicrophoneLeaseRuntime::default();
        {
            let _claim = runtime
                .claim(MICROPHONE_OWNER_DICTATION, "composer dictation")
                .unwrap();
            assert!(runtime.claim(MICROPHONE_OWNER_HUDDLE, "a huddle").is_err());
        }
        let mut huddle = runtime.claim(MICROPHONE_OWNER_HUDDLE, "a huddle").unwrap();
        huddle.retain();
        drop(huddle);
        assert!(runtime
            .claim(MICROPHONE_OWNER_DICTATION, "composer dictation")
            .is_err());
        runtime.release(MICROPHONE_OWNER_HUDDLE);
        assert!(runtime
            .claim(MICROPHONE_OWNER_DICTATION, "composer dictation")
            .is_ok());
    }
}
