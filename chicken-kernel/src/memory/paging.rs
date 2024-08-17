use core::{
    arch::asm,
    error::Error,
    fmt::{Debug, Display, Formatter},
    ptr,
};

use chicken_util::{BootInfo, memory::{
    MemoryType
    , paging::{
        KERNEL_MAPPING_OFFSET,
        KERNEL_STACK_MAPPING_OFFSET, manager::{PageFrameAllocator, PageTableManager}, PageEntryFlags, PageTable,
    }, PhysicalAddress,
}, PAGE_SIZE};

use crate::{
    base::msr::Efer,
    memory::pmm::{BitMapAllocator, PageFrameAllocatorError},
};

const VIRTUAL_PHYSICAL_BASE: u64 = 0;
const VIRTUAL_DATA_BASE: u64 = 0xFFFF_FFFF_7000_0000;
/// Function to set up custom paging scheme. Returns virtual address of page manager level 4 table.
// New setup:
// 0xffff'ffff'ffff'ffff   --+ <- End of virtual address space
//                           |
//                           |
//                           |
//                           |
// 0xffff'ffff'8000'0000   --+ <- Kernel code and data segment (Higher half kernel)
//                           |    Maps to the physical memory containing the kernel image
//                           |
// 0xffff'ffff'7000'0000   --+ <- Kernel data (Contains boot info and memory map)
//                           |    Maps to the physical memory containing kernel data
//                           |
// 0xffff'ffff'6000'0000   --+ <- Kernel stack
//                           |    Maps to the stack pages in physical memory
//                           |
//                           |
//                           |
// 0xffff'8000'0000'0000   --+ <- Direct-mapped physical memory
//                           |    Every physical address has a corresponding virtual address
//                           |
//                           |
// 0x0000'0000'0000'0000   --+ <- Start of virtual address space
pub(super) fn setup(
    mut frame_allocator: BitMapAllocator,
    boot_info: BootInfo,
) -> Result<PhysicalAddress, PagingError> {
    let memory_map = boot_info.memory_map;
    // Allocate and clear a new PML4 page
    let pml4_addr = frame_allocator.request_page().map_err(PagingError::from)?;
    if (pml4_addr as usize) % align_of::<PageTable>() != 0 {
        return Err(PagingError::Pml4PointerMisaligned);
    }
    let pml4_table = pml4_addr as *mut PageTable;
    unsafe { ptr::write_bytes(pml4_table, 0, 1) };

    let mut manager: PageTableManager<BitMapAllocator, PageFrameAllocatorError> =
        PageTableManager::new(pml4_table, frame_allocator);
    let smallest_addr = |desc_type: MemoryType| {
        memory_map
            .descriptors()
            .iter()
            .filter(|desc| desc.r#type == desc_type)
            .map(|desc| desc.phys_start)
            .min()
            .ok_or(PagingError::InvalidMemoryMap)
    };
    let smallest_kernel_stack_addr = smallest_addr(MemoryType::KernelStack)?;
    let smallest_kernel_data_addr = smallest_addr(MemoryType::KernelData)?;

    memory_map.descriptors().iter().try_for_each(|desc| {
        let (virtual_base, physical_base, page_entry_flags) = match desc.r#type {
            MemoryType::Available | MemoryType::LoaderPageTables => (
                VIRTUAL_PHYSICAL_BASE,
                desc.phys_start,
                PageEntryFlags::default_nx(),
            ),
            // don't map reserved memory
            MemoryType::Reserved => return Ok::<(), PagingError>(()),
            MemoryType::KernelCode => (
                KERNEL_MAPPING_OFFSET,
                desc.phys_start,
                PageEntryFlags::default(),
            ),
            MemoryType::KernelStack => (
                KERNEL_STACK_MAPPING_OFFSET,
                desc.phys_start - smallest_kernel_stack_addr,
                PageEntryFlags::default_nx(),
            ),
            MemoryType::KernelData => (
                VIRTUAL_DATA_BASE,
                desc.phys_start - smallest_kernel_data_addr,
                PageEntryFlags::default_nx(),
            ),
        };

        for page in 0..desc.num_pages {
            let physical_address = desc.phys_start + page * PAGE_SIZE as u64;
            let virtual_address = virtual_base + physical_base + page * PAGE_SIZE as u64;

            manager
                .map_memory(virtual_address, physical_address, page_entry_flags)
                .map_err(PagingError::from)?;
        }

        Ok(())
    })?;

    let framebuffer_metadata = boot_info.framebuffer_metadata;
    // identity map framebuffer
    let fb_base_address = framebuffer_metadata.base;
    let fb_num_pages = (framebuffer_metadata.size + PAGE_SIZE - 1) / PAGE_SIZE;

    for page in 0..fb_num_pages {
        let address = fb_base_address + (page * PAGE_SIZE) as u64;
        manager.map_memory(address, address, PageEntryFlags::default_nx()).map_err(PagingError::from)?;
    }

    // free reserved LoaderPageTables frames
    let frame_allocator = manager.frame_allocator();
    memory_map
        .descriptors()
        .iter()
        .filter(|desc| desc.r#type == MemoryType::LoaderPageTables)
        .try_for_each(|desc| {
            frame_allocator.free_reserved_frames(desc.phys_start, desc.num_pages as usize)
        })
        .map_err(PagingError::from)?;

    // enable no-execute feature if available
    if let Some(mut efer) = Efer::read() {
        efer.insert(Efer::NXE);
        efer.write();
    }

    Ok(pml4_addr)
}

/// Note: technically this only switches to a custom page table, since paging has already been enabled by uefi.
pub(super) fn enable(pml4_address: PhysicalAddress) {
    unsafe {
        asm!("mov cr3, {}", in(reg) pml4_address);
    }
}

#[derive(Copy, Clone)]
pub(in crate::memory) enum PagingError {
    PhysicalAllocationFailed(PageFrameAllocatorError),
    Pml4PointerMisaligned,
    InvalidMemoryMap,
}

impl Debug for PagingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            PagingError::PhysicalAllocationFailed(allocator_err) => write!(
                f,
                "Paging Error: Physical Frame Allocation Failed: {}.",
                allocator_err
            ),
            PagingError::Pml4PointerMisaligned => {
                write!(f, "Paging Error: Page Map Level 4 pointer is misaligned.")
            }
            PagingError::InvalidMemoryMap => write!(f, "Paging Error: Invalid memory map."),
        }
    }
}

impl Display for PagingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for PagingError {}

impl From<PageFrameAllocatorError> for PagingError {
    fn from(value: PageFrameAllocatorError) -> Self {
        Self::PhysicalAllocationFailed(value)
    }
}