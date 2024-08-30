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
        vmm::VmmError,
    },
    print,
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
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(OnceCell::new()),
        }
    }

    pub(super) fn init() {
        let scheduler = SCHEDULER.inner.lock();
        scheduler.get_or_init(|| TaskScheduler::try_new().unwrap());
    }

    pub(crate) fn lock(&self) -> Guard<OnceCell<TaskScheduler>> {
        self.inner.lock()
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

        Ok(instance)
    }
}

fn idle() {
    hlt_loop();
}

fn test() {
    loop {
        without_interrupts(|| print!("B"));
    }
}

impl TaskScheduler {
    // todo: implement ability to mark tasks as DEAD
    pub(crate) fn schedule(&mut self, context: *const CpuState) -> *const CpuState {
        if let Some(mut active_task) = self.active_task {
            let active_task = unsafe { active_task.as_mut() };

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
                    // remove dead tasks
                    TaskStatus::Dead => self.remove_task(current_ref.pid).unwrap(),
                    // should never happen
                    TaskStatus::Running => {}
                }

                if current_ref.next.is_some() {
                    next_active_task = current_ref.next;
                } else {
                    next_active_task = self.head;
                }
            }

            if let Some(mut next_active_task) = next_active_task {
                let next_active_task_ref = unsafe { next_active_task.as_mut() };
                // save currently active state
                active_task.status = TaskStatus::Ready;
                active_task.context = context;

                next_active_task_ref.status = TaskStatus::Running;

                self.active_task = Some(next_active_task);

                // switch to other paging scheme
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
                unsafe { paging::enable(new_mappings); }
                let ptm = binding.get_mut().unwrap();
                unsafe {
                    ptm.update_pml4(new_mappings);
                }
                PTM.unlock();

                next_active_task_ref.context
            } else {
                context
            }
        } else {
            // first time context switch is called. start with IDLE task
            let idle = self.head;
            assert!(idle.is_some(), "Head Process must be idle task");
            let idle_ref = unsafe { idle.unwrap().as_mut() };
            idle_ref.status = TaskStatus::Running;

            self.active_task = idle;

            idle_ref.context
        }
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
            id,
            unsafe { active_task.unwrap().as_ref().pid },
            "Active task must not be removed."
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
                    let heap_ptr = self.head.unwrap().as_ptr();
                    self.head = current_ref.next;

                    heap_ptr
                };

                if let Some(mut next) = current_ref.next {
                    let next_ref = unsafe { next.as_mut() };
                    next_ref.prev = current_ref.prev;
                }

                // deallocate task
                unsafe {
                    dealloc(heap_ptr as *mut u8, Layout::new::<Process>());
                }

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
