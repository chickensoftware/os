use alloc::string::String;

use chicken_util::memory::VirtualAddress;

use crate::{
    base::interrupts::without_interrupts,
    scheduling::{SchedulerError, SCHEDULER},
};

pub(crate) mod process;
pub(crate) mod thread;

#[derive(Debug)]
pub(crate) struct JoinHandle {
    tid: u64,
}

impl JoinHandle {
    fn try_new(tid: Result<u64, SchedulerError>) -> Result<JoinHandle, SchedulerError> {
        let tid = tid?;
        Ok(JoinHandle { tid })
    }

    pub(in crate::scheduling) fn into_inner(self) -> u64 {
        self.tid
    }
}

/// Spawns a new thread to the current process.
/// todo: Automate adding of [`crate::scheduling::GlobalTaskScheduler::kill_active`]
pub(crate) fn spawn_thread(
    entry: fn(),
    name: Option<String>,
) -> Result<JoinHandle, SchedulerError> {
    without_interrupts(|| -> Result<JoinHandle, SchedulerError> {
        let mut scheduler = SCHEDULER.lock();
        assert!(
            scheduler.get_mut().is_some(),
            "Tasks can only be spawned after global task scheduler has been initialized."
        );
        let scheduler = scheduler.get_mut().unwrap();
        assert!(
            scheduler.active_task.is_some(),
            "Scheduler must have at least one active task (IDLE)"
        );
        let active = unsafe { scheduler.active_task.unwrap().as_mut() };
        JoinHandle::try_new(active.add_thread(name, entry))
    })
}

/// Spawns a new process.
pub(crate) fn spawn_process(entry: TaskEntry, name: Option<String>) -> Result<(), SchedulerError> {
    without_interrupts(|| -> Result<(), SchedulerError> {
        let mut scheduler = SCHEDULER.lock();
        assert!(
            scheduler.get_mut().is_some(),
            "Tasks can only be spawned after global task scheduler has been initialized."
        );
        let scheduler = scheduler.get_mut().unwrap();
        scheduler.add_task(name, entry)
    })
}
#[derive(Debug)]
pub(crate) struct ProgramData {
    pub(crate) virt_start: VirtualAddress,
    pub(crate) virt_end: VirtualAddress,
}

#[derive(Debug)]
pub(crate) enum TaskEntry {
    Kernel(fn()),
    User(ProgramData),
}
