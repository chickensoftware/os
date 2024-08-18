use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::slice;

use chicken_util::{
    memory::{PhysicalAddress, VirtualAddress},
    PAGE_SIZE,
};
use goblin::{elf::Elf, elf32::program_header::PT_LOAD};
use uefi::{fs::FileSystem, prelude::BootServices, table::boot::AllocateType, CString16, Handle};

use crate::memory::KERNEL_CODE;

/// Gets data of a file from the filesystem
pub(super) fn get_file_data(
    image_handle: Handle,
    boot_services: &BootServices,
    filename: &str,
) -> Result<Vec<u8>, String> {
    let mut file_system = FileSystem::new(
        boot_services
            .get_image_file_system(image_handle)
            .map_err(|_| "Cannot get filesystem protocol".to_string())?,
    );
    file_system
        .read(
            CString16::try_from(filename)
                .map_err(|_| format!("Invalid filename: {filename}"))?
                .as_ref(),
        )
        .map_err(|_| format!("Unable to read file with name: {filename}"))
}

/// Allocates the file data in memory and returns entry point, file base address and number of pages
pub(super) fn parse_elf(
    data: Vec<u8>,
    boot_services: &BootServices,
) -> Result<(VirtualAddress, PhysicalAddress, usize), String> {
    let data = data.as_slice();
    let elf =
        Elf::parse(data).map_err(|_| "Unable to parse file to elf!".to_string())?;

    let mut dest_start = u64::MAX;
    let mut dest_end = 0;

    if !elf.is_64 {
        return Err("Invalid elf format.".to_string());
    }

    // set up range of memory needed to be allocated
    for pheader in elf.program_headers.iter() {
        // skip non-load segments (e.g.: dynamic linking info)
        if pheader.p_type != PT_LOAD {
            continue;
        }

        dest_start = dest_start.min(pheader.p_paddr);
        dest_end = dest_end.max(pheader.p_paddr + pheader.p_memsz);
    }

    let num_pages = (dest_end as usize - dest_start as usize + PAGE_SIZE - 1) / PAGE_SIZE;

    // allocate file data
    boot_services
        .allocate_pages(AllocateType::Address(dest_start), KERNEL_CODE, num_pages)
        .map_err(|error| format!("Could not allocate pages for elf file: {}.", error))?;

    // Copy program segments of elf into memory
    for pheader in elf.program_headers.iter() {
        // skip non-load segments (e.g.: dynamic linking info)
        if pheader.p_type != PT_LOAD {
            continue;
        }
        let base_address = pheader.p_paddr;
        let offset = pheader.p_offset as usize;
        let size_in_file = pheader.p_filesz as usize;
        let size_in_memory = pheader.p_memsz as usize;

        let dest = unsafe { slice::from_raw_parts_mut(base_address as *mut u8, size_in_memory) };
        dest[..size_in_file].copy_from_slice(&data[offset..offset + size_in_file]);
        dest[size_in_file..].fill(0);
    }

    Ok((elf.entry, dest_start, num_pages))
}
