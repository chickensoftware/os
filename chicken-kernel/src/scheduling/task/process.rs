use alloc::{boxed::Box, string::String};
use core::{ptr, ptr::NonNull};

use chicken_util::{
    memory::{paging::PageTable, VirtualAddress},
    PAGE_SIZE,
};

use crate::{
    base::{
        gdt::{KERNEL_CS, KERNEL_DS},
        interrupts::{CpuState, RFlags},
    },
    memory::{
        paging::{PagingError, PTM},
        vmm::{AllocationType, object::VmFlags, VMM, VmmError},
    },
    scheduling::SchedulerError,
};

/// Size of stack for new processes.
const PROCESS_STACK_SIZE: usize = PAGE_SIZE * 4;

#[derive(Debug)]
pub(crate) struct Process {
    pub(in crate::scheduling) status: TaskStatus,
    pub(in crate::scheduling) context: *const CpuState,
    pub(in crate::scheduling) pid: u64,
    pub(in crate::scheduling) page_table_mappings: *const PageTable,
    pub(in crate::scheduling) name: Option<String>,
    pub(in crate::scheduling) next: Option<NonNull<Process>>,
    pub(in crate::scheduling) prev: Option<NonNull<Process>>,
}

impl Process {
    // todo: maybe add arguments to entry function signature
    /// Allocates memory on the heap for new process and initializes it. Returns the new task or an error code if the initialization failed.
    pub(in crate::scheduling) fn create(
        name: String,
        entry: fn(),
        pid: u64,
    ) -> Result<Option<NonNull<Self>>, SchedulerError> {
        // set up new cpu state
        let rsp = allocate_stack()?;
        let cpu_state = Box::into_raw(Box::new(CpuState::basic(
            KERNEL_DS as u64,
            rsp,
            RFlags::RESERVED_1 | RFlags::INTERRUPTS_ENABLED,
            KERNEL_CS as u64,
            entry as usize as u64,
            0,
        )));
        // set up new page table mappings
        let pml4 = allocate_page_mappings()?;
        // initialize new process
        let default = Process::empty();
        let process = NonNull::new(Box::into_raw(Box::new(default)));

        let process_ref = unsafe { process.unwrap().as_mut() };

        process_ref.name = Some(name);
        process_ref.pid = pid;
        process_ref.status = TaskStatus::Ready;
        process_ref.page_table_mappings = pml4;
        process_ref.context = cpu_state;
        Ok(process)
    }

    fn empty() -> Self {
        Self {
            status: TaskStatus::Dead,
            context: ptr::null_mut(),
            next: None,
            prev: None,
            pid: 0,
            page_table_mappings: ptr::null_mut(),
            name: None,
        }
    }
}

/// Allocate a stack of [`PROCESS_STACK_SIZE`] for a new process. Returns the pointer to the top of the stack or an error value. The caller is responsible fpr freeing the memory allocated.
fn allocate_stack() -> Result<VirtualAddress, SchedulerError> {
    let mut binding = VMM.lock();
    if let Some(vmm) = binding.get_mut() {
        let stack_bottom = vmm
            .alloc(PROCESS_STACK_SIZE, VmFlags::WRITE, AllocationType::AnyPages)
            .map_err(SchedulerError::from)?;
        Ok(stack_bottom + PROCESS_STACK_SIZE as u64 - 1)
    } else {
        Err(SchedulerError::MemoryAllocationError(
            VmmError::GlobalVirtualMemoryManagerUninitialized,
        ))
    }
}
/// Allocate new page table mappings. Copies the higher half mappings from the global page table manager. Returns the address to the new pml4 table or an error value. The caller is responsible fpr freeing the memory allocated.
fn allocate_page_mappings() -> Result<*const PageTable, SchedulerError> {
    // get page table size
    let current_mapping = {
        let mut binding = PTM.lock();
        if let Some(ptm) = binding.get_mut() {
            Ok(unsafe { ptm.pml4_virtual().read().entries })
        } else {
            Err(SchedulerError::PageTableManagerError(
                PagingError::GlobalPageTableManagerUninitialized,
            ))
        }
    }?;

    let mut binding = VMM.lock();
    if let Some(vmm) = binding.get_mut() {
        let new_pml4_address = vmm.alloc(PAGE_SIZE, VmFlags::WRITE, AllocationType::AnyPages)?;
        let new_pml4 = unsafe { (new_pml4_address as *mut PageTable).as_mut().unwrap() };

        new_pml4.entries[256..512].copy_from_slice(&current_mapping[256..512]);

        Ok(new_pml4_address as *const PageTable)
    } else {
        Err(SchedulerError::MemoryAllocationError(
            VmmError::GlobalVirtualMemoryManagerUninitialized,
        ))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Ready,
    Running,
    Dead,
}
