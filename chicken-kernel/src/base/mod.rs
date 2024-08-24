use chicken_util::BootInfo;

use crate::{
    base::{
        acpi::{
            rsd::Rsd,
            sdt::get_xsdt,
        },
        interrupts::idt,
    }
    ,
    println,
};

mod acpi;
mod gdt;
pub(crate) mod interrupts;
pub(crate) mod msr;

pub(super) fn setup(boot_info: &BootInfo) {
    gdt::initialize();

    let rsd = Rsd::get(boot_info.rsdp).unwrap();
    let xsdt = get_xsdt(rsd.rsd_table_address(), &boot_info.memory_map).unwrap();
    println!("xsdt: {:#?}", xsdt);

    idt::initialize();
    interrupts::enable();
}
