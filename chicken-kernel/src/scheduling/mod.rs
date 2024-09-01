use alloc::{
    alloc::dealloc,
    format,
    string::{String, ToString},
    vec,
};
use core::{
    alloc::Layout,
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
    ptr::NonNull,
};

use chicken_util::memory::{paging::PageTable, VirtualAddress};

use crate::{base::interrupts::{CpuState, without_interrupts}, hlt_loop, main_task, memory::{
    paging,
    paging::{PagingError, PTM},
    vmm::{VMM, VmmError},
}, print, scheduling::{
    spin::{Guard, SpinLock},
    task::{
        JoinHandle,
        process::{copy_higher_half_mappings, NextThread, Process, TaskStatus},
    },
}};

pub(crate) mod spin;
mod task;

pub(crate) static SCHEDULER: GlobalTaskScheduler = GlobalTaskScheduler::new();
pub(super) fn set_up() {
    GlobalTaskScheduler::init();
}

#[derive(Debug)]
pub(crate) struct GlobalTaskScheduler {
    inner: SpinLock<OnceCell<TaskScheduler>>,
}

unsafe impl Sync for GlobalTaskScheduler {}

impl GlobalTaskScheduler {
    /// Create new empty Global Task Scheduler instance.
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(OnceCell::new()),
        }
    }

    /// Initialize Global Task Scheduler.
    pub(super) fn init() {
        let scheduler = SCHEDULER.inner.lock();
        scheduler.get_or_init(|| TaskScheduler::try_new().unwrap());
    }

    pub(crate) fn lock(&self) -> Guard<OnceCell<TaskScheduler>> {
        self.inner.lock()
    }

    /// Mark currently active thread as dead.
    pub(crate) fn kill_active() {
        // loop in case of interrupt during function call
        loop {
            without_interrupts(|| {
                let mut binding = SCHEDULER.lock();
                if let Some(scheduler) = binding.get_mut() {
                    assert!(
                        scheduler.active_task.is_some(),
                        "Global task scheduler must have at least one active task (IDLE)."
                    );
                    let active = unsafe { scheduler.active_task.unwrap().as_mut() };
                    let thread = unsafe { active.active_thread_ref() };
                    let mut can_die = true;

                    // check for any joins
                    if let Some(ref joins) = thread.joins {
                        // loop through each thread of active process and check if it has been joined & is alive
                        let mut current_thread = active.main_thread;

                        while let Some(current_thread_ptr) = current_thread {
                            let thread_ref = unsafe { current_thread_ptr.as_ref() };

                            if thread_ref.tid != thread.tid
                                && thread_ref.status != TaskStatus::Dead
                                && joins.iter().copied().any(|id| id == thread_ref.tid)
                            {
                                can_die = false;
                            }

                            current_thread = thread_ref.next;
                        }
                    }
                    let thread = unsafe { active.active_thread_mut() };

                    if can_die && thread.status != TaskStatus::Dead {
                        thread.status = TaskStatus::Dead;
                    }
                }
            });
        }
    }

    /// Joins the thread specified by the handle to the current one.
    fn join(handle: JoinHandle) {
        without_interrupts(|| {
            let mut binding = SCHEDULER.lock();
            if let Some(scheduler) = binding.get_mut() {
                assert!(
                    scheduler.active_task.is_some(),
                    "Global task scheduler must have at least one active task (IDLE)."
                );
                let active = unsafe { scheduler.active_task.unwrap().as_mut() };
                assert!(
                    active.active_thread.is_some(),
                    "Each active task must have at least one active thread (MAIN)."
                );
                let thread = unsafe { active.active_thread_mut() };

                if let Some(ref mut joins) = thread.joins {
                    joins.push(handle.into_inner());
                } else {
                    thread.joins = Some(vec![handle.into_inner()]);
                }
            }
        });
    }
}

#[derive(Debug)]
pub(crate) struct TaskScheduler {
    head: Option<NonNull<Process>>,
    active_task: Option<NonNull<Process>>,
    id_counter: u64,
}

impl TaskScheduler {
    /// Attempts to initialize a new task scheduler with an idle task.
    fn try_new() -> Result<Self, SchedulerError> {
        let mut instance = Self {
            head: None,
            active_task: None,
            id_counter: 0,
        };

        instance.add_task(Some("IDLE".to_string()), idle)?;
        instance.add_task(Some("A".to_string()), a)?;

        Ok(instance)
    }
}

fn idle() {
    hlt_loop();
}

fn a() {
    let handle1 = task::spawn_thread(b, Some("mythready".to_string())).unwrap();
    let handle2 = task::spawn_thread(c, Some("mythready2".to_string())).unwrap();
    GlobalTaskScheduler::join(handle1);
    GlobalTaskScheduler::join(handle2);

    task::spawn_process(main_task, Some("KERNEL-MAIN".to_string())).unwrap();

    GlobalTaskScheduler::kill_active();
}

