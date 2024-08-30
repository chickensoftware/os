use alloc::boxed::Box;
use alloc::string::{String, ToString};
use core::ptr;
use core::ptr::NonNull;

use chicken_util::memory::VirtualAddress;
use chicken_util::PAGE_SIZE;

use crate::base::gdt::{KERNEL_CS, KERNEL_DS};
use crate::base::interrupts::{CpuState, RFlags};
use crate::memory::vmm::{AllocationType, VMM, VmmError};
use crate::memory::vmm::object::VmFlags;
use crate::scheduling::SchedulerError;
use crate::scheduling::task::process::TaskStatus;

/// Size of stack for new threads.
const THREAD_STACK_SIZE: usize = PAGE_SIZE * 4;

#[derive(Debug)]
pub(crate) struct Thread {
    pub(in crate::scheduling) context: *const CpuState,
    pub(in crate::scheduling) stack_start: VirtualAddress,

    pub(in crate::scheduling) tid: u64,
    pub(in crate::scheduling) pid: u64,
    pub(in crate::scheduling) status: TaskStatus,
    pub(in crate::scheduling) name: String,

    pub(in crate::scheduling) next: Option<NonNull<Thread>>,
    pub(in crate::scheduling) prev: Option<NonNull<Thread>>,
}

impl Thread {
    pub(crate) fn create(name: String, entry: fn(), tid: u64, pid: u64) -> Result<Option<NonNull<Thread>>, SchedulerError>{
        // set up new cpu state
        let (stack_start, rsp) = allocate_stack()?;
        let cpu_state = Box::into_raw(Box::new(CpuState::basic(
            KERNEL_DS as u64,
            rsp,
            RFlags::RESERVED_1 | RFlags::INTERRUPTS_ENABLED,
            KERNEL_CS as u64,
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
        thread_ref.status = TaskStatus::Ready;

        Ok(thread)
    }

    fn empty() -> Self {
        Self {
            context: ptr::null_mut(),
            stack_start: 0,
            tid: 0,
            pid: 0,
            status: TaskStatus::Dead,
            name: "".to_string(),
            next: None,
            prev: None,
        }
    }
}


/// Allocate a stack of [`THREAD_STACK_SIZE`] for a new process. Returns the pointer to the stack bottom and the top of the stack or an error value. The caller is responsible fpr freeing the memory allocated.
fn allocate_stack() -> Result<(VirtualAddress, VirtualAddress), SchedulerError> {
    let mut binding = VMM.lock();
    if let Some(vmm) = binding.get_mut() {
        let stack_bottom = vmm
            .alloc(THREAD_STACK_SIZE, VmFlags::WRITE, AllocationType::AnyPages)
            .map_err(SchedulerError::from)?;
        Ok((stack_bottom, stack_bottom + THREAD_STACK_SIZE as u64 - 1))
    } else {
        Err(SchedulerError::MemoryAllocationError(
            VmmError::GlobalVirtualMemoryManagerUninitialized,
        ))
    }
}