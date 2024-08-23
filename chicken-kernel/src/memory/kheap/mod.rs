use core::{
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use chicken_util::{
    memory::{
        paging::{manager::PageTableManager, PageEntryFlags},
        VirtualAddress,
    },
    PAGE_SIZE,
};

use crate::{
    memory::{kheap::linked_list::LinkedListAllocator, paging::PagingError},
    scheduling::spin::SpinLock,
};

mod bump;

mod linked_list;

pub(in crate::memory) const VIRTUAL_KERNEL_HEAP_BASE: u64 = 0xFFFF_FFFF_F000_0000;


pub(super) const KERNEL_HEAP_PAGE_COUNT: usize = 0x100; // 1 MiB
pub(super) const MAX_KERNEL_HEAP_PAGE_COUNT: usize = 0x4000; // 64 MiB

#[global_allocator]
static ALLOCATOR: SpinLock<OnceCell<LinkedListAllocator>> = SpinLock::new(OnceCell::new());

unsafe impl Send for LinkedListAllocator {}

pub(super) fn init(
    heap_address: VirtualAddress,
    heap_page_count: usize,
    page_table_manager: &mut PageTableManager,
) -> Result<(), PagingError> {
    for page in 0..heap_page_count {
        let physical_address = page_table_manager
            .pmm()
            .request_page()
            .map_err(PagingError::from)?;
        page_table_manager
            .map_memory(
                heap_address + (page * PAGE_SIZE) as u64,
                physical_address,
                PageEntryFlags::default_nx(),
            )
            .unwrap();
    }

    ALLOCATOR.lock().get_or_init(|| {
        LinkedListAllocator::try_new(heap_address, heap_page_count * PAGE_SIZE).unwrap()
    });

    Ok(())
}

#[derive(Copy, Clone)]
pub(in crate::memory) enum HeapError {
    InvalidBlockSize(usize),
    OutOfMemory,
}

impl Debug for HeapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            HeapError::InvalidBlockSize(size) => {
                write!(f, "Heap Error: Invalid block size: {}.", size)
            }
            HeapError::OutOfMemory => write!(f, "Heap Error: Out of memory."),
        }
    }
}

impl Display for HeapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for HeapError {}
