use chicken_util::BootInfo;

use crate::base::interrupts::idt;

mod acpi;
mod gdt;
mod io;

pub(crate) mod interrupts;
pub(crate) mod msr;

pub(super) fn set_up(boot_info: &BootInfo) {
    gdt::initialize();
    idt::initialize();
    io::initialize(boot_info);
    interrupts::enable();
}
