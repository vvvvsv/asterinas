// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicU32, Ordering};

use atomic_integer_wrapper::define_atomic_version_of_integer_like_type;
use bitflags::bitflags;

use super::PosixThread;
use crate::prelude::*;

bitflags! {
    pub struct Personality: u32 {
        const ADDR_NO_RANDOMIZE = 0x0040000;
    }
}

impl TryFrom<u32> for Personality {
    type Error = Error;

    fn try_from(value: u32) -> core::result::Result<Self, Self::Error> {
        Self::from_bits(value)
            .ok_or_else(|| Error::with_message(Errno::EINVAL, "invalid personality"))
    }
}

impl From<Personality> for u32 {
    fn from(value: Personality) -> Self {
        value.bits()
    }
}

define_atomic_version_of_integer_like_type!(Personality, try_from = true, {
    /// An atomic version of `Personality`.
    #[derive(Debug)]
    pub(super) struct AtomicPersonality(AtomicU32);
});

impl PosixThread {
    pub fn personality(&self) -> Personality {
        self.personality.load(Ordering::Relaxed)
    }

    pub fn set_personality(&self, personality: Personality) {
        self.personality.store(personality, Ordering::Relaxed);
    }
}
