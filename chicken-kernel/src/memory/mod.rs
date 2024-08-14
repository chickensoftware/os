use chicken_util::BootInfo;

use crate::memory::pmm::BitMapAllocator;

pub(in crate::memory) mod paging;
pub(in crate::memory) mod pmm;

pub(super) fn setup(boot_info: &BootInfo) {
    let pmm = BitMapAllocator::try_new(boot_info.memory_map).unwrap();

    let pml4 = paging::setup(pmm, boot_info.memory_map).unwrap();
    paging::enable(pml4);
}
