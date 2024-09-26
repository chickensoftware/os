use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::{ptr, ptr::NonNull};

use chicken_util::{memory::VirtualAddress, PAGE_SIZE};

use crate::{
    base::{
        gdt::{KERNEL_CS, KERNEL_DS, USER_CS, USER_DS},
        interrupts::{CpuState, RFlags},
    },
    memory::vmm::{object::VmFlags, AllocationType, VmmError, VMM},
    scheduling::SchedulerError,
};

/// Size of stack for new threads.
const THREAD_STACK_SIZE: usize = PAGE_SIZE * 4;

#[derive(Debug)]
pub(crate) struct Thread {
    pub(in crate::scheduling) context: *const CpuState,
    pub(in crate::scheduling) stack_start: VirtualAddress,

    pub(in crate::scheduling) tid: u64,
    pub(in crate::scheduling) pid: u64,
    pub(in crate::scheduling) status: ThreadStatus,
    pub(in crate::scheduling) name: String,
    pub(in crate::scheduling) user: bool,

    pub(in crate::scheduling) joins: Option<Vec<u64>>,

    pub(in crate::scheduling) next: Option<NonNull<Thread>>,
    pub(in crate::scheduling) prev: Option<NonNull<Thread>>,
}

impl Thread {
    pub(crate) fn create(
        name: String,
        entry: fn(),
        tid: u64,
        pid: u64,
        user: bool,
    ) -> Result<Option<NonNull<Thread>>, SchedulerError> {
        // set up new cpu state
        let (stack_start, rsp) = allocate_stack(user)?;

        let (cs, ds) = if user {
            // (KERNEL_CS, KERNEL_DS)
            (USER_CS, USER_DS)
        } else {
            (KERNEL_CS, KERNEL_DS)
        };

        let cpu_state = Box::into_raw(Box::new(CpuState::basic(
            ds as u64,
            rsp,
            RFlags::RESERVED_1 | RFlags::INTERRUPTS_ENABLED,
            cs as u64,
            entry as usize as u64,
            0,
        )));

        // initialize new thread
        let default = Thread::empty();
        let thread = NonNull::new(Box::into_raw(Box::new(default)));

        let thread_ref = unsafe { thread.unwrap().as_mut() };

        thread_ref.context = cpu_state;
        thread_ref.stack_start = stack_start;

        thread_ref.tid = tid;
        thread_ref.pid = pid;
        thread_ref.name = name;
        thread_ref.status = ThreadStatus::Ready;
        thread_ref.user = user;

        Ok(thread)
    }

    fn empty() -> Self {
        Self {
            context: ptr::null_mut(),
            stack_start: 0,
            tid: 0,
            pid: 0,
            status: ThreadStatus::Dead,
            name: "".to_string(),
            next: None,
            prev: None,
            joins: None,
            user: false,
        }
    }
}

/// Allocate a stack of [`THREAD_STACK_SIZE`] for a new process. Returns the pointer to the stack bottom and the top of the stack or an error value. The caller is responsible fpr freeing the memory allocated.
fn allocate_stack(user: bool) -> Result<(VirtualAddress, VirtualAddress), SchedulerError> {
    let flags = if user {
        VmFlags::WRITE | VmFlags::USER
    } else {
        VmFlags::WRITE
    };

    let mut binding = VMM.lock();
    if let Some(vmm) = binding.get_mut() {
        let stack_bottom = vmm
            .alloc(THREAD_STACK_SIZE + 1, flags, AllocationType::AnyPages)
            .map_err(SchedulerError::from)?;
        Ok((stack_bottom, stack_bottom + THREAD_STACK_SIZE as u64))
    } else {
        Err(SchedulerError::MemoryAllocationError(
            VmmError::GlobalVirtualMemoryManagerUninitialized,
        ))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ThreadStatus {
    Ready,
    Running,
    Dead,
    Sleep(u64),
}
