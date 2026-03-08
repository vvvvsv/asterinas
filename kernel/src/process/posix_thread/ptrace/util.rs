// SPDX-License-Identifier: MPL-2.0

//! Ptrace utilities for POSIX threads.

#[cfg(target_arch = "x86_64")]
use ostd::arch::cpu::context::{GeneralRegs, c_user_regs_struct};

use crate::{
    prelude::*,
    process::{
        WaitOptions,
        signal::{sig_num::SigNum, signals::Signal},
    },
};

/// The requests that can continue a stopped tracee.
#[derive(Debug)]
pub enum PtraceContRequest {
    Continue(Option<SigNum>),
    #[cfg_attr(not(target_arch = "x86_64"), expect(dead_code))]
    SingleStep(Option<SigNum>),
    #[expect(dead_code)]
    Syscall(Option<SigNum>),
}

impl PtraceContRequest {
    pub(super) fn sig_num(&self) -> Option<SigNum> {
        match self {
            Self::Continue(Some(sig_num))
            | Self::SingleStep(Some(sig_num))
            | Self::Syscall(Some(sig_num)) => Some(*sig_num),
            _ => None,
        }
    }
}

/// The result of a ptrace-stop.
pub enum PtraceStopResult {
    /// The ptrace-stop is continued by the tracer,
    /// or ends because the tracer exits or detaches.
    Continued(Option<Box<dyn Signal>>),
    /// The ptrace-stop is interrupted by `SIGKILL`.
    Interrupted,
    /// The thread is not traced, returning the stop signal back.
    NotTraced(Box<dyn Signal>),
}

/// The signal associated with a ptrace-stop and its later signal delivery.
#[derive(Default)]
pub(super) enum StopDeliverySignal {
    /// The signal that has not yet been reported through `wait`.
    Pending(Box<dyn Signal>),
    /// The signal that has been reported through `wait`.
    Consumed(Box<dyn Signal>),
    /// The signal that is injected by the tracer.
    Injected(Box<dyn Signal>),
    /// No ptrace-stop signal is recorded.
    #[default]
    Empty,
}

impl StopDeliverySignal {
    /// Records the signal associated with a ptrace-stop.
    pub(super) fn stop(&mut self, signal: Box<dyn Signal>) {
        *self = Self::Pending(signal);
    }

    /// Clears and returns the signal associated with a ptrace-stop,
    /// unless it has already been consumed by `wait`.
    pub(super) fn clear(&mut self) -> Option<Box<dyn Signal>> {
        let this = core::mem::replace(self, Self::Empty);

        match this {
            Self::Pending(signal) | Self::Injected(signal) => Some(signal),
            Self::Consumed(_) | Self::Empty => None,
        }
    }

    /// Returns the signal associated with a ptrace-stop,
    /// if it has not yet been reported through `wait`.
    pub(super) fn wait(&mut self, options: WaitOptions) -> Option<&dyn Signal> {
        let this = core::mem::replace(self, Self::Empty);

        match this {
            Self::Pending(signal) => {
                if !options.contains(WaitOptions::WNOWAIT) {
                    *self = Self::Consumed(signal);
                } else {
                    *self = Self::Pending(signal);
                }
                Some(self.get().unwrap())
            }
            Self::Consumed(signal) => {
                *self = Self::Consumed(signal);
                None
            }
            Self::Injected(_) => unreachable!(),
            Self::Empty => None,
        }
    }

    /// Injects a signal by the tracer.
    pub(super) fn inject(&mut self, signal: Box<dyn Signal>) {
        *self = Self::Injected(signal);
    }

    /// Returns the signal associated with a ptrace-stop,
    /// but does not change the state.
    fn get(&self) -> Option<&dyn Signal> {
        match self {
            Self::Pending(signal) | Self::Consumed(signal) | Self::Injected(signal) => {
                Some(signal.as_ref())
            }
            Self::Empty => None,
        }
    }
}

#[cfg(target_arch = "x86_64")]
macro_rules! general_regs_ptrace_setter {
    ([ $field:ident, $($meta:tt)+ ]) => {
        paste::paste! {
            #[inline(always)]
            pub(super) fn [<ptrace_set_ $field>](regs: &mut GeneralRegs, value: usize) -> Result<()> {
                general_regs_ptrace_setter!(@body regs, value, [ $field, $($meta)+ ]);
                Ok(())
            }
        }
    };

    (@body $regs:ident, $value:ident, [ $field:ident, set ]) => {{
        paste::paste! {
            $regs.[<set_ $field>]($value);
        }
    }};

    (@body $regs:ident, $value:ident, [ $field:ident, set_if($check:expr) ]) => {{
        if ($check)($value) {
            paste::paste! {
                $regs.[<set_ $field>]($value);
            }
        } else {
            return Err(Error::with_message(Errno::EIO, "invalid register value"));
        }
    }};

    (@body $regs:ident, $value:ident, [ $field:ident, set_bits_truncate($mask:expr) ]) => {{
        let old_value = $regs.$field();
        const MASK: usize = $mask;
        paste::paste! {
            $regs.[<set_ $field>]((old_value & !MASK) | ($value & MASK));
        }
    }};

    (@body $regs:ident, $value:ident, [ $field:ident, fixed($expected:expr) ]) => {{
        let _ = $regs;
        const EXPECTED: usize = $expected;
        if $value != EXPECTED {
            return Err(Error::with_message(Errno::EIO, "invalid segment selector"));
        }
    }};
}

#[cfg(target_arch = "x86_64")]
ostd::for_all_general_regs!(general_regs_ptrace_setter);

/// Checks whether the given offset is valid for in `struct user`.
//
// Reference: <https://elixir.bootlin.com/linux/v6.16.5/source/arch/x86/include/asm/user_64.h#L103-L132>
#[cfg(target_arch = "x86_64")]
pub(super) fn check_user_offset(offset: usize) -> Result<()> {
    if !offset.is_multiple_of(core::mem::size_of::<usize>()) {
        return_errno_with_message!(Errno::EIO, "invalid USER area offset");
    }

    // We only support the offsets for general-purpose registers currently.
    // `struct user_regs_struct` is the first field in `struct user`.
    if offset >= core::mem::size_of::<c_user_regs_struct>() {
        return_errno_with_message!(
            Errno::EOPNOTSUPP,
            "only offsets for general-purpose registers are supported currently"
        );
    }
    Ok(())
}
