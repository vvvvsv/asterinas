// SPDX-License-Identifier: MPL-2.0

//! Out-Of-Memory (OOM) Controller.

use ostd::sync::Mutex;

use crate::{
    prelude::*,
    process::{
        process_table,
        signal::{constants::SIGKILL, signals::kernel::KernelSignal},
        Process,
    },
};

static OOM_LOCK: Mutex<()> = Mutex::new(());

/// Calculates the Out-Of-Memory score for a given process.
/// Returns None if the process is unkillable.
///
/// The score is used to determine which process should be killed.
/// Higher scores indicate higher memory usage and higher probability
/// of being killed.
fn oom_score(process: &Arc<Process>) -> Option<usize> {
    if process.is_init_process() {
        return None;
    }
    Some(process.get_rss())
}

/// Handles an Out-Of-Memory situation by selecting and terminating
/// the process with the highest memory usage.
///
/// If we run out of memory, we have the choice between either
/// killing a random task (bad), letting the system crash (worse)
/// OR try to be smart about which process to kill.
pub(super) fn out_of_memory() -> Result<()> {
    if let Some(_gurad) = OOM_LOCK.try_lock() {
        let mut worst_process: Option<&Arc<Process>> = None;
        let mut highest_score = usize::MIN;

        let process_table_mut = process_table::process_table_mut();

        for process in process_table_mut.iter() {
            if let Some(score) = oom_score(process)
                && score > highest_score
            {
                highest_score = score;
                worst_process = Some(process);
            }
        }

        if let Some(process) = worst_process {
            info!("OOM: killing process pid={} with score={}", process.pid(), highest_score);
            process.enqueue_signal(KernelSignal::new(SIGKILL));
            Ok(())
        } else {
            return_errno_with_message!(
                Errno::ENOMEM,
                "The Out-Of-Memory controller failed to select a process to kill."
            );
        }
    } else {
        // Someone is already handling OOM.
        Ok(())
    }
}
