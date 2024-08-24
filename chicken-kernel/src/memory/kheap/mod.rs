use core::{
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use chicken_util::{
    memory::{paging::PageEntryFlags, pmm::PageFrameAllocatorError, VirtualAddress},
    PAGE_SIZE,
};

use crate::{
    memory::{
        kheap::linked_list::LinkedListAllocator,
        paging::{PagingError, PTM}
        ,
    },
    scheduling::spin::{Guard, SpinLock},
};

mod bump;

mod linked_list;

pub(in crate::memory) const VIRTUAL_KERNEL_HEAP_BASE: u64 = 0xFFFF_FFFF_F000_0000;

pub(super) const KERNEL_HEAP_PAGE_COUNT: usize = 0x100; // 1 MiB
pub(super) const MAX_KERNEL_HEAP_PAGE_COUNT: usize = 0x4000; // 64 MiB

/// Heap used by the kernel itself. Provides dynamic allocations for the VMM.
/// User Applications have their own user heap that depends on the VMM.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::new();

#[derive(Debug)]
pub(super) struct LockedHeap {
    inner: SpinLock<OnceCell<LinkedListAllocator>>,
}

unsafe impl Send for LockedHeap {}

unsafe impl Sync for LockedHeap {}


impl LockedHeap {
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(OnceCell::new()),
        }
    }

    pub(super) fn init(
        heap_address: VirtualAddress,
        heap_page_count: usize,
    ) -> Result<(), HeapError> {
        if let Some(page_table_manager) = PTM.lock().get_mut() {
            for page in 0..heap_page_count {
                let physical_address = page_table_manager
                    .pmm()
                    .request_page()
                    .map_err(HeapError::from)?;

                page_table_manager
                    .map_memory(
                        heap_address + (page * PAGE_SIZE) as u64,
                        physical_address,
                        PageEntryFlags::default_nx(),
                    )
                    .map_err(HeapError::from)?;
            }
            let heap = LinkedListAllocator::try_new(heap_address, heap_page_count * PAGE_SIZE)?;
            ALLOCATOR.lock().get_or_init(|| heap);
            Ok(())
        } else {
            Err(HeapError::PageTableManagerError(
                PagingError::GlobalPageTableManagerUninitialized,
            ))
        }
    }

    fn lock(&self) -> Guard<OnceCell<LinkedListAllocator>> {
        self.inner.lock()
    }
}

#[derive(Copy, Clone)]
pub(in crate::memory) enum HeapError {
    InvalidBlockSize(usize),
    OutOfMemory,
    PageTableManagerError(PagingError),
    PageFrameAllocationFailed(PageFrameAllocatorError),
}

impl Debug for HeapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            HeapError::InvalidBlockSize(size) => {
                write!(f, "Heap Error: Invalid block size: {}.", size)
            }
            HeapError::OutOfMemory => write!(f, "Heap Error: Out of memory."),
            HeapError::PageTableManagerError(value) => write!(f, "Heap Error: {}", value),
            HeapError::PageFrameAllocationFailed(value) => write!(
                f,
                "Heap Error: Page frame allocation or mapping has failed: {}.",
                value
            ),
        }
    }
}

impl Display for HeapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for HeapError {}

impl From<PageFrameAllocatorError> for HeapError {
    fn from(value: PageFrameAllocatorError) -> Self {
        Self::PageFrameAllocationFailed(value)
    }
}

impl From<PagingError> for HeapError {
    fn from(value: PagingError) -> Self {
        Self::PageTableManagerError(value)
    }
}
