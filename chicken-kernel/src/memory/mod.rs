use chicken_util::{
    BootInfo,
    memory::{
        MemoryMap,
        MemoryType,
        paging::{KERNEL_MAPPING_OFFSET, KERNEL_STACK_MAPPING_OFFSET}, pmm::PageFrameAllocator, VirtualAddress,
    },
};

use crate::memory::{
    kheap::{KERNEL_HEAP_PAGE_COUNT, LockedHeap, VIRTUAL_KERNEL_HEAP_BASE},
    paging::{GlobalPageTableManager, smallest_address, VIRTUAL_DATA_BASE, VIRTUAL_PHYSICAL_BASE},
    vmm::{
        AllocationType, GlobalVirtualMemoryManager, object::VmFlags, VIRTUAL_VMM_BASE, VMM,
        VMM_PAGE_COUNT, VmmError,
    },
};

pub(in crate::memory) mod paging;

mod kheap;
mod vmm;

/// Sets up memory management and returns Boot info with proper virtual address pointers
pub(super) fn setup(boot_info: &BootInfo) -> BootInfo {
    // get physical memory manager
    let pmm = unsafe { (boot_info.pmm_address as *const PageFrameAllocator).read() };

    // set up paging
    let (manager, mut boot_info) = paging::setup(pmm, boot_info).unwrap();
    let pml4 = manager.pml4() as u64;

    // switch to new paging scheme
    paging::enable(pml4);

    // initialize static global page table manager
    GlobalPageTableManager::init(manager);

    // initialize kernel heap
    LockedHeap::init(VIRTUAL_KERNEL_HEAP_BASE, KERNEL_HEAP_PAGE_COUNT).unwrap();

    // initialize static global vmm
    GlobalVirtualMemoryManager::init(VIRTUAL_VMM_BASE, VMM_PAGE_COUNT);

    // use vmm to map framebuffer
    mmio(&mut boot_info).unwrap();
    let mut vmm = VMM.lock();
    let vmm = vmm.get_mut().unwrap();
    // test use case of vmm
    let page_sized_buffer = vmm
        .alloc(0x932, VmFlags::WRITE, AllocationType::AnyPages)
        .unwrap();
    vmm.free(page_sized_buffer).unwrap();

    boot_info
}

/// Aligns a given number to the specified alignment.
pub(in crate::memory) fn align_up(number: u64, align: usize) -> u64 {
    let align = align as u64;
    (number + align - 1) & !(align - 1)
}

/// Sets up MMIO memory regions like the framebuffer.
fn mmio(boot_info: &mut BootInfo) -> Result<(), VmmError> {
    let mut vmm = VMM.lock();
    if let Some(vmm) = vmm.get_mut() {
        let framebuffer_metadata = boot_info.framebuffer_metadata;
        // identity map framebuffer
        let fb_base_address = framebuffer_metadata.base;

        let fb_virtual_address = vmm.alloc(
            framebuffer_metadata.size,
            VmFlags::MMIO | VmFlags::WRITE,
            AllocationType::Address(fb_base_address),
        )?;
        boot_info.framebuffer_metadata.base = fb_virtual_address;
        Ok(())
    } else {
        Err(VmmError::GlobalVirtualMemoryManagerUninitialized)
    }
}

/// Returns the virtual memory offset used in the direct mapping for the given memory type. Returns None if the memory type does not get mapped or if the memory map is invalid/empty.
pub(crate) fn get_virtual_offset(
    memory_type: MemoryType,
    memory_map: &MemoryMap,
) -> Option<VirtualAddress> {
    match memory_type {
        MemoryType::Available => Some(VIRTUAL_PHYSICAL_BASE),
        MemoryType::Reserved => None,
        MemoryType::KernelCode => Some(KERNEL_MAPPING_OFFSET),
        MemoryType::KernelStack => Some(
            KERNEL_STACK_MAPPING_OFFSET
                - smallest_address(&[MemoryType::KernelStack], memory_map).ok()?,
        ),
        MemoryType::KernelData | MemoryType::AcpiData => Some(
            VIRTUAL_DATA_BASE
                - smallest_address(&[MemoryType::KernelData, MemoryType::AcpiData], memory_map)
                    .ok()?,
        ),
    }
}
