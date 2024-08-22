use chicken_util::{BootInfo, memory::pmm::PageFrameAllocator};

pub(in crate::memory) mod paging;

/// Sets up memory management and returns Boot info with proper virtual address pointers
pub(super) fn setup(boot_info: &BootInfo) -> BootInfo {
    let pmm = unsafe { (boot_info.pmm_address as *const PageFrameAllocator).read() };
    let (manager, boot_info) = paging::setup(pmm, boot_info).unwrap();
    let pml4 = manager.pml4() as u64;

    paging::enable(pml4);

    boot_info
}
