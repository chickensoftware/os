#![feature(vec_into_raw_parts)]
#![no_std]
#![no_main]

extern crate alloc;
use alloc::{format, vec::Vec};
use core::{arch::asm, fmt::Write, panic::PanicInfo};

use log::error;
use qemu_print::qemu_println;
use uefi::{
    entry,
    Handle,
    proto::console::text::{Color, Output},
    Status, table::{Boot, boot::MemoryType, Runtime, SystemTable},
};

use chicken_util::{
    BootInfo,
    graphics::font::Font,
    memory::{paging::KERNEL_MAPPING_OFFSET, pmm::PageFrameAllocator}, PAGE_SIZE,
};

use crate::memory::{allocate_boot_info, allocate_kernel_stack, KernelInfo, set_up_address_space};

mod file;
mod graphics;
mod memory;

const KERNEL_FILE_NAME: &str = "kernel.elf";
const FONT_FILE_NAME: &str = "font.psf";

const KERNEL_STACK_SIZE: usize = 1024 * 1024; // 1 MB

/// Entry point of uefi application (bootloader)
#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut system_table).unwrap();
    let stdout = system_table.stdout();

    stdout
        .clear()
        .expect("Standard Output Protocol Error: Could not clear screen for stdout.");

    println!("CHICKEN OS", stdout, Color::Yellow);

    println!(stdout);

    // get kernel file data in bytes
    print!("boot: Egg-quiring kernel file from filesystem", stdout);
    let file = file::get_file_data(image_handle, system_table.boot_services(), KERNEL_FILE_NAME);
    let stdout = system_table.stdout();

    validate!(file, stdout);
    let file = file.unwrap();
    println!(
        format!("boot: Kernel file size: {} bytes", file.len()).as_str(),
        stdout
    );

    // allocate pages and load kernel file data into memory
    print!("boot: Loading kernel image into memory", stdout);
    let kernel_elf = file::parse_elf(file, system_table.boot_services());
    let stdout = system_table.stdout();

    validate!(kernel_elf, stdout);
    let (kernel_entry_addr, kernel_file_start_addr, kernel_file_num_pages) = kernel_elf.unwrap();
    println!(
        format!("boot: Kernel entry address: {:#x}", kernel_entry_addr).as_str(),
        stdout
    );

    // allocate pages for kernel stack
    print!(
        format!(
            "boot: Allocating memory for kernel stack ({} MB)",
            KERNEL_STACK_SIZE / (1024 * 1024)
        )
        .as_str(),
        stdout
    );

    let kernel_stack_info = allocate_kernel_stack(system_table.boot_services());
    let stdout = system_table.stdout();

    validate!(kernel_stack_info, stdout);
    let (kernel_stack_start_addr, kernel_stack_num_pages) = kernel_stack_info.unwrap();

    print!(
        "boot: Allocating memory for kernel boot information",
        stdout
    );

    let kernel_boot_info = allocate_boot_info(system_table.boot_services());
    let stdout = system_table.stdout();

    validate!(kernel_boot_info, stdout);
    let (kernel_boot_info_addr, mmap_descriptors) = kernel_boot_info.unwrap();

    print!("boot: Allocating memory for framebuffer font", stdout);

    let font_info = graphics::load_font(image_handle, system_table.boot_services());
    let stdout = system_table.stdout();

    validate!(font_info, stdout);
    let (font_header, font_buffer_addr, font_buffer_size) = font_info.unwrap();

    print!("boot: Retrieving root system descriptor pointer", stdout);

    let rsdp = memory::get_rsdp(&system_table);
    let stdout = system_table.stdout();

    validate!(rsdp, stdout);
    let rsdp = rsdp.unwrap();

    // Exit boot services and handover control to kernel
    println!(
        "boot: Setting up address space and dropping boot services",
        stdout
    );
    println!("boot: Chicken OS is hatching...", stdout);
    print_chicken(stdout);

    // switch to graphics mode
    let fb_metadata = graphics::initialize_framebuffer(system_table.boot_services());
    let stdout = system_table.stdout();

    // text mode may still be enabled if operation failed
    validate!(fb_metadata, stdout);
    let fb_metadata = fb_metadata.unwrap();
    let kernel_info = KernelInfo {
        kernel_code_address: kernel_file_start_addr,
        kernel_code_page_count: kernel_file_num_pages,
        kernel_stack_address: kernel_stack_start_addr,
        kernel_stack_page_count: kernel_stack_num_pages,
        kernel_boot_info_address: kernel_boot_info_addr,
    };

    let (_runtime, mmap) = drop_boot_services(system_table, mmap_descriptors, &kernel_info);

    // set up basic memory management and the virtual address space for the higher half kernel
    let address_space_info = set_up_address_space(&mmap, kernel_info);

    // note: validate is no longer available after switching to graphics mode
    let (pml4_address, virtual_rsp, kernel_boot_info_virtual_address, pmm) =
        address_space_info.unwrap();

    let boot_info = unsafe { &mut *(kernel_boot_info_addr as *mut BootInfo) };
    boot_info.memory_map = mmap;
    boot_info.framebuffer_metadata = fb_metadata;
    boot_info.font = Font {
        header: font_header,
        glyph_buffer_address: font_buffer_addr as *const u8,
        glyph_buffer_size: font_buffer_size,
    };
    boot_info.pmm_address = &pmm as *const PageFrameAllocator as u64;
    boot_info.rsdp = rsdp;

    unsafe {
        asm!(
        // boot info address
        "mov rdi, {0}",
        // switch to custom paging
        "mov cr3, {2}",
        // set stack pointer to kernel stack top
        "mov rsp, {1}",
        // jump to kernel entry
        "jmp {3}",
        in(reg) kernel_boot_info_virtual_address,
        in(reg) virtual_rsp,
        in(reg) pml4_address,
        in(reg) kernel_entry_addr
        );
    }

    // should never happen, because kernel diverges
    Status::ABORTED
}

