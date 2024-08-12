use crate::base::interrupts::idt;

mod gdt;
pub(crate) mod interrupts;

pub(super) fn setup() {
    gdt::initialize();
    idt::initialize();
    interrupts::enable();

    // triggering divide by zero exception for testing
    unsafe { core::arch::asm!("mov dx, 0", "div dx") };
}

