use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::ptr;

use chicken_util::{
    memory::{
        paging::{
            manager::OwnedPageTableManager, PageEntryFlags, PageTable, KERNEL_STACK_MAPPING_OFFSET,
        },
        pmm::{PageFrameAllocator, PageFrameAllocatorError},
        PhysicalAddress, VirtualAddress,
    },
    PAGE_SIZE,
};
use uefi::{
    prelude::BootServices,
    table::{
        boot::{AllocateType::AnyPages, MemoryType},
        cfg::{ACPI2_GUID, ACPI_GUID},
        Boot, SystemTable,
    },
};

use crate::{ChickenMemoryDescriptor, ChickenMemoryMap, KERNEL_MAPPING_OFFSET, KERNEL_STACK_SIZE};

#[derive(Copy, Clone, Debug)]
pub(super) struct KernelInfo {
    pub(super) kernel_code_address: PhysicalAddress,
    pub(super) kernel_code_page_count: usize,
    pub(super) kernel_stack_address: PhysicalAddress,
    pub(super) kernel_stack_page_count: usize,
    pub(super) kernel_boot_info_address: PhysicalAddress,
}

/// Allocate pages for kernel stack. Returns physical address of allocated stack and amount of pages allocated.
pub(super) fn allocate_kernel_stack(bt: &BootServices) -> Result<(PhysicalAddress, usize), String> {
    let num_pages = (KERNEL_STACK_SIZE + PAGE_SIZE - 1) / PAGE_SIZE + 1; // + 1 to ENSURE sufficient size
    let start_addr = bt
        .allocate_pages(AnyPages, MemoryType::LOADER_DATA, num_pages)
        .map_err(|_| {
            format!(
                "Could not allocate {} pages for the kernel stack.",
                num_pages
            )
        })?;
    Ok((start_addr, num_pages))
}

/// Allocate a single page to store the boot information in
pub(super) fn allocate_boot_info(
    bt: &BootServices,
) -> Result<(PhysicalAddress, Vec<ChickenMemoryDescriptor>), String> {
    let boot_info_addr = bt
        .allocate_pages(AnyPages, MemoryType::LOADER_DATA, 1)
        .map_err(|_| "Could not allocate page for kernel boot information.".to_string())?;

    // get uefi mmap meta data to allocate enough later for custom memory map in `drop_boot_services`
    let uefi_memory_map_meta = bt
        .memory_map(MemoryType::LOADER_DATA)
        .map_err(|error| format!("Could not get uefi memory map: {error}"))?
        .as_raw()
        .1;

    // allocate enough memory for the map
    let sufficient_memory_map_size = uefi_memory_map_meta.map_size;

    // allocate descriptors in memory
    let descriptors = Vec::with_capacity(sufficient_memory_map_size);

    Ok((boot_info_addr, descriptors))
}

/// Sets up paging that includes mappings for higher half kernel and higher half stack. Returns address pointing to page table manager, stack pointer, boot info as well as the initialized physical memory manager.
// note: currently all page entry flags are set to the default value, may change to set up nx capability in bootloader already
pub(super) fn set_up_address_space(
    memory_map: &ChickenMemoryMap,
    kernel_info: KernelInfo,
) -> Result<
    (
        PhysicalAddress,
        VirtualAddress,
        VirtualAddress,
        PageFrameAllocator,
    ),
    PageFrameAllocatorError,
> {
    let KernelInfo {
        kernel_code_address,
        kernel_code_page_count,
        kernel_stack_address,
        kernel_stack_page_count,
        kernel_boot_info_address,
    } = kernel_info;

    // set up physical memory manager
    let mut pmm = PageFrameAllocator::try_new(*memory_map)?;
    let pml4_addr = pmm.request_page()?;
    assert_eq!(
        (pml4_addr as usize) % align_of::<PageTable>(),
        0,
        "pml4 pointer is not aligned"
    );

    let pml4_table = pml4_addr as *mut PageTable;
    // zero out new table
    unsafe { ptr::write_bytes(pml4_table, 0, 1) };

    let mut manager = OwnedPageTableManager::new(pml4_table, pmm);
    for desc in memory_map.descriptors().iter() {
        for page in 0..desc.num_pages {
            let physical_address = PAGE_SIZE as u64 * page + desc.phys_start;
            manager.map_memory(
                physical_address,
                physical_address,
                PageEntryFlags::default(),
            )?;
        }
    }

    // map higher half kernel virtual addresses to physical kernel addresses
    for page in 0..kernel_code_page_count {
        let physical_address = ((PAGE_SIZE * page) as u64) + kernel_code_address;
        let virtual_address = KERNEL_MAPPING_OFFSET + physical_address;
        manager.map_memory(virtual_address, physical_address, PageEntryFlags::default())?;
    }

    // map kernel stack to higher half address
    for page in 0..kernel_stack_page_count {
        let physical_address = ((page * PAGE_SIZE) as u64) + kernel_stack_address;
        let virtual_address = KERNEL_STACK_MAPPING_OFFSET + (page * PAGE_SIZE) as u64;
        manager.map_memory(virtual_address, physical_address, PageEntryFlags::default())?;
    }

    // map boot info page to higher half right above stack
    let kernel_boot_info_virtual_address =
        KERNEL_STACK_MAPPING_OFFSET + (kernel_stack_page_count * PAGE_SIZE) as u64;
    manager.map_memory(
        kernel_boot_info_virtual_address,
        kernel_boot_info_address,
        PageEntryFlags::default(),
    )?;

    let pmm: PageFrameAllocator = manager.into();
    Ok((
        pml4_addr,
        KERNEL_STACK_MAPPING_OFFSET + KERNEL_STACK_SIZE as u64,
        kernel_boot_info_virtual_address,
        pmm,
    ))
}

/// Get root system descriptor pointer address
pub(super) fn get_rsdp(st: &SystemTable<Boot>) -> Result<u64, String> {
    let mut config_entries = st.config_table().iter();
    // look for an ACPI2 RSDP first
    let acpi2_rsdp = config_entries.find(|entry| matches!(entry.guid, ACPI2_GUID));
    // if ACPI2 isn't found, use ACPI1 RSDP instead
    let rsdp = acpi2_rsdp.or_else(|| config_entries.find(|entry| matches!(entry.guid, ACPI_GUID)));
    rsdp.map(|entry| entry.address as u64)
        .ok_or("Could not find RSDP.".to_string())
}
