// SPDX-License-Identifier: MPL-2.0

//! Ptrace utilities for POSIX threads.

#[cfg(target_arch = "x86_64")]
use ostd::arch::cpu::context::{GeneralRegs, c_user_regs_struct};

use crate::{
    prelude::*,
    process::{
        ExitCode, WaitOptions,
        signal::{c_types::siginfo_t, constants::SIGTRAP, sig_num::SigNum, signals::Signal},
    },
    thread::Tid,
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
    pub(super) fn get(&self) -> Option<&dyn Signal> {
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

bitflags! {
    /// Options accepted by `PTRACE_SETOPTIONS`.
    pub struct PtraceOptions: usize {
        /// Marks syscall stops with signal number set to `SIGTRAP | 0x80`.
        const PTRACE_O_TRACESYSGOOD = 1;
        /// Stops the tracee at `fork` and automatically traces the new thread.
        const PTRACE_O_TRACEFORK = 1 << PtraceEvent::Fork(0).code();
        /// Stops the tracee at `vfork` and automatically traces the new thread.
        const PTRACE_O_TRACEVFORK = 1 << PtraceEvent::Vfork(0).code();
        /// Stops the tracee at `clone` and automatically traces the new thread.
        const PTRACE_O_TRACECLONE = 1 << PtraceEvent::Clone(0).code();
        /// Stops the tracee at `execve`.
        const PTRACE_O_TRACEEXEC = 1 << PtraceEvent::Exec(0).code();
        /// Stops the tracee at the completion of `vfork`.
        const PTRACE_O_TRACEVFORKDONE = 1 << PtraceEvent::VforkDone(0).code();
        /// Stops the tracee at `exit`.
        const PTRACE_O_TRACEEXIT = 1 << PtraceEvent::Exit(0).code();
        /// Send a `SIGKILL` signal to the tracee if the tracer exits.
        const PTRACE_O_EXITKILL = 1 << 20;
    }
}

/// The ptrace-stop events.
#[derive(Debug, Clone)]
pub enum PtraceEvent {
    /// A `fork` event stop with the new child thread ID.
    Fork(Tid),
    /// A `vfork` event stop with the new child thread ID.
    Vfork(Tid),
    /// A `clone` event stop with the new child thread ID.
    Clone(Tid),
    /// An `execve` event stop with the former thread ID.
    Exec(Tid),
    /// A done `vfork` event stop with the child thread ID.
    VforkDone(Tid),
    /// An `exit` event stop with the tracee's exit code.
    Exit(ExitCode),
}

impl PtraceEvent {
    /// Returns the Linux `PTRACE_EVENT_*` code of this event.
    const fn code(&self) -> u32 {
        match self {
            Self::Fork(_) => 1,
            Self::Vfork(_) => 2,
            Self::Clone(_) => 3,
            Self::Exec(_) => 4,
            Self::VforkDone(_) => 5,
            Self::Exit(_) => 6,
        }
    }

    /// Returns whether the given code is a Linux `PTRACE_EVENT_*` code.
    const fn is_code(code: i32) -> bool {
        matches!(code, 1..=6)
    }

    /// Returns the `PtraceOptions` corresponding to this event.
    pub(super) const fn option(&self) -> PtraceOptions {
        PtraceOptions::from_bits(1 << self.code()).unwrap()
    }

    /// Returns the message of this event.
    pub(super) const fn message(&self) -> usize {
        match self {
            Self::Fork(tid)
            | Self::Vfork(tid)
            | Self::Clone(tid)
            | Self::Exec(tid)
            | Self::VforkDone(tid) => *tid as usize,
            Self::Exit(exit_code) => *exit_code as usize,
        }
    }

    /// Creates a `siginfo_t` for the ptrace-stop triggered by this event.
    pub(super) fn siginfo(&self, ctx: &Context) -> siginfo_t {
        let code = SIGTRAP.as_u8() as i32 | ((self.code() as i32) << 8);
        let mut siginfo = siginfo_t::new(SIGTRAP, code);
        siginfo.set_pid_uid(
            ctx.posix_thread.tid(),
            ctx.posix_thread.credentials().ruid(),
        );
        siginfo
    }
}

/// The `si_status` code of a ptrace-stop for `wait` syscalls.
pub type PtraceWaitStatus = i32;

impl From<siginfo_t> for PtraceWaitStatus {
    fn from(siginfo: siginfo_t) -> Self {
        let is_ptrace_event = siginfo.si_code & 0xff == SIGTRAP.as_u8() as i32
            && PtraceEvent::is_code(siginfo.si_code >> 8);

        if is_ptrace_event {
            siginfo.si_code
        } else {
            siginfo.si_signo
        }
    }
}
