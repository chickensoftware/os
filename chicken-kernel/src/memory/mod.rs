use chicken_util::BootInfo;

use crate::memory::pmm::BitMapAllocator;

pub(in crate::memory) mod paging;
pub(in crate::memory) mod pmm;

/// Sets up memory management and returns Boot info with proper virtual address pointers
pub(super) fn setup(boot_info: BootInfo) -> BootInfo {
    let pmm = BitMapAllocator::try_new(boot_info.memory_map).unwrap();
    let (pml4, boot_info) = paging::setup(pmm, boot_info).unwrap();
    paging::enable(pml4);

    boot_info
}