type ChickenMemoryMap = chicken_util::memory::MemoryMap;
type ChickenMemoryDescriptor = chicken_util::memory::MemoryDescriptor;
type ChickenMemoryType = chicken_util::memory::MemoryType;

/// Drops boot services and returns converted memory map and runtime system table
fn drop_boot_services(
    system_table: SystemTable<Boot>,
    mut descriptors: Vec<ChickenMemoryDescriptor>,
    kernel_info: &KernelInfo,
) -> (SystemTable<Runtime>, ChickenMemoryMap) {
    // drop boot services
    let (runtime, uefi_mmap) = unsafe { system_table.exit_boot_services(MemoryType::LOADER_DATA) };
    let mut first_addr = u64::MAX;
    let mut first_available_addr = u64::MAX;
    let mut last_addr = u64::MIN;
    let mut last_available_addr = u64::MIN;
    // collect available memory descriptors (convert uefi mmap to chicken mmap)
    uefi_mmap.entries().for_each(|descriptor| {
        let phys_end = descriptor.phys_start + descriptor.page_count * PAGE_SIZE as u64;

        if descriptor.phys_start < first_addr {
            first_addr = descriptor.phys_start;
        }

        if descriptor.phys_start != 0x0
            && matches!(
                descriptor.ty,
                MemoryType::CONVENTIONAL
                    | MemoryType::BOOT_SERVICES_CODE
                    | MemoryType::BOOT_SERVICES_DATA
            )
        {
            if descriptor.phys_start < first_available_addr {
                first_available_addr = descriptor.phys_start;
            }
            if phys_end > last_available_addr {
                last_available_addr = phys_end;
            }
        }

        if phys_end > last_addr {
            last_addr = phys_end;
        }

        let r#type = if descriptor.phys_start < 0x1000 {
            ChickenMemoryType::Reserved
        }
        // mark kernel file as kernel code
        else if descriptor.phys_start <= kernel_info.kernel_code_address
            && phys_end
                >= kernel_info.kernel_code_address
                    + (kernel_info.kernel_code_page_count * PAGE_SIZE) as u64
        {
            ChickenMemoryType::KernelCode
        }
        // mark stack as kernel stack
        else if descriptor.phys_start <= kernel_info.kernel_stack_address
            && phys_end
                >= kernel_info.kernel_stack_address
                    + (kernel_info.kernel_stack_page_count * PAGE_SIZE) as u64
        {
            ChickenMemoryType::KernelStack
        } else {
            // Determine the core memory type based on the UEFI memory type
            match descriptor.ty {
                MemoryType::CONVENTIONAL
                | MemoryType::BOOT_SERVICES_DATA
                | MemoryType::BOOT_SERVICES_CODE => ChickenMemoryType::Available,
                // mark mmap data, boot info, font data, ... as kernel data
                MemoryType::LOADER_DATA => ChickenMemoryType::KernelData,
                MemoryType::ACPI_RECLAIM | MemoryType::ACPI_NON_VOLATILE  => ChickenMemoryType::AcpiData,
                _ => ChickenMemoryType::Reserved,
            }
        };

        descriptors.push(ChickenMemoryDescriptor {
            phys_start: descriptor.phys_start,
            phys_end,
            num_pages: descriptor.page_count,
            r#type,
        });
    });

    let (ptr, len, _cap) = descriptors.into_raw_parts();
    (
        runtime,
        ChickenMemoryMap {
            descriptors: ptr as *mut ChickenMemoryDescriptor,
            descriptors_len: len as u64,
            first_addr,
            first_available_addr,
            last_addr,
            last_available_addr,
        },
    )
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occurred: \n{:#?}", info);
    qemu_println!("Panic occurred: \n{:#?}", info);
    loop {}
}

// print chicken :)
fn print_chicken(stdout: &mut Output) {
    println!("   \\\\", stdout);
    println!("   (o>", stdout);
    println!("\\\\_//)", stdout);
    println!(" \\_/_)", stdout);
    println!("   _|_", stdout);
    println!(stdout);
}

#[macro_export]
macro_rules! println {
    ($s:expr, $stdout:expr) => {
        print!($s, $stdout);
        println!($stdout);
    };
    ($s:expr, $stdout:expr, $color:expr) => {
        $stdout
            .set_color($color, Color::Black)
            .expect("Standard Output Protocol Error: Could not set color.");
        print!($s, $stdout);
        println!($stdout);
        $stdout
            .set_color(Color::White, Color::Black)
            .expect("Standard Output Protocol Error: Could not set color.");
    };
    ($stdout:expr) => {
        $stdout.write_char('\n').expect(
            "Standard Output Protocol Error: Could not write next line character to screen.",
        );
    };
}

#[macro_export]
macro_rules! print {
    ($s:expr, $stdout:expr) => {
        $stdout
            .write_str($s)
            .expect("Standard Output Protocol Error: Could not write text to screen.");
    };
}

#[macro_export]
macro_rules! validate {
    ($result:expr, $stdout:expr) => {
        if let Err(error_message) = $result {
            println!(" [error] ", $stdout, Color::Red);
            println!(error_message.as_str(), $stdout);
            return Status::PROTOCOL_ERROR;
        }

        println!(" [success] ", $stdout, Color::Green);
    };
}
