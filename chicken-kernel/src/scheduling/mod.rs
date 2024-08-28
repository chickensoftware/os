use alloc::{alloc::dealloc, boxed::Box};
use core::{
    alloc::Layout,
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
    ptr,
    ptr::NonNull,
};

use crate::{
    base::interrupts::CpuState,
    scheduling::{
        spin::{Guard, SpinLock},
        task::{Task, TaskStatus},
    },
};

pub(crate) mod spin;
pub(crate) mod task;

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
        let vmm = SCHEDULER.inner.lock();
        vmm.get_or_init(TaskScheduler::new);
    }

    pub(crate) fn lock(&self) -> Guard<OnceCell<TaskScheduler>> {
        self.inner.lock()
    }
}

#[derive(Debug)]
pub(crate) struct TaskScheduler {
    head: Option<NonNull<Task>>,
    active_task: Option<NonNull<Task>>,
    id_counter: u64,
}

impl TaskScheduler {
    /// Initializes a new task scheduler with an idle task
    fn new() -> Self {
        let idle_task = Task {
            status: TaskStatus::Ready,
            context: ptr::null_mut(),
            next: None,
            prev: None,
            id: 0,
        };

        let mut instance = Self {
            head: None,
            active_task: None,
            id_counter: 0,
        };

        instance.add_task(idle_task);
        instance.active_task = instance.head;
        instance
    }
}

impl TaskScheduler {
    pub(crate) fn schedule(&mut self, context: *const CpuState) -> *const CpuState {
        let active_task = self.active_task;
        assert!(active_task.is_some(), "Active task must be present.");
        let active_task = unsafe { active_task.unwrap().as_mut() };

        // remove dead tasks from the list and get next active task
        let mut next_active_task = active_task.next;
        while let Some(current_task) = next_active_task {
            let current_ref = unsafe { current_task.as_ref() };

            // could not find valid task
            if current_ref.id == active_task.id {
                break;
            }

            match current_ref.status {
                // found valid next task
                TaskStatus::Ready => break,
                // remove dead tasks
                TaskStatus::Dead => self.remove_task(current_ref.id).unwrap(),
                // should never happen
                TaskStatus::Running => {}
            }

            if current_ref.next.is_some() {
                next_active_task = current_ref.next;
            } else {
                next_active_task = self.head;
            }
        }

        if let Some(next_active_task) = next_active_task {
            let next_active_task_ref = unsafe { next_active_task.as_ref() };
            // save currently active state
            active_task.status = TaskStatus::Ready;
            active_task.context = context;

            self.active_task = Some(next_active_task);

            next_active_task_ref.context
        } else {
            context
        }
    }
}

impl TaskScheduler {
    /// Appends a task to the list of tasks.
    fn add_task(&mut self, mut task: Task) {
        let mut current = self.head;

        // every task ever created has a unique ID
        self.id_counter += 1;

        if current.is_none() {
            let task_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(task))) };
            self.head = Some(task_ptr);
            return;
        }

        while let Some(mut current_task) = current {
            let current_task = unsafe { current_task.as_mut() };
            if current_task.next.is_none() {
                task.prev = current;

                let task_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(task))) };
                current_task.next = Some(task_ptr);

                return;
            }
            current = current_task.next;
        }
    }

    /// Removes the specified task from the list. Returns whether the action succeeds. The task to be removed must not be the currently active one.
    fn remove_task(&mut self, id: u64) -> Result<(), SchedulerError> {
        let active_task = self.active_task;
        assert!(active_task.is_some(), "Active task must be present.");

        assert_ne!(
            id,
            unsafe { active_task.unwrap().as_ref().id },
            "Active task must not be removed."
        );

        let mut current = self.head;
        while let Some(mut current_task) = current {
            let current_ref = unsafe { current_task.as_mut() };

            if current_ref.id == id {
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
                    dealloc(heap_ptr as *mut u8, Layout::new::<Task>());
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
}

impl Debug for SchedulerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            SchedulerError::TaskNotFound(id) => write!(
                f,
                "Scheduler Error: Could not find task with ID: {} in task list.",
                id
            ),
        }
    }
}

impl Display for SchedulerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for SchedulerError {}
