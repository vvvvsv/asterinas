// SPDX-License-Identifier: MPL-2.0

use aster_util::printer::VmPrinter;
use ostd::mm::MAX_USERSPACE_VADDR;

use super::TidDirOps;
use crate::{
    fs::{
        procfs::template::{FileOps, ProcFileBuilder},
        utils::{mkmod, Inode},
    },
    prelude::*,
    process::Process,
    vm::{self, vmar::VMAR_LOWEST_ADDR},
};

/// Represents the inode at `/proc/[pid]/task/[tid]/maps` (and also `/proc/[pid]/maps`).
pub struct MapsFileOps(Arc<Process>);

impl MapsFileOps {
    pub fn new_inode(dir: &TidDirOps, parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        let process_ref = dir.process_ref.clone();
        // Reference: <https://elixir.bootlin.com/linux/v6.16.5/source/fs/proc/base.c#L3343>
        ProcFileBuilder::new(Self(process_ref), mkmod!(a+r))
            .parent(parent)
            .build()
            .unwrap()
    }
}

impl FileOps for MapsFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        let mut printer = VmPrinter::new_skip(writer, offset);

        let vmar_guard = self.0.lock_vmar();
        let Some(vmar) = vmar_guard.as_ref() else {
            return_errno_with_message!(Errno::ESRCH, "the process has exited");
        };

        let guard = vmar.query(VMAR_LOWEST_ADDR..MAX_USERSPACE_VADDR);
        for vm_mapping in guard.iter() {
            let init_stack_top = vmar.process_vm().init_stack_top();
            if vm_mapping.map_to_addr() <= init_stack_top && vm_mapping.map_end() >= init_stack_top
            {
                // println!("init_stack_top:{:012x}", init_stack_top);
                writeln!(
                    printer,
                    "{:012x}-{:012x} {:4} {:08x} {:5} {:>5} {:>26}",
                    vm_mapping.map_to_addr(),
                    vm_mapping.map_end(),
                    "rw-p",
                    0,
                    "00:00",
                    0,
                    "[stack]"
                )?;
            }
        }

        Ok(printer.bytes_written())
    }
}
