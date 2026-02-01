// SPDX-License-Identifier: MPL-2.0


use super::SyscallReturn;
use crate::{prelude::*, process::{posix_thread::AsPosixThread}};
use ostd::user::UserContextApi;

pub fn sys_test_ptrace(pid: u32, request: u32, ctx: &Context) -> Result<SyscallReturn> {
    let children = ctx.process.children().lock();

    let process = children
        .as_ref()
        .and_then(|children_map| children_map.get(&pid))
        .cloned()
        .unwrap();

    let thread = process.main_thread();
    let posix_thread = thread.as_posix_thread().unwrap();
    let mut user_ctx = posix_thread.user_ctx().lock();

    if request == 0 {
        let rip = user_ctx.instruction_pointer();
        user_ctx.set_instruction_pointer(rip - 1);
        user_ctx.set_tf();
    } else if request == 1 {
        user_ctx.unset_tf();
    } else {
        return_errno!(Errno::EINVAL);
    }

    Ok(SyscallReturn::Return(0))
}