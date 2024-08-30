use alloc::{
    alloc::dealloc,
    boxed::Box,
    format,
    string::{String, ToString},
};
use core::{alloc::Layout, ptr, ptr::NonNull};

use qemu_print::qemu_println;

use chicken_util::{memory::paging::PageTable, PAGE_SIZE};

use crate::{
    memory::{
        paging::{PagingError, PTM},
        vmm::{AllocationType, object::VmFlags, VMM, VmmError},
    },
    scheduling::{SchedulerError, task::thread::Thread},
};

const MAIN_THREAD_NAME: &str = "MAIN-";
#[derive(Debug)]
pub(crate) struct Process {
    pub(in crate::scheduling) page_table_mappings: *const PageTable,

    pub(in crate::scheduling) thread_id_counter: u64,
    pub(in crate::scheduling) main_thread: Option<NonNull<Thread>>,
    pub(in crate::scheduling) active_thread: Option<NonNull<Thread>>,

    pub(in crate::scheduling) pid: u64,
    pub(in crate::scheduling) status: TaskStatus,
    pub(in crate::scheduling) name: String,

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
        // set up new page table mappings
        let pml4 = allocate_page_mappings()?;

        // initialize new process
        let default = Process::empty();
        let process = NonNull::new(Box::into_raw(Box::new(default)));
        let process_ref = unsafe { process.unwrap().as_mut() };

        process_ref.name = name;
        process_ref.pid = pid;
        process_ref.status = TaskStatus::Ready;
        process_ref.page_table_mappings = pml4;

        // set up main thread
        process_ref.add_thread(format!("{}{}", MAIN_THREAD_NAME, pid), entry)?;

        Ok(process)
    }

    fn empty() -> Self {
        Self {
            status: TaskStatus::Dead,
            next: None,
            prev: None,
            pid: 0,
            page_table_mappings: ptr::null_mut(),
            thread_id_counter: 0,
            active_thread: None,
            name: "".to_string(),
            main_thread: None,
        }
    }
}

impl Process {
    pub(crate) fn active_thread(&mut self) -> &mut Thread {
        assert!(
            self.active_thread.is_some(),
            "Each task must have an active thread."
        );
        unsafe { self.active_thread.unwrap().as_mut() }
    }
    pub(crate) fn add_thread(&mut self, name: String, entry: fn()) -> Result<(), SchedulerError> {
        let mut current = self.main_thread;

        // every thread ever created has a unique ID
        self.thread_id_counter += 1;

        // main thread initialization
        if current.is_none() {
            let thread_ptr = Thread::create(name, entry, self.thread_id_counter, self.pid)?;
            self.main_thread = thread_ptr;
            self.active_thread = self.main_thread;
            return Ok(());
        }

        while let Some(mut current_thread) = current {
            let current_thread = unsafe { current_thread.as_mut() };
            // append at the end of the list
            if current_thread.next.is_none() {
                let thread_ptr = Thread::create(name, entry, self.thread_id_counter, self.pid)?;
                let thread = unsafe { thread_ptr.unwrap().as_mut() };
                thread.prev = current;

                current_thread.next = thread_ptr;
                return Ok(());
            }
            current = current_thread.next;
        }
        qemu_println!("added thread: {}", name);

        Ok(())
    }

    /// Removes the specified thread from the list. Returns whether the action succeeds. The thread to be removed must not be the currently active.
    pub(in crate::scheduling) fn remove_thread(
        &mut self,
        tid: u64,
        force: bool,
    ) -> Result<(), SchedulerError> {
        let active_thread = self.active_thread;
        assert!(active_thread.is_some(), "Active thread must be present.");
        if !force {
            assert_ne!(
                unsafe { active_thread.unwrap().as_ref().tid },
                tid,
                "Active thread must not be removed while still active."
            );
        }

        let mut current = self.main_thread;

        while let Some(mut current_thread) = current {
            let current_ref = unsafe { current_thread.as_mut() };

            if current_ref.tid == tid {
                // remove thread from linked list
                let heap_ptr = if let Some(mut prev) = current_ref.prev {
                    let prev_ref = unsafe { prev.as_mut() };
                    let heap_ptr = prev_ref.next.unwrap().as_ptr();
                    prev_ref.next = current_ref.next;
                    heap_ptr
                } else {
                    let heap_ptr = self.main_thread.unwrap().as_ptr();
                    self.main_thread = current_ref.next;

                    heap_ptr
                };

                if let Some(mut next) = current_ref.next {
                    let next_ref = unsafe { next.as_mut() };
                    next_ref.prev = current_ref.prev;
                }

                // deallocate thread
                unsafe {
                    dealloc(heap_ptr as *mut u8, Layout::new::<Thread>());
                }

                let mut binding = VMM.lock();
                let vmm = binding
                    .get_mut()
                    .ok_or(SchedulerError::MemoryAllocationError(
                        VmmError::GlobalVirtualMemoryManagerUninitialized,
                    ))?;

                // free thread's stack
                let stack_address = current_ref.stack_start;
                vmm.free(stack_address).map_err(SchedulerError::from)?;

                return Ok(());
            }
            current = current_ref.next;
        }

        Err(SchedulerError::ThreadNotFound(self.pid, tid))
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