fn b() {
    for _ in 0..500 {
        print!("B");
    }
    GlobalTaskScheduler::kill_active();
}

fn c() {
    for _ in 0..500 {
        print!("C");
    }
    GlobalTaskScheduler::kill_active();
}

impl TaskScheduler {
    pub(crate) fn schedule(&mut self, context: *const CpuState, _uptime: u64) -> *const CpuState {
        if let Some(mut active_task) = self.active_task {
            let active_task = unsafe { active_task.as_mut() };
            match active_task.get_next_thread() {
                // switch to next process
                NextThread::None => {
                    // store state of previously active thread
                    let previously_active_thread = unsafe { active_task.active_thread_mut() };
                    if previously_active_thread.status != TaskStatus::Dead {
                        previously_active_thread.status = TaskStatus::Ready;
                        previously_active_thread.context = context;
                    }

                    // set active thread to main thread
                    active_task.active_thread = active_task.main_thread;
                }
                // switch to next process
                NextThread::TaskDead => {
                    // mark task as dead, so it gets removed later.
                    active_task.status = TaskStatus::Dead;
                }
                // execute next ready thread in current process
                NextThread::Found(next_thread) => {
                    // save state of previously active thread
                    let active_thread = unsafe { active_task.active_thread_mut() };
                    if active_thread.status != TaskStatus::Dead {
                        active_thread.context = context;
                        active_thread.status = TaskStatus::Ready;
                    }

                    // set active thread to found thread
                    active_task.active_thread = next_thread;
                    unsafe {
                        active_task.active_thread_mut().status = TaskStatus::Running;
                    }

                    // return context of next thread
                    return unsafe { active_task.active_thread_ref().context };
                }
            }
            // no threads are ready in the current process
            self.switch_processes(active_task, context)
        } else {
            // first time context switch is called. start with IDLE task
            let idle = self.head;
            assert!(idle.is_some(), "Head Process must be idle task");
            let idle_ref = unsafe { idle.unwrap().as_mut() };
            idle_ref.status = TaskStatus::Running;

            idle_ref.active_thread = idle_ref.main_thread;
            unsafe {
                idle_ref.active_thread_mut().status = TaskStatus::Running;
            }

            self.active_task = idle;
            unsafe { idle_ref.active_thread_mut().context }
        }
    }

    fn switch_processes(
        &mut self,
        active_task: &mut Process,
        context: *const CpuState,
    ) -> *const CpuState {
        let next_active_task = self.get_next_process(active_task);

        // set up new next task and remove old one if it's dead
        if let Some(mut next_active_task) = next_active_task {
            let next_active_task_ref = unsafe { next_active_task.as_mut() };

            // save currently active state if task is not dead
            if active_task.status != TaskStatus::Dead {
                active_task.status = TaskStatus::Ready;
            }

            // update new active task
            next_active_task_ref.status = TaskStatus::Running;
            self.active_task = Some(next_active_task);

            // switch to other paging scheme
            let mut binding = PTM.lock();
            assert!(
                binding.get().is_some(),
                "PTM must be set up when calling scheduler."
            );
            let manager = binding.get_mut().unwrap();

            // copy higher half page tables if kernel mappings have been changed by current process
            if active_task.update_kernel_mappings {
                unsafe {
                    copy_higher_half_mappings(
                        manager.pml4_virtual(),
                        next_active_task_ref.page_table_mappings as *mut PageTable,
                    )
                    .unwrap();
                }
            }
            let new_mappings_address =
                manager.get_physical(next_active_task_ref.page_table_mappings as VirtualAddress);

            assert!(
                new_mappings_address.is_some(),
                "Page table mappings of each process must be set up."
            );
            let new_mappings = new_mappings_address.unwrap();
            unsafe {
                paging::enable(new_mappings);
            }
            let ptm = binding.get_mut().unwrap();
            unsafe {
                ptm.update_pml4(new_mappings);
            }
            PTM.unlock();

            unsafe { next_active_task_ref.main_thread.unwrap().as_ref().context }
        } else {
            context
        }
    }

    fn get_next_process(&mut self, active_task: &mut Process) -> Option<NonNull<Process>> {
        // remove dead tasks from the list and get next active task
        let mut next_active_task = if active_task.next.is_some() {
            active_task.next
        } else {
            self.head
        };

        while let Some(current_task) = next_active_task {
            let current_ref = unsafe { current_task.as_ref() };
            // could not find valid task
            if current_ref.pid == active_task.pid {
                break;
            }
            match current_ref.status {
                // found valid next task
                TaskStatus::Ready => break,
                // remove dead task
                TaskStatus::Dead => self.remove_task(current_ref.pid).unwrap(),
                TaskStatus::Running => {}
            }

            // round-robin
            if current_ref.next.is_some() {
                next_active_task = current_ref.next;
            } else {
                next_active_task = self.head;
            }
        }

        next_active_task
    }
}

