// SPDX-License-Identifier: MPL-2.0

use hashbrown::HashMap;

use super::{
    ExitCode, Pid, Process,
    process_filter::ProcessFilter,
    signal::{constants::SIGCHLD, with_sigmask_changed},
};
use crate::{
    prelude::*,
    process::{
        ReapedChildrenStats, Uid, pid_table,
        posix_thread::{AsPosixThread, PosixThread},
        signal::sig_num::SigNum,
        status::StopWaitStatus,
    },
    thread::{Thread, Tid},
    time::clocks::ProfClock,
};

// The definition of WaitOptions is from Occlum
bitflags! {
    pub struct WaitOptions: u32 {
        const WNOHANG = 0x1;
        const WSTOPPED = 0x2; // Same as WUNTRACED
        const WEXITED = 0x4;
        const WCONTINUED = 0x8;
        const WNOWAIT = 0x01000000;
        const WNOTHREAD = 0x20000000;
        const WALL = 0x40000000;
        const WCLONE = 0x80000000;
    }
}

impl WaitOptions {
    pub fn check(&self) -> Result<()> {
        let supported_args = WaitOptions::WNOHANG
            | WaitOptions::WSTOPPED
            | WaitOptions::WCONTINUED
            | WaitOptions::WNOWAIT;
        if !supported_args.contains(*self) {
            warn!(
                "unsupported wait options are found: {:?}",
                *self - supported_args
            );
        }

        Ok(())
    }
}

pub fn do_wait(
    child_filter: ProcessFilter,
    wait_options: WaitOptions,
    ctx: &Context,
) -> Result<Option<WaitStatus>> {
    wait_options.check()?;

    let is_nonblocking = if let ProcessFilter::WithPidfd(pid_file) = &child_filter {
        pid_file.is_nonblocking()
    } else {
        false
    };

    let zombie_child = with_sigmask_changed(
        ctx,
        |sigmask| sigmask + SIGCHLD,
        || {
            ctx.process.children_wait_queue().pause_until(|| {
                let has_unready_tracee = match try_wait_tracees(&child_filter, wait_options, ctx) {
                    Some(Some(status)) => return Some(Ok(Some(status))),
                    Some(None) => true,
                    None => false,
                };

                let has_unready_child = match try_wait_children(&child_filter, wait_options, ctx) {
                    Some(Some(status)) => return Some(Ok(Some(status))),
                    Some(None) => true,
                    None => false,
                };

                if !has_unready_tracee && !has_unready_child {
                    return Some(Err(Error::with_message(
                        Errno::ECHILD,
                        "the process has no child to wait",
                    )));
                }

                if wait_options.contains(WaitOptions::WNOHANG) {
                    return Some(Ok(None));
                }

                if is_nonblocking {
                    return Some(Err(Error::with_message(
                        Errno::EAGAIN,
                        "the PID file is nonblocking and the child has not terminated",
                    )));
                }

                // wait
                None
            })
        },
    )??;

    Ok(zombie_child)
}

fn wait_filter(child_pid: Pid, child: &Arc<Process>, child_filter: &ProcessFilter) -> bool {
    match &child_filter {
        ProcessFilter::Any => true,
        ProcessFilter::WithPid(pid) => child_pid == *pid,
        ProcessFilter::WithPgid(pgid) => child.pgid() == *pgid,
        ProcessFilter::WithPidfd(pid_file) => match pid_file.process_opt() {
            Some(process) => Arc::ptr_eq(&process, child),
            None => false,
        },
    }
}

pub enum WaitStatus {
    Zombie(Arc<Process>),
    Stop(Arc<Process>, SigNum),
    Continue(Arc<Process>),
    TraceeExit(Arc<Thread>),
    TraceeStop(Arc<Thread>, SigNum),
}

impl WaitStatus {
    pub fn pid(&self) -> Pid {
        match self.source() {
            WaitStatusSource::Process(process) => process.pid(),
            WaitStatusSource::Thread(thread) => thread.tid(),
        }
    }

    pub fn uid(&self) -> Uid {
        match self.source() {
            WaitStatusSource::Process(process) => process
                .main_thread()
                .as_posix_thread()
                .unwrap()
                .credentials()
                .ruid(),
            WaitStatusSource::Thread(thread) => thread.credentials().ruid(),
        }
    }

    pub fn prof_clock(&self) -> &Arc<ProfClock> {
        match self.source() {
            WaitStatusSource::Process(process) => process.prof_clock(),
            WaitStatusSource::Thread(thread) => thread.prof_clock(),
        }
    }

    fn source(&self) -> WaitStatusSource<'_> {
        match self {
            WaitStatus::Zombie(process)
            | WaitStatus::Stop(process, _)
            | WaitStatus::Continue(process) => WaitStatusSource::Process(process.as_ref()),
            WaitStatus::TraceeExit(thread) | WaitStatus::TraceeStop(thread, _) => {
                WaitStatusSource::Thread(thread.as_posix_thread().unwrap())
            }
        }
    }
}

