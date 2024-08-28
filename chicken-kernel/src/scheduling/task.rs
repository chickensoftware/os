use core::ptr::NonNull;
use crate::base::interrupts::CpuState;

#[derive(Clone, Debug)]
pub(crate) struct Task {
    pub(super) status: TaskStatus,
    pub(super) context: *const CpuState,
    pub(super) next: Option<NonNull<Task>>,
    pub(super) prev: Option<NonNull<Task>>,
    pub(super) id: u64,
}


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Ready,
    Running,
    Dead
}