impl TaskScheduler {
    /// Appends a task to the list of tasks.
    fn add_task(&mut self, name: Option<String>, entry: fn()) -> Result<(), SchedulerError> {
        let mut current = self.head;

        // every task ever created has a unique ID
        self.id_counter += 1;

        if current.is_none() {
            let task_ptr = Process::create(
                name.unwrap_or(format!("TASK-{}", self.id_counter)),
                entry,
                self.id_counter,
            )?;
            self.head = task_ptr;
            return Ok(());
        }

        while let Some(mut current_task) = current {
            let current_task = unsafe { current_task.as_mut() };
            if current_task.next.is_none() {
                let task_ptr = Process::create(
                    name.unwrap_or(format!("TASK-{}", self.id_counter)),
                    entry,
                    self.id_counter,
                )?;
                let task = unsafe { task_ptr.unwrap().as_mut() };
                task.prev = current;

                current_task.next = task_ptr;
                return Ok(());
            }
            current = current_task.next;
        }
        Ok(())
    }

    /// Removes the specified task from the list. Returns whether the action succeeds. The task to be removed must not be the currently active one.
    fn remove_task(&mut self, id: u64) -> Result<(), SchedulerError> {
        let active_task = self.active_task;
        assert!(active_task.is_some(), "Active task must be present.");
        assert_ne!(
            unsafe { active_task.unwrap().as_ref().pid },
            id,
            "Active task must not be removed while still active."
        );
        assert_ne!(
            unsafe { self.head.unwrap().as_ref().pid },
            id,
            "Idle task must not be removed."
        );

        let mut current = self.head;
        while let Some(mut current_task) = current {
            let current_ref = unsafe { current_task.as_mut() };

            if current_ref.pid == id {
                // remove task from linked list
                let heap_ptr = if let Some(mut prev) = current_ref.prev {
                    let prev_ref = unsafe { prev.as_mut() };
                    let heap_ptr = prev_ref.next.unwrap().as_ptr();
                    prev_ref.next = current_ref.next;
                    heap_ptr
                } else {
                    // will never happen, since the idle task cannot be removed.
                    let heap_ptr = self.head.unwrap().as_ptr();
                    self.head = current_ref.next;

                    heap_ptr
                };

                if let Some(mut next) = current_ref.next {
                    let next_ref = unsafe { next.as_mut() };
                    next_ref.prev = current_ref.prev;
                }

                // remove all threads of the process
                let mut current_thread = current_ref.main_thread;

                while let Some(mut thread) = current_thread {
                    let thread_ref = unsafe { thread.as_mut() };
                    current_ref.remove_thread(thread_ref.tid, true)?;
                    current_thread = thread_ref.next;
                }

                // deallocate the process
                unsafe {
                    dealloc(heap_ptr as *mut u8, Layout::new::<Process>());
                }

                let mut binding = VMM.lock();
                let vmm = binding
                    .get_mut()
                    .ok_or(SchedulerError::MemoryAllocationError(
                        VmmError::GlobalVirtualMemoryManagerUninitialized,
                    ))?;

                // free the process's page tables
                let pml4_address = current_ref.page_table_mappings as u64;
                vmm.free(pml4_address).map_err(SchedulerError::from)?;

                return Ok(());
            }
            current = current_ref.next;
        }

        Err(SchedulerError::TaskNotFound(id))
    }
}

#[derive(Copy, Clone)]
pub(crate) enum SchedulerError {
    TaskNotFound(u64),
    ThreadNotFound(u64, u64),
    MemoryAllocationError(VmmError),
    PageTableManagerError(PagingError),
}

impl Debug for SchedulerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            SchedulerError::TaskNotFound(id) => write!(
                f,
                "Scheduler Error: Could not find task with ID: {} in task list.",
                id
            ),
            SchedulerError::ThreadNotFound(pid, tid) => write!(
                f,
                "Scheduler Error: Could not find thread with TID: {} in task: PID: {}.",
                tid, pid
            ),
            SchedulerError::MemoryAllocationError(value) => {
                write!(f, "Scheduler Error: Memory allocation failed: {}", value)
            }
            SchedulerError::PageTableManagerError(value) => {
                write!(f, "Scheduler Error: Memory mapping failed: {}", value)
            }
        }
    }
}

impl Display for SchedulerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for SchedulerError {}

impl From<VmmError> for SchedulerError {
    fn from(value: VmmError) -> Self {
        Self::MemoryAllocationError(value)
    }
}

impl From<PagingError> for SchedulerError {
    fn from(value: PagingError) -> Self {
        Self::PageTableManagerError(value)
    }
}
