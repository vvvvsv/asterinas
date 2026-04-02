// SPDX-License-Identifier: MPL-2.0

use super::SyscallReturn;
use crate::{prelude::*, process::posix_thread::Personality};

pub fn sys_personality(personality: u32, ctx: &Context) -> Result<SyscallReturn> {
    let old_personality = ctx.posix_thread.personality();
    if personality == GET_PERSONALITY {
        return Ok(SyscallReturn::Return(old_personality.bits() as _));
    }

    let personality = Personality::try_from(personality)?;
    ctx.posix_thread.set_personality(personality);

    Ok(SyscallReturn::Return(old_personality.bits() as _))
}

const GET_PERSONALITY: u32 = 0xffffffff;
