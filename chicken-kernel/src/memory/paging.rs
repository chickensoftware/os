use core::{
    arch::asm,
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
    ptr,
};

use chicken_util::{
    BootInfo,
    graphics::font::Font,
    memory::{
        MemoryDescriptor,
        MemoryMap,
        MemoryType, paging::{
            KERNEL_MAPPING_OFFSET, KERNEL_STACK_MAPPING_OFFSET, manager::PageTableManager, PageEntryFlags,
            PageTable,
        }, PhysicalAddress, pmm::{PageFrameAllocator, PageFrameAllocatorError},
    }, PAGE_SIZE,
};

use crate::{
    base::msr::Efer,
    scheduling::spin::{Guard, SpinLock},
};

pub(in crate::memory) static PTM: GlobalPageTableManager = GlobalPageTableManager::new();

const VIRTUAL_PHYSICAL_BASE: u64 = 0xFFFF_8000_0000_0000;
const VIRTUAL_DATA_BASE: u64 = 0xFFFF_FFFF_7000_0000;
#[derive(Debug)]
pub(in crate::memory) struct GlobalPageTableManager {
    inner: SpinLock<OnceCell<PageTableManager<'static>>>,
}

unsafe impl Send for GlobalPageTableManager {}
unsafe impl Sync for GlobalPageTableManager {}

impl GlobalPageTableManager {
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(OnceCell::new()),
        }
    }

    pub(super) fn init(page_table_manager: PageTableManager<'static>) {
        let ptm = PTM.inner.lock();
        ptm.get_or_init(|| page_table_manager);
    }
    pub(in crate::memory) fn lock(&self) -> Guard<OnceCell<PageTableManager<'static>>> {
        self.inner.lock()
    }
}

/// Function to set up custom paging scheme. Returns virtual address of page manager level 4 table. Also returns boot info with updated usable virtual addresses
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
pub(super) fn setup<'a>(
    mut frame_allocator: PageFrameAllocator<'a>,
    old_boot_info: &BootInfo,
) -> Result<(PageTableManager<'a>, BootInfo), PagingError> {
    let memory_map = old_boot_info.memory_map;
    // Allocate and clear a new PML4 page
    let pml4_addr = frame_allocator.request_page().map_err(PagingError::from)?;
    if (pml4_addr as usize) % align_of::<PageTable>() != 0 {
        return Err(PagingError::Pml4PointerMisaligned);
    }
    let pml4_table = pml4_addr as *mut PageTable;
    unsafe { ptr::write_bytes(pml4_table, 0, 1) };

    let mut manager: PageTableManager = PageTableManager::new(pml4_table, frame_allocator);
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
            MemoryType::Available => (
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

    let framebuffer_metadata = old_boot_info.framebuffer_metadata;
    // identity map framebuffer
    let fb_base_address = framebuffer_metadata.base;
    let fb_num_pages = (framebuffer_metadata.size + PAGE_SIZE - 1) / PAGE_SIZE;

    for page in 0..fb_num_pages {
        let address = fb_base_address + (page * PAGE_SIZE) as u64;
        manager
            .map_memory(address, address, PageEntryFlags::default_nx())
            .map_err(PagingError::from)?;
    }

    // enable no-execute feature if available
    if let Some(mut efer) = Efer::read() {
        efer.insert(Efer::NXE);
        efer.write();
    }

    let old_font = old_boot_info.font;
    // update boot info
    let boot_info = BootInfo {
        memory_map: MemoryMap {
            descriptors: (memory_map.descriptors as u64 - smallest_kernel_data_addr
                + VIRTUAL_DATA_BASE) as *mut MemoryDescriptor,
            ..memory_map
        },
        font: Font {
            glyph_buffer_address: (old_font.glyph_buffer_address as u64 - smallest_kernel_data_addr
                + VIRTUAL_DATA_BASE) as *const u8,
            ..old_font
        },
        ..*old_boot_info
    };

    // update pmm memory map and bit map pointer to use mapped virtual addresses
    let old_pmm_bit_map_buffer_address = manager.pmm().bit_map_buffer_address();

    unsafe {
        manager.pmm().update(
            old_pmm_bit_map_buffer_address + VIRTUAL_PHYSICAL_BASE,
            memory_map.descriptors as u64 - smallest_kernel_data_addr + VIRTUAL_DATA_BASE,
        );
    }

    // update page table addresses to virtual ones
    unsafe {
        manager.update(VIRTUAL_PHYSICAL_BASE);
    }

    // todo: free reserved loader page tables, since they are no longer needed

    Ok((manager, boot_info))
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
    GlobalPageTableManagerUninitialized,
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
            PagingError::GlobalPageTableManagerUninitialized => write!(
                f,
                "Paging Error: Global page table manager has not been initialized."
            ),
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
