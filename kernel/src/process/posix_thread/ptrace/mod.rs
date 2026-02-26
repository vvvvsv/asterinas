// SPDX-License-Identifier: MPL-2.0

//! Ptrace implementation for POSIX threads.

use hashbrown::HashMap;

use super::{AsPosixThread, PosixThread};
use crate::{
    prelude::*,
    thread::{Thread, Tid},
};

impl PosixThread {
    /// Returns the tracer of this thread if it is being traced.
    pub fn tracer(&self) -> Option<Arc<Thread>> {
        self.tracee_status.get().and_then(|status| status.tracer())
    }

    /// Sets the tracer of this thread.
    ///
    /// # Errors
    ///
    /// Returns `EPERM` if this thread is already being traced.
    fn set_tracer(&self, tracer: Weak<Thread>) -> Result<()> {
        let status = self.tracee_status.call_once(TraceeStatus::new);
        status.set_tracer(tracer)
    }

    /// Detaches the tracer of this thread.
    pub(in crate::process) fn detach_tracer(&self) {
        if let Some(status) = self.tracee_status.get() {
            status.detach_tracer();
        }
    }
}

impl PosixThread {
    /// Attaches a tracee to this tracer.
    ///
    /// # Errors
    ///
    /// Returns `EPERM` if the tracee is already being traced.
    ///
    /// # Panics
    ///
    /// Panics if `tracer_thread` and `self` do not point to the same thread,
    /// or if `tracee_thread` is not a POSIX thread.
    pub fn attach_tracee(
        &self,
        tracer_thread: &Arc<Thread>,
        tracee_thread: &Arc<Thread>,
    ) -> Result<()> {
        debug_assert!(core::ptr::eq(
            tracer_thread.as_posix_thread().unwrap(),
            self
        ));

        let tracees = self.tracees.call_once(|| Mutex::new(HashMap::new()));

        // Lock order: tracer.tracees -> tracee.tracee_status
        let mut tracees = tracees.lock();
        if tracer_thread.is_exited() {
            // Pretend that the tracer immediately detaches the tracee,
            // if the tracer has already exited.
            // Reference: <https://elixir.bootlin.com/linux/v6.16.5/source/kernel/ptrace.c#L496-L498>
            return Ok(());
        }

        let tracee = tracee_thread.as_posix_thread().unwrap();
        tracee.set_tracer(Arc::downgrade(tracer_thread))?;
        tracees.insert(tracee.tid(), Arc::downgrade(tracee_thread));

        Ok(())
    }

    /// Returns the tracee map of this thread if it is a tracer.
    pub(super) fn tracees(&self) -> Option<&Mutex<HashMap<Tid, Weak<Thread>>>> {
        self.tracees.get()
    }

    /// Clears all tracees of this tracer on exit.
    pub(in crate::process) fn clear_tracees(&self) {
        let Some(tracees) = self.tracees() else {
            return;
        };

        // Lock order: tracer.tracees -> tracee.tracee_status
        let tracees = tracees.lock();
        for (_, tracee) in tracees.iter() {
            let Some(tracee) = tracee.upgrade() else {
                continue;
            };
            let tracee = tracee.as_posix_thread().unwrap();
            tracee.detach_tracer();
        }
    }

    /// Updates the TID used to index a tracee after `execve` promotes it to the leader.
    ///
    /// # Panics
    ///
    /// This method panics in debug builds if `old_tid` and `new_tid` are the same.
    pub(in crate::process) fn update_tracee_tid(&self, old_tid: Tid, new_tid: Tid) {
        debug_assert_ne!(old_tid, new_tid);
        let Some(tracees) = self.tracees() else {
            return;
        };

        let mut tracees = tracees.lock();
        if let Some(tracee) = tracees.remove(&old_tid) {
            tracees.insert(new_tid, tracee);
        };
    }
}

pub(super) struct TraceeStatus {
    state: Mutex<TraceeState>,
}

impl TraceeStatus {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(TraceeState::new()),
        }
    }

    fn tracer(&self) -> Option<Arc<Thread>> {
        self.state.lock().tracer()
    }

    fn set_tracer(&self, tracer: Weak<Thread>) -> Result<()> {
        let mut state = self.state.lock();
        if state.tracer().is_some() {
            return_errno_with_message!(Errno::EPERM, "the thread is already being traced");
        }
        state.tracer = tracer;

        Ok(())
    }

    fn detach_tracer(&self) {
        self.state.lock().tracer = Weak::new();
    }
}

struct TraceeState {
    tracer: Weak<Thread>,
}

impl TraceeState {
    fn new() -> Self {
        Self {
            tracer: Weak::new(),
        }
    }

    fn tracer(&self) -> Option<Arc<Thread>> {
        self.tracer.upgrade()
    }
}
