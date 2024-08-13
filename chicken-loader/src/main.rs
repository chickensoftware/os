#![feature(vec_into_raw_parts)]
#![no_std]
#![no_main]

extern crate alloc;
use alloc::{format, vec::Vec};
use core::{arch::asm, fmt::Write, mem, panic::PanicInfo};

use log::error;
use qemu_print::qemu_println;
use uefi::{
    entry,
    Handle,
    proto::console::text::{Color, Output},
    Status, table::{Boot, boot::MemoryType, Runtime, SystemTable},
};

use chicken_util::{BootInfo, PAGE_SIZE, memory::paging::KERNEL_MAPPING_OFFSET};

use crate::memory::{
    allocate_boot_info, allocate_kernel_stack, KERNEL_CODE, KERNEL_DATA, KERNEL_STACK,
    KernelInfo, set_up_address_space,
};

mod file;
mod memory;

const KERNEL_FILE_NAME: &str = "kernel.elf";

const KERNEL_STACK_SIZE: usize = 1024 * 1024; // 1 MB

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

macro_rules! print {
    ($s:expr, $stdout:expr) => {
        $stdout
            .write_str($s)
            .expect("Standard Output Protocol Error: Could not write text to screen.");
    };
}

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
    let kernel_elf = file::load_elf(file.as_slice(), system_table.boot_services());
    let stdout = system_table.stdout();

    validate!(kernel_elf, stdout);
    let (kernel_entry_addr, kernel_file_start_addr, kernel_file_num_pages) = kernel_elf.unwrap();
    println!(
        format!("boot: Kernel entry address: {:#x}", kernel_entry_addr).as_str(),
        stdout
    );

    // setup kernel start function
    let _start: extern "sysv64" fn(&BootInfo) -> ! = unsafe { mem::transmute(kernel_entry_addr) };

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

    print!(
        "boot: Setting up address space for higher half kernel",
        stdout
    );

    // setup paging + virtual address space for higher half kernel
    let address_space_info = set_up_address_space(
        &mut system_table,
        KernelInfo {
            kernel_file_start_addr,
            kernel_file_num_pages,
            kernel_stack_start_addr,
            kernel_stack_num_pages,
            kernel_boot_info_addr,
        },
    );

    let stdout = system_table.stdout();

    validate!(address_space_info, stdout);
    let (pml4_addr, kernel_stack_addr, kernel_boot_info_addr) = address_space_info.unwrap();

    // Exit boot services and handover control to kernel
    println!(
        "boot: Dropping boot services. Chicken OS is hatching...",
        stdout
    );
    print_chicken(stdout);

    let (_runtime, mmap) = drop_boot_services(system_table, mmap_descriptors);

    // switch to custom paging implementation and update stack pointer
    unsafe {
        asm!("mov cr3, {0}", in(reg) pml4_addr);
    }

    let boot_info = unsafe { &mut *(kernel_boot_info_addr as *mut BootInfo) };
    boot_info.memory_map = mmap;
    unsafe {
        asm!("mov rsp, {0}", in(reg) kernel_stack_addr);
    }
    // call kernel entry
    _start(boot_info);
}

type ChickenMemoryMap = chicken_util::memory::MemoryMap;
type ChickenMemoryDescriptor = chicken_util::memory::MemoryDescriptor;
type ChickenMemoryType = chicken_util::memory::MemoryType;

/// Drops boot services and returns converted memory map and runtime system table
fn drop_boot_services(
    system_table: SystemTable<Boot>,
    mut descriptors: Vec<ChickenMemoryDescriptor>,
) -> (SystemTable<Runtime>, ChickenMemoryMap) {
    // drop boot services
    let (runtime, uefi_mmap) = unsafe { system_table.exit_boot_services(MemoryType::LOADER_DATA) };

    let mut first_addr = u64::MAX;
    let mut last_addr = u64::MIN;
    let desc_start_addr = descriptors.as_ptr() as u64;
    let desc_end_addr = desc_start_addr + (descriptors.capacity() * size_of::<ChickenMemoryDescriptor>()) as u64;
    // collect available memory descriptors (convert uefi mmap to chicken mmap)
    uefi_mmap.entries().for_each(|descriptor| {
        let phys_end = descriptor.phys_start + descriptor.page_count * PAGE_SIZE as u64;

        if descriptor.phys_start < first_addr {
            first_addr = descriptor.phys_start;
        }
        if phys_end > last_addr {
            last_addr = phys_end
        };

        if descriptor.phys_start < 0x1000 {
            descriptors.push(ChickenMemoryDescriptor {
                phys_start: descriptor.phys_start,
                phys_end,
                num_pages: descriptor.page_count,
                r#type: ChickenMemoryType::Reserved,
            });
            return;
        }
        // mark mmap data as boot info
        else if descriptor.phys_start <= desc_start_addr && phys_end >= desc_end_addr {
            descriptors.push(ChickenMemoryDescriptor {
                phys_start: descriptor.phys_start,
                phys_end,
                num_pages: descriptor.page_count,
                r#type: ChickenMemoryType::KernelData,
            });
            return;
        }


        let r#type = match descriptor.ty {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::BOOT_SERVICES_CODE => ChickenMemoryType::Available,
            KERNEL_DATA => ChickenMemoryType::KernelData,
            KERNEL_STACK => ChickenMemoryType::KernelStack,
            KERNEL_CODE => ChickenMemoryType::KernelCode,

            _ => ChickenMemoryType::Reserved,
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
            last_addr,
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
