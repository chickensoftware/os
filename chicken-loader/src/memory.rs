use alloc::{
    format,
    string::{String, ToString},
};
use core::ptr;

use uefi::{
    prelude::BootServices,
    table::{
        boot::{AllocateType::AnyPages, MemoryType},
        Boot, SystemTable,
    },
};
use uefi::data_types::VirtualAddress;

use chicken_util::{
    memory::{paging::PageTable, PhysicalAddress},
    PAGE_SIZE,
};
use chicken_util::memory::paging::manager::{PageFrameAllocator, PageTableManager};

use crate::{KERNEL_MAPPING_OFFSET, KERNEL_STACK_SIZE};

pub(super) const KERNEL_CODE: MemoryType = MemoryType::custom(0x80000000);
pub(super) const KERNEL_STACK: MemoryType = MemoryType::custom(0x80000001);

#[derive(Copy, Clone, Debug)]
pub(super) struct KernelInfo {
    pub(super) kernel_file_start_addr: PhysicalAddress,
    pub(super) kernel_file_num_pages: usize,
    pub(super) kernel_stack_start_addr: PhysicalAddress,
    pub(super) kernel_stack_num_pages: usize,
}

/// Allocate pages for kernel stack. Returns physical address of allocated stack and amount of pages allocated.
pub(super) fn allocate_kernel_stack(bt: &BootServices) -> Result<(PhysicalAddress, usize), String> {
    let num_pages = KERNEL_STACK_SIZE / PAGE_SIZE;
    let start_addr = bt.allocate_pages(AnyPages, KERNEL_STACK, num_pages)
        .map_err(|_| {
            format!(
                "Could not allocate {} pages for the kernel stack.",
                num_pages
            )
        })?;
    Ok((start_addr, num_pages))
}

/// Sets up paging that includes mappings for higher half kernel and higher half stack. Returns address pointing to page table manager
pub(super) fn set_up_address_space(
    system_table: &mut SystemTable<Boot>,
    kernel_info: KernelInfo,
) -> Result<(PhysicalAddress, VirtualAddress), String> {
    let KernelInfo { kernel_file_start_addr, kernel_file_num_pages, kernel_stack_start_addr, kernel_stack_num_pages } = kernel_info;

    let pml4_addr = system_table
        .boot_services()
        .allocate_pages(AnyPages, MemoryType::LOADER_DATA, 1)
        .map_err(|_| "Could not allocate new page table".to_string())?;

    assert_eq!(
        (pml4_addr as usize) % align_of::<PageTable>(),
        0,
        "pml4 pointer is not aligned"
    );

    let pml4_table = pml4_addr as *mut PageTable;

    // zero out new table
    unsafe { ptr::write_bytes(pml4_table, 0, 1) };

    let page_frame_allocator = BootServiceWrapper(system_table.boot_services());

    let mut manager: PageTableManager<BootServiceWrapper, String> = PageTableManager::new(pml4_table, page_frame_allocator);

    // identity map physical address space
    let mmap = system_table
        .boot_services()
        .memory_map(MemoryType::LOADER_DATA)
        .map_err(|_| "Could not get memory map.".to_string())?;
    let first_addr = mmap
        .entries()
        .filter(|desc| matches!(desc.ty, MemoryType::CONVENTIONAL | MemoryType::BOOT_SERVICES_DATA | MemoryType::BOOT_SERVICES_CODE) && desc.phys_start > 0x0) // skip areas like 0x0
        .map(|desc| desc.phys_start)
        .min()
        .ok_or("Memory map is empty".to_string())?;
    let last_addr = mmap
        .entries()
        .filter(|desc| matches!(desc.ty, MemoryType::CONVENTIONAL | MemoryType::BOOT_SERVICES_DATA| MemoryType::BOOT_SERVICES_CODE))
        .map(|desc| desc.phys_start + PAGE_SIZE as u64 * desc.page_count)
        .max()
        .ok_or("Memory map is empty".to_string())?;
    let num_pages = (last_addr - first_addr) as usize / PAGE_SIZE;
    for page in 0..num_pages {
        let physical_address = (PAGE_SIZE * page) as u64 + first_addr;
        manager
            .map_memory(physical_address, physical_address)
            .map_err(|_| {
                format!(
                    "Could not identity map physical address: {:#x}",
                    physical_address
                )
            })?;
    }

    // map higher half kernel virtual addresses to physical kernel addresses
    for page in 0..kernel_file_num_pages {
        let physical_address = (PAGE_SIZE * page) as u64 + kernel_file_start_addr;
        let virtual_address = KERNEL_MAPPING_OFFSET + physical_address;
        manager
            .map_memory(virtual_address, physical_address)
            .map_err(|_| {
                format!(
                    "Could not map kernel physical address: {:#x} to higher half address: {:#x}",
                    physical_address, virtual_address
                )
            })?;
    }
    // map kernel stack directly behind kernel code (file)
    let kernel_stack_virtual_start_addr = (PAGE_SIZE * kernel_file_num_pages) as u64 + kernel_file_start_addr + KERNEL_MAPPING_OFFSET;
    for page in 0..kernel_stack_num_pages {
        let physical_address = (page * PAGE_SIZE) as u64 + kernel_stack_start_addr;
        let virtual_address = kernel_stack_virtual_start_addr + (page * PAGE_SIZE) as u64;
        manager
            .map_memory(virtual_address, physical_address)
            .map_err(|_| {
                format!(
                    "Could not map kernel stack physical address: {:#x} to higher half address: {:#x}",
                    physical_address, virtual_address
                )
            })?;
    }

    Ok((pml4_addr, kernel_stack_virtual_start_addr + KERNEL_STACK_SIZE as u64))
}

/// Wrapper for BootServices that allows PageFrameAllocator implementation
struct BootServiceWrapper<'a>(&'a BootServices);

impl<'a> PageFrameAllocator<'a, String> for BootServiceWrapper<'a> {
    fn request_page(&mut self) -> Result<PhysicalAddress, String> {
        self.0.allocate_pages(AnyPages, MemoryType::LOADER_DATA, 1).map_err(|_| "Could not allocate page for page table manager.".to_string())
    }
}