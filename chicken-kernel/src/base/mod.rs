use crate::base::interrupts::idt;

mod gdt;
pub(crate) mod interrupts;

pub(super) fn setup() {
    gdt::initialize();
    idt::initialize();
    interrupts::enable();

}

