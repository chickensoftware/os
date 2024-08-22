use chicken_util::{
    memory::{
        paging::{manager::PageTableManager, PageEntryFlags},
        VirtualAddress,
    },
    PAGE_SIZE,
};

use crate::{
    memory::{kheap::bump::BumpAllocator, paging::PagingError},
    scheduling::spin::SpinLock,
};

mod bump;

pub(in crate::memory) const VIRTUAL_KERNEL_HEAP_BASE: u64 = 0xFFFF_FFFF_F000_0000;

pub(super) const KERNEL_HEAP_PAGE_COUNT: usize = 0x100; // 1 MiB

#[global_allocator]
static ALLOCATOR: SpinLock<BumpAllocator> = SpinLock::new(BumpAllocator::new());

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

    unsafe {
        ALLOCATOR
            .lock()
            .init(heap_address, heap_page_count * PAGE_SIZE);
    }
    Ok(())
}
