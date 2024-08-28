use core::sync::atomic::{AtomicPtr, Ordering};

use chicken_util::BootInfo;

use crate::base::{
    acpi::madt::{
        entry::{InterruptSourceOverride, IOApic},
        Madt,
    },
    io::{
        apic::{
            ioapic::{KEYBOARD_IRQ, TIMER_IRQ},
            lapic::LocalApicControl,
        },
        IOError,
    },
};

pub(super) mod ioapic;
pub(in crate::base) mod lapic;

static EOI_POINTER: AtomicPtr<u32> = AtomicPtr::new(0 as *mut u32);

/// Configures APIC and LAPIC of BSP. Also sets up memory mappings for LAPIC registers MMIO.
pub(super) fn set_up(boot_info: &BootInfo) -> Result<ApicConfig, IOError> {
    let lapic = LocalApicControl::enable()?;

    // store address in atomic pointer
    EOI_POINTER.store(lapic.eoi_pointer(), Ordering::Relaxed);

    let madt = unsafe { Madt::get(boot_info).as_ref().ok_or(IOError::MadtNotFound)? };
    let overrides = madt.parse_entries::<InterruptSourceOverride>();
    let keyboard_source = overrides
        .iter()
        .find(|iso| iso.source() == KEYBOARD_IRQ)
        .map(|iso| iso.gsi() as u8)
        .unwrap_or(KEYBOARD_IRQ);

    let pit_source = overrides
        .iter()
        .find(|iso| iso.source() == TIMER_IRQ)
        .map(|iso| iso.gsi() as u8)
        .unwrap_or(TIMER_IRQ);

    let io_apic_address = madt
        .parse_entry_first::<IOApic>()
        .ok_or(IOError::IOApicEntryNotFound)?
        .io_apic_address();

    let lapic_id = lapic.lapic_id();

    Ok(ApicConfig {
        io_apic_address,
        lapic_id,
        keyboard_source,
        pit_source,
    })
}
#[derive(Debug)]
pub(super) struct ApicConfig {
    /// Address of IO APIC that is used to handle hardware interrupts.
    pub(super) io_apic_address: u64,
    /// LAPIC ID of the BSP.
    pub(super) lapic_id: u8,
    /// Either the default [`KEYBOARD_IRQ`] or a source override specified in the MADT.
    pub(super) keyboard_source: u8,
    /// Either the default [`TIMER_IRQ`] or a source override specified in the MADT.
    pub(super) pit_source: u8,
}
