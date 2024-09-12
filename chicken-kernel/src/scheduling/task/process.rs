use alloc::{
    alloc::dealloc,
    boxed::Box,
    format,
    string::{String, ToString},
};
use core::{alloc::Layout, ptr, ptr::NonNull};

use chicken_util::{
    memory::{
        paging::{
            index::PageMapIndexer,
            manager::{OwnedPageTableManager, PageTableManager},
            PageEntryFlags, PageTable,
        },
        VirtualAddress,
    },
    PAGE_SIZE,
};

use super::TaskEntry;
use crate::{
    memory::{
        paging::{PagingError, PTM},
        vmm::{object::VmFlags, AllocationType, VmmError, VMM},
    },
    print, println,
    scheduling::{
        task::thread::{Thread, ThreadStatus},
        SchedulerError,
    },
};

const MAIN_THREAD_NAME: &str = "MAIN-";
#[derive(Debug)]
pub(crate) struct Process {
    pub(in crate::scheduling) page_table_mappings_virtual: *const PageTable,
    // whether the kernel page mappings should be copied when switching from one process to another. For now always true.
    pub(in crate::scheduling) update_kernel_mappings: bool,

    pub(in crate::scheduling) thread_id_counter: u64,
    pub(in crate::scheduling) main_thread: Option<NonNull<Thread>>,
    pub(in crate::scheduling) active_thread: Option<NonNull<Thread>>,

    pub(in crate::scheduling) pid: u64,
    pub(in crate::scheduling) status: TaskStatus,
    pub(in crate::scheduling) name: String,
    pub(in crate::scheduling) user: bool,

    pub(in crate::scheduling) next: Option<NonNull<Process>>,
    pub(in crate::scheduling) prev: Option<NonNull<Process>>,
}

impl Process {
    // todo: maybe add arguments to entry function signature
    /// Allocates memory on the heap for new process and initializes it. Returns the new task or an error code if the initialization failed.
    pub(in crate::scheduling) fn create(
        name: String,
        entry: TaskEntry,
        pid: u64,
    ) -> Result<Option<NonNull<Self>>, SchedulerError> {
        // set up new page table mappings
        let (pml4, entry_address) = allocate_page_mappings(&entry)?;

        // initialize new process
        let default = Process::empty();
        let process = NonNull::new(Box::into_raw(Box::new(default)));
        let process_ref = unsafe { process.unwrap().as_mut() };

        process_ref.name = name;
        process_ref.pid = pid;
        process_ref.status = TaskStatus::Ready;
        process_ref.page_table_mappings_virtual = pml4;
        process_ref.user = if let TaskEntry::User(_) = entry {
            true
        } else {
            false
        };

        // set up main thread
        process_ref.add_thread(Some(format!("{}{}", MAIN_THREAD_NAME, pid)), entry_address)?;

        Ok(process)
    }

    fn empty() -> Self {
        Self {
            status: TaskStatus::Dead,
            next: None,
            prev: None,
            pid: 0,
            page_table_mappings_virtual: ptr::null_mut(),
            thread_id_counter: 0,
            active_thread: None,
            name: "".to_string(),
            main_thread: None,
            user: false,
            // always update higher half mappings when switching processes
            // note: may be exchanged by a more efficient approach, that only updates the mappings if necessary, in the future.
            update_kernel_mappings: true,
        }
    }
}

impl Process {
    /// Get mutable reference to active thread.
    ///
    /// # Safety
    /// Caller must ensure that active thread exists.
    pub(in crate::scheduling) unsafe fn active_thread_mut(&mut self) -> &mut Thread {
        unsafe { self.active_thread.unwrap().as_mut() }
    }
    /// Get immutable reference to active thread.
    ///
    /// # Safety
    /// Caller must ensure that active thread exists.
    pub(in crate::scheduling) unsafe fn active_thread_ref(&self) -> &Thread {
        unsafe { self.active_thread.unwrap().as_ref() }
    }

