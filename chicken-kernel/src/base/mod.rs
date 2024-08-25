use chicken_util::BootInfo;

use crate::{
    base::{
        acpi::madt::{
            entry::{InterruptSourceOverride, IOApic, LApic, LApicNmi},
            Madt,
        },
        interrupts::idt,
    },
    println,
};

mod acpi;
mod gdt;
pub(crate) mod interrupts;
pub(crate) mod msr;

pub(super) fn setup(boot_info: &BootInfo) {
    gdt::initialize();

    let madt = unsafe { Madt::get(boot_info).as_ref().unwrap() };
    println!("successfully, parsed madt");

    let overrides = madt.parse_entries::<InterruptSourceOverride>();
    for entry in overrides.iter() {
        println!("{:?}", entry);
    }

    let ioapics = madt.parse_entries::<IOApic>();
    println!("ioapics: {:?}", ioapics);

    let lapics = madt.parse_entries::<LApic>();
    for entry in lapics.iter() {
        println!("{:?}", entry);
    }

    let lapic_nmis = madt.parse_entries::<LApicNmi>();
    for entry in lapic_nmis.iter() {
        println!("{:?}", entry);
    }

    idt::initialize();
    interrupts::enable();
}
