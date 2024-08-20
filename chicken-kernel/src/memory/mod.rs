use chicken_util::BootInfo;
use chicken_util::memory::pmm::PageFrameAllocator;

pub(in crate::memory) mod paging;

/// Sets up memory management and returns Boot info with proper virtual address pointers
pub(super) fn setup(boot_info: &BootInfo) -> BootInfo {
    let pmm = unsafe { (boot_info.pmm_address as *const PageFrameAllocator).read() };
    let (pml4, boot_info) = paging::setup(pmm, boot_info).unwrap();
    paging::enable(pml4);

    boot_info
}
