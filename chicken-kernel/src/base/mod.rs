use chicken_util::BootInfo;

use crate::{
    base::{
        interrupts::idt,
        io::timer::{pit::PIT, Timer},
    },
    println,
};

mod acpi;
pub(crate) mod gdt;
pub(crate) mod interrupts;
pub(crate) mod io;
pub(crate) mod msr;

pub(super) fn set_up(boot_info: &BootInfo) {
    gdt::initialize();
    println!("kernel: Set up gdt.");
    idt::initialize();
    println!("kernel: Set up idt.");
    io::initialize(boot_info);
    println!(
        "kernel: Set up io, pit frequency: {}.",
        PIT.lock().frequency()
    );
}