enum WaitStatusSource<'a> {
    Process(&'a Process),
    Thread(&'a PosixThread),
}

/// Checks tracees for exited or ptrace-stopped threads.
///
/// Returns:
/// - `Some(Some(status))` if a tracee status is found,
/// - `Some(None)` if there are tracees matching `child_filter`, but none is ready,
/// - `None` if no tracee matches `child_filter`.
fn try_wait_tracees(
    child_filter: &ProcessFilter,
    wait_options: WaitOptions,
    ctx: &Context,
) -> Option<Option<WaitStatus>> {
    // Currently, we only support the main thread as the tracer,
    // so there is no need to check the tracees of other threads.
    //
    // Lock order: tracer.tracees -> tracee.tracee_status
    let tracees = ctx.posix_thread.tracees()?;
    let tracees = tracees.lock();

    let mut fallback_result = None;

    for thread in tracees.values() {
        let Some(thread) = thread.upgrade() else {
            continue;
        };
        let tracee = thread.as_posix_thread().unwrap();
        let Some(process) = tracee.weak_process().upgrade() else {
            continue;
        };
        if !wait_filter(tracee.tid(), &process, child_filter) {
            continue;
        }

        // We have found at least one tracee matching `child_filter`.
        fallback_result = Some(None);

        if thread.is_exited() {
            if !wait_options.contains(WaitOptions::WNOWAIT) {
                cleanup_exited_tracee(&process, &thread, tracee, tracees, ctx);
            }
            return Some(Some(WaitStatus::TraceeExit(thread)));
        }

        // Waiting for ptrace-stops does not require `WaitOptions::WSTOPPED`.
        if let Some(sig_num) = tracee.wait_ptrace_stopped(wait_options) {
            return Some(Some(WaitStatus::TraceeStop(thread, sig_num)));
        }
    }

    fallback_result
}

fn cleanup_exited_tracee(
    process: &Arc<Process>,
    thread: &Arc<Thread>,
    tracee: &PosixThread,
    mut tracees: MutexGuard<HashMap<Tid, Weak<Thread>>>,
    ctx: &Context,
) {
    tracees.remove(&tracee.tid());
    tracee.detach_tracer();
    drop(tracees);

    // Exit/death by signal is reported first to the tracer, then,
    // when the tracer consumes the waitpid(2) result, to the real
    // parent (to the real parent only when the whole multithreaded
    // process exits). If the tracer and the real parent are the same
    // process, the report is sent only once.
    //
    // Reference: <https://man7.org/linux/man-pages/man2/ptrace.2.html>
    if !process.status().is_zombie() {
        return;
    }
    let is_our_child = core::ptr::eq(
        Weak::as_ptr(process.parent().lock().process()),
        Arc::as_ptr(&ctx.process),
    );
    if !is_our_child {
        return;
    }
    if !Arc::ptr_eq(&process.main_thread(), thread) {
        return;
    }

    reap_zombie_child(
        process.pid(),
        ctx.process.children().lock().as_mut().unwrap(),
        ctx.process.reaped_children_stats(),
    );
}

/// Checks children for zombie or stopped/continued events.
///
/// Returns:
/// - `Some(Some(status))` if a child status is found,
/// - `Some(None)` if there are children matching `child_filter`, but none is ready,
/// - `None` if no child matches `child_filter`.
fn try_wait_children(
    child_filter: &ProcessFilter,
    wait_options: WaitOptions,
    ctx: &Context,
) -> Option<Option<WaitStatus>> {
    // Acquire the children lock at first to prevent race conditions.
    // We want to ensure that multiple waiting threads
    // do not return the same waited process status.
    let mut children_lock = ctx.process.children().lock();
    let children_mut = children_lock.as_mut().unwrap();

    let mut fallback_result = None;

    for child in children_mut.values() {
        if !wait_filter(child.pid(), child, child_filter) {
            continue;
        }

        // We have found at least one child matching `child_filter`.
        fallback_result = Some(None);

        if child.main_thread().as_posix_thread().unwrap().is_traced() {
            continue;
        }

        if child.status().is_zombie() {
            let child = child.clone();
            if !wait_options.contains(WaitOptions::WNOWAIT) {
                reap_zombie_child(
                    child.pid(),
                    children_mut,
                    ctx.process.reaped_children_stats(),
                );
            }
            return Some(Some(WaitStatus::Zombie(child)));
        }

        if !wait_options.intersects(WaitOptions::WSTOPPED | WaitOptions::WCONTINUED) {
            continue;
        }
        let Some(stop_wait_status) = child.wait_stopped_or_continued(wait_options) else {
            continue;
        };
        let wait_status = match stop_wait_status {
            StopWaitStatus::Stopped(sig_num) => WaitStatus::Stop(child.clone(), sig_num),
            StopWaitStatus::Continue => WaitStatus::Continue(child.clone()),
        };
        return Some(Some(wait_status));
    }

    fallback_result
}

/// Free zombie child with `child_pid`, returns the exit code of child process.
fn reap_zombie_child(
    child_pid: Pid,
    children_lock: &mut BTreeMap<Pid, Arc<Process>>,
    reaped_children_stats: &Mutex<ReapedChildrenStats>,
) -> ExitCode {
    let child_process = children_lock.remove(&child_pid).unwrap();
    assert!(child_process.status().is_zombie());

    let mut pid_table = pid_table::pid_table_mut();

    // Lock order: children of process -> PID table -> tasks of process
    for task in child_process.tasks().lock().as_slice() {
        pid_table.remove_thread(task.as_posix_thread().unwrap().tid());
    }

    // Lock order: children of process -> PID table
    // -> group of process -> group inner -> session inner

    // Remove the process from the global table
    pid_table.remove_process(child_process.pid());

    // Remove the process group and the session from global table, if necessary
    let mut child_group_mut = child_process.process_group.lock();
    child_process.clear_old_group_and_session(&mut child_group_mut, &mut pid_table);

    let (mut user_time, mut kernel_time) = child_process.reaped_children_stats().lock().get();
    user_time += child_process.prof_clock().user_clock().read_time();
    kernel_time += child_process.prof_clock().kernel_clock().read_time();
    reaped_children_stats.lock().add(user_time, kernel_time);

    child_process.status().exit_code()
}
