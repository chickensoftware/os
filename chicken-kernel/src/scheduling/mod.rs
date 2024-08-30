use alloc::{
    alloc::dealloc,
    string::{String, ToString},
};
use core::{
    alloc::Layout,
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
    ptr::NonNull,
};

use chicken_util::memory::VirtualAddress;

use crate::{
    base::interrupts::{CpuState, without_interrupts},
    hlt_loop,
    memory::{
        paging,
        paging::{PagingError, PTM},
        vmm::{VMM, VmmError},
    },
    print, println,
    scheduling::{
        spin::{Guard, SpinLock},
        task::process::{Process, TaskStatus},
    },
};

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
    fn kill_active() {
        // loop in case of interrupt during function call
        loop {
            without_interrupts(|| {
                let mut binding = SCHEDULER.lock();
                if let Some(scheduler) = binding.get_mut() {
                    let active = unsafe { scheduler.active_task.unwrap().as_mut() };
                    active.active_thread().status = TaskStatus::Dead;
                }
            });
        }
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

        instance.add_task("IDLE".to_string(), idle)?;
        instance.add_task("TEST".to_string(), test)?;
        instance.add_task("A".to_string(), a)?;

        Ok(instance)
    }
}

fn idle() {
    println!("idle");
    hlt_loop();
}

fn test() {
    println!("Test task called!");
    GlobalTaskScheduler::kill_active();
}

fn a() {
    without_interrupts(|| {
        let mut binding = SCHEDULER.lock();
        if let Some(scheduler) = binding.get_mut() {
            let active = unsafe { scheduler.active_task.unwrap().as_mut() };
            active.add_thread("B".to_string(), b).unwrap();
            active.add_thread("C".to_string(), c).unwrap();
        }
    });

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
    pub(crate) fn schedule(&mut self, context: *const CpuState) -> *const CpuState {
        if let Some(mut active_task) = self.active_task {
            let active_task = unsafe { active_task.as_mut() };
            let mut next_active_thread = active_task.active_thread().next;
            let mut dead = true;

            if next_active_thread.is_none() {
                next_active_thread = active_task.main_thread
            }

            let active_tid = active_task.active_thread().tid;

            // iterate through each thread of the current process
            while let Some(mut current_thread) = next_active_thread {
                let thread_ref = unsafe { current_thread.as_mut() };

                if thread_ref.status != TaskStatus::Dead {
                    dead = false;
                }

                if thread_ref.tid == active_tid {
                    break;
                }

                if thread_ref.status == TaskStatus::Ready {
                    let active_thread_ref = active_task.active_thread();
                    // save state of previous active thread
                    if active_thread_ref.status != TaskStatus::Dead {
                        active_thread_ref.context = context;
                        active_thread_ref.status = TaskStatus::Ready;
                    }
                    // switch to the next thread in the current process
                    active_task.active_thread = Some(current_thread);
                    thread_ref.status = TaskStatus::Running;

                    return thread_ref.context;
                }

                next_active_thread = if thread_ref.next.is_some() {
                    thread_ref.next
                } else {
                    active_task.main_thread
                };
            }

            if dead {
                active_task.status = TaskStatus::Dead;
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
            idle_ref.active_thread().status = TaskStatus::Running;

            self.active_task = idle;

            idle_ref.active_thread().context
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
                active_task.active_thread().context = context;
            }

            // update new active task
            next_active_task_ref.status = TaskStatus::Running;
            self.active_task = Some(next_active_task);

            // switch to other paging scheme
            // todo: maybe issue with expanding kernel heap in one process and making it smaller in other process?
            let mut binding = PTM.lock();
            assert!(
                binding.get().is_some(),
                "PTM must be set up when calling scheduler."
            );
            let new_mappings = binding
                .get()
                .unwrap()
                .get_physical(next_active_task_ref.page_table_mappings as VirtualAddress);
            assert!(
                new_mappings.is_some(),
                "Page table mappings of each process must be set up."
            );
            let new_mappings = new_mappings.unwrap();
            unsafe {
                paging::enable(new_mappings);
            }
            let ptm = binding.get_mut().unwrap();
            unsafe {
                ptm.update_pml4(new_mappings);
            }
            PTM.unlock();

            next_active_task_ref.active_thread().context
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
    fn add_task(&mut self, name: String, entry: fn()) -> Result<(), SchedulerError> {
        let mut current = self.head;

        // every task ever created has a unique ID
        self.id_counter += 1;

        if current.is_none() {
            let task_ptr = Process::create(name, entry, self.id_counter)?;
            self.head = task_ptr;
            return Ok(());
        }

        while let Some(mut current_task) = current {
            let current_task = unsafe { current_task.as_mut() };
            if current_task.next.is_none() {
                let task_ptr = Process::create(name, entry, self.id_counter)?;
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
