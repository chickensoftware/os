use chicken_util::{BootInfo, memory::pmm::PageFrameAllocator};

use crate::memory::kheap::{KERNEL_HEAP_PAGE_COUNT, VIRTUAL_KERNEL_HEAP_BASE};

mod kheap;
pub(in crate::memory) mod paging;

/// Sets up memory management and returns Boot info with proper virtual address pointers
pub(super) fn setup(boot_info: &BootInfo) -> BootInfo {
    let pmm = unsafe { (boot_info.pmm_address as *const PageFrameAllocator).read() };
    let (mut manager, boot_info) = paging::setup(pmm, boot_info).unwrap();
    let pml4 = manager.pml4() as u64;

    paging::enable(pml4);

    kheap::init(
        VIRTUAL_KERNEL_HEAP_BASE,
        KERNEL_HEAP_PAGE_COUNT,
        &mut manager,
    )
    .unwrap();

    boot_info
}

/// Aligns a given number to the specified alignment.
pub(in crate::memory) fn align_up(number: u64, align: usize) -> u64 {
    let align = align as u64;
    (number + align - 1) & !(align - 1)
}
