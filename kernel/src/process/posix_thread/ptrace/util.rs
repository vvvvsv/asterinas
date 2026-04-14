// SPDX-License-Identifier: MPL-2.0

//! Ptrace utilities for POSIX threads.

#[cfg(target_arch = "x86_64")]
use ostd::{
    arch::cpu::context::{DR6_RESERVED, DebugRegs, GeneralRegs, c_user_regs_struct},
    mm::MAX_USERSPACE_VADDR,
};

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

#[cfg(target_arch = "x86_64")]
pub(super) enum UserArea {
    GeneralRegs(usize),
    DebugRegs(usize),
}

#[cfg(target_arch = "x86_64")]
#[repr(C)]
struct c_user_i387_struct {
    cwd: u16,
    swd: u16,
    twd: u16,
    fop: u16,
    rip: u64,
    rdp: u64,
    mxcsr: u32,
    mxcsr_mask: u32,
    st_space: [u32; 32],
    xmm_space: [u32; 64],
    padding: [u32; 24],
}

#[cfg(target_arch = "x86_64")]
#[repr(C)]
struct c_user_struct {
    regs: c_user_regs_struct,
    u_fpvalid: i32,
    pad0: i32,
    i387: c_user_i387_struct,
    u_tsize: usize,
    u_dsize: usize,
    u_ssize: usize,
    start_code: usize,
    start_stack: usize,
    signal: isize,
    reserved: i32,
    pad1: i32,
    u_ar0: usize,
    u_fpstate: usize,
    magic: usize,
    u_comm: [u8; 32],
    u_debugreg: [usize; 8],
    error_code: usize,
    fault_address: usize,
}

#[cfg(target_arch = "x86_64")]
const USER_DEBUGREG_OFFSET: usize = core::mem::offset_of!(c_user_struct, u_debugreg);
#[cfg(target_arch = "x86_64")]
const USER_DEBUGREG_SIZE: usize = size_of::<[usize; 8]>();

/// Parses the given word offset in `struct user`.
//
// Reference: <https://elixir.bootlin.com/linux/v6.16.5/source/arch/x86/include/asm/user_64.h#L103-L132>
#[cfg(target_arch = "x86_64")]
pub(super) fn parse_user_offset(offset: usize) -> Result<UserArea> {
    if !offset.is_multiple_of(size_of::<usize>()) {
        return_errno_with_message!(Errno::EIO, "invalid USER area offset");
    }

    if offset >= size_of::<c_user_regs_struct>() {
        let debugreg_end = USER_DEBUGREG_OFFSET + USER_DEBUGREG_SIZE;
        if (USER_DEBUGREG_OFFSET..debugreg_end).contains(&offset) {
            return Ok(UserArea::DebugRegs(
                (offset - USER_DEBUGREG_OFFSET) / size_of::<usize>(),
            ));
        }

        return_errno_with_message!(Errno::EIO, "unsupported USER area offset");
    }

    Ok(UserArea::GeneralRegs(offset))
}

#[cfg(target_arch = "x86_64")]
pub(super) fn peek_debug_reg(debug_regs: &DebugRegs, reg_num: usize) -> Result<usize> {
    match reg_num {
        0..=3 | 6 | 7 => Ok(debug_regs.reg(reg_num)),
        4 | 5 => Ok(0),
        _ => unreachable!("invalid x86 debug register index"),
    }
}

#[cfg(target_arch = "x86_64")]
pub(super) fn poke_debug_reg(
    debug_regs: &mut DebugRegs,
    reg_num: usize,
    value: usize,
) -> Result<()> {
    match reg_num {
        0..=3 => {
            if value >= MAX_USERSPACE_VADDR {
                return_errno_with_message!(Errno::EINVAL, "invalid debug register address");
            }
            debug_regs.set_reg(reg_num, value);
        }
        4 | 5 => return_errno_with_message!(Errno::EIO, "DR4 and DR5 do not exist on x86_64"),
        6 => debug_regs.set_reg(reg_num, value | DR6_RESERVED),
        7 => debug_regs.set_reg(reg_num, normalize_dr7(debug_regs, value)?),
        _ => unreachable!("invalid x86 debug register index"),
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn normalize_dr7(debug_regs: &DebugRegs, value: usize) -> Result<usize> {
    const DR_CONTROL_RESERVED: usize = 0xFFFF_FFFF_0000_FC00;
    const DR_ENABLE_SIZE: usize = 2;
    const DR_CONTROL_SHIFT: usize = 16;
    const DR_CONTROL_SIZE: usize = 4;
    const DR_LEN_MASK: usize = 0xC;
    const DR_RW_MASK: usize = 0x3;
    const DR_LEN_1: usize = 0x0;
    const DR_LEN_2: usize = 0x4;
    const DR_LEN_4: usize = 0xC;
    const DR_LEN_8: usize = 0x8;
    const DR_RW_EXECUTE: usize = 0x0;
    const DR_RW_WRITE: usize = 0x1;
    const DR_RW_READ_WRITE: usize = 0x3;
    let normalized = value & !DR_CONTROL_RESERVED;

    for reg_num in 0..4 {
        let enabled = ((normalized >> (reg_num * DR_ENABLE_SIZE)) & 0x3) != 0;
        if !enabled {
            continue;
        }

        let control = (normalized >> (DR_CONTROL_SHIFT + reg_num * DR_CONTROL_SIZE)) & 0xF;
        let rw = control & DR_RW_MASK;
        let len = control & DR_LEN_MASK;

        match rw {
            DR_RW_EXECUTE => {
                if len != DR_LEN_1 {
                    return_errno_with_message!(
                        Errno::EINVAL,
                        "instruction breakpoints must use length 1 encoding"
                    );
                }
            }
            DR_RW_WRITE | DR_RW_READ_WRITE => {
                let align_mask = match len {
                    DR_LEN_1 => 0,
                    DR_LEN_2 => 1,
                    DR_LEN_4 => 3,
                    DR_LEN_8 => 7,
                    _ => {
                        return_errno_with_message!(
                            Errno::EINVAL,
                            "invalid x86 hardware breakpoint length"
                        );
                    }
                };
                let addr = debug_regs.reg(reg_num);
                if addr & align_mask != 0 {
                    return_errno_with_message!(
                        Errno::EINVAL,
                        "misaligned x86 hardware breakpoint address"
                    );
                }
            }
            _ => {
                return_errno_with_message!(Errno::EINVAL, "invalid x86 hardware breakpoint type")
            }
        }
    }

    Ok(normalized)
}
