// SPDX-License-Identifier: MPL-2.0

use ostd::cpu::CpuId;

use super::SyscallReturn;
use crate::{prelude::*, thread::Thread};

pub fn sys_sched_yield(_ctx: &Context) -> Result<SyscallReturn> {
    warn!("sys_sched_yield on CPU {:?}", CpuId::current_racy());
    Thread::yield_now_except_idle(); // CPU 3 上真yield了？为啥？
    warn!("sys_sched_yield on CPU {:?} done", CpuId::current_racy());
    Ok(SyscallReturn::Return(0))
}