    /// Adds the thread to the list of threads of the process. Returns the tid for the new thread or an error.
    pub(in crate::scheduling) fn add_thread(
        &mut self,
        name: Option<String>,
        entry: fn(),
    ) -> Result<u64, SchedulerError> {
        let mut current = self.main_thread;

        // every thread ever created has a unique ID
        self.thread_id_counter += 1;

        // main thread initialization
        if current.is_none() {
            let thread_ptr = Thread::create(
                name.unwrap_or(format!("MAIN-{}", self.thread_id_counter)),
                entry,
                self.thread_id_counter,
                self.pid,
                self.user,
            )?;
            self.main_thread = thread_ptr;
            self.active_thread = self.main_thread;
            return Ok(self.thread_id_counter);
        }

        while let Some(mut current_thread) = current {
            let current_thread = unsafe { current_thread.as_mut() };
            // append at the end of the list
            if current_thread.next.is_none() {
                let thread_ptr = Thread::create(
                    name.unwrap_or(format!("THREAD-{}", self.thread_id_counter)),
                    entry,
                    self.thread_id_counter,
                    self.pid,
                    self.user,
                )?;
                let thread = unsafe { thread_ptr.unwrap().as_mut() };
                thread.prev = current;

                current_thread.next = thread_ptr;
                return Ok(self.thread_id_counter);
            }
            current = current_thread.next;
        }

        // will not get called.
        Ok(0)
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

                // free vec of joins
                let _ = current_ref.joins.take();

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

    /// Gets the next ready thread information of the process. Returns whether the task has any alive threads, if all threads have been run for one iteration or the next ready thread.
    pub(in crate::scheduling) fn get_next_thread(&self, uptime: u64) -> NextThread {
        // mark task as dead.
        if self.is_dead() {
            return NextThread::TaskDead;
        }

        let mut next_thread = unsafe { self.active_thread_ref().next };

        // get next thread that is ready
        while let Some(mut thread) = next_thread {
            let thread_ref = unsafe { thread.as_mut() };

            if let ThreadStatus::Sleep(wake_time_ms) = thread_ref.status {
                if uptime >= wake_time_ms {
                    thread_ref.status = ThreadStatus::Ready;
                }
            }

            if thread_ref.status == ThreadStatus::Ready {
                break;
            }

            next_thread = thread_ref.next;
        }

        // all threads of the current process have been run once, switch to the next process.
        if next_thread.is_none() {
            NextThread::None
        }
        // run the next thread in the current process.
        else {
            NextThread::Found(next_thread)
        }
    }
}

impl Process {
    fn is_dead(&self) -> bool {
        if self.status == TaskStatus::Dead {
            return true;
        }

        assert!(
            self.main_thread.is_some(),
            "Each task must have a main thread."
        );

        if unsafe { self.main_thread.unwrap().as_ref().status == ThreadStatus::Dead } {
            return true;
        }

        let mut dead = true;
        let mut next_thread = self.main_thread;

        while let Some(thread) = next_thread {
            let thread_ref = unsafe { thread.as_ref() };
            if thread_ref.status != ThreadStatus::Dead {
                dead = false;
            }

            next_thread = thread_ref.next;
        }

        dead
    }
}

/// Copies higher half mappings from one page-table manager to another. Takes the virtual addresses of the root page tables.
///
/// # Safety
/// The caller must ensure that both addresses are mapped and point to valid page tables.
pub(in crate::scheduling) unsafe fn copy_higher_half_mappings(
    src_pml4: *mut PageTable,
    dst_pml4: *mut PageTable,
) -> Result<(), SchedulerError> {
    let src = src_pml4
        .as_mut()
        .ok_or(SchedulerError::PageTableManagerError(
            PagingError::Pml4PointerMisaligned,
        ))?;
    let dst = dst_pml4
        .as_mut()
        .ok_or(SchedulerError::PageTableManagerError(
            PagingError::Pml4PointerMisaligned,
        ))?;
    dst.entries.copy_from_slice(src.entries.as_slice());

    Ok(())
}

/// Copies the specified range of virtual memory mappings from one page-table manager to another.
///
/// # Safety
/// The caller must ensure that the addresses are mapped and point to valid page tables, as well as the fact that the virtual address range is valid and mapped in the source table.
pub(in crate::scheduling) unsafe fn map_user_process_from_active(
    active_manager: &mut OwnedPageTableManager,
    dst_pml4_virtual: *mut PageTable,
    virt_start: VirtualAddress,
    virt_end: VirtualAddress,
) {
    let (active_manager, pmm) = active_manager.get();
    let dst_pml4_physical = active_manager
        .get_physical(dst_pml4_virtual as VirtualAddress)
        .expect("User program must be mapped into active process before being loaded.")
        as *mut PageTable;

    let mut temp_manager = PageTableManager::new(dst_pml4_physical);
    unsafe {
        temp_manager.update_pml4_virtual(dst_pml4_virtual as VirtualAddress);
        temp_manager.update_offset(active_manager.offset());
    }

    assert!(virt_start < virt_end, "Copied page table mapping range must be valid: start index must be smaller than end index.");
    for virtual_address in (virt_start..virt_end).step_by(PAGE_SIZE) {
        let (physical_address, flags) = active_manager
            .get_entry_data(virtual_address)
            .expect("User program must be mapped into active process before being loaded.");

        assert!(
            flags.contains(PageEntryFlags::USER_SUPER),
            "Mapped user program must contain USER_SUPER flag."
        );
        // todo: proper error handling
        temp_manager
            .map_memory(virtual_address, physical_address, flags, pmm)
            .unwrap();

        println!(
            "current: phys: {:#x} to virt: {:#x} with flags: {:?}",
            physical_address, virtual_address, flags
        );
    }
}

/// Allocate new page table mappings. Copies the higher half mappings from the global page table manager. Returns the address to the new pml4 table and the mapped task entry pointer or an error value. The caller is responsible fpr freeing the memory allocated.
fn allocate_page_mappings(entry: &TaskEntry) -> Result<(*const PageTable, fn()), SchedulerError> {
    let mut binding = VMM.lock();
    if binding.get_mut().is_none() {
        return Err(SchedulerError::MemoryAllocationError(
            VmmError::GlobalVirtualMemoryManagerUninitialized,
        ));
    }

    // VMM is unlocked on drop
    let new_pml4 = {
        let vmm = binding.get_mut().unwrap();
        vmm.alloc(PAGE_SIZE, VmFlags::WRITE, AllocationType::AnyPages)? as *mut PageTable
    };
    // PTM must have been initialized at this point.
    assert!(
        PTM.lock().get().is_some(),
        "Global page table manager must have been initialized after using VMM."
    );
    let mut binding = PTM.lock();
    let owned_maanger = binding.get_mut().unwrap();
    let current_pml4 = owned_maanger.manager().pml4_virtual();

    unsafe {
        copy_higher_half_mappings(current_pml4, new_pml4)?;
    }
    crate::println!("before");
    let address = match entry {
        TaskEntry::Kernel(pointer) => *pointer,
        TaskEntry::User(data) => unsafe {
            println!("NOW");
            map_user_process_from_active(owned_maanger, new_pml4, data.virt_start, data.virt_end);
            core::mem::transmute::<u64, fn()>(data.virt_start)
        },
    };

    crate::println!("successs");

    Ok((new_pml4, address))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Ready,
    Running,
    Dead,
}

#[derive(Debug)]
pub(in crate::scheduling) enum NextThread {
    None,
    TaskDead,
    Found(Option<NonNull<Thread>>),
}
