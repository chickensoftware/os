use core::cell::OnceCell;

use chicken_util::BootInfo;

use crate::{
    base::{
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
    },
    scheduling::spin::SpinLock,
};

pub(in crate::base)  static LAPIC_CONTROL: SpinLock<OnceCell<LocalApicControl>> =
    SpinLock::new(OnceCell::new());

pub(super) mod ioapic;
mod lapic;

/// Configures APIC and LAPIC of BSP. Also sets up memory mappings for LAPIC registers MMIO.
pub(super) fn set_up(boot_info: &BootInfo) -> Result<ApicConfig, IOError> {
    let lapic = LocalApicControl::enable()?;

    let binding = LAPIC_CONTROL.lock();
    let lapic = binding.get_or_init(|| lapic);

    let madt = unsafe { Madt::get(boot_info).as_ref().ok_or(IOError::MadtNotFound)? };
    let overrides = madt.parse_entries::<InterruptSourceOverride>();
    let keyboard_source = overrides
        .iter()
        .find(|iso| iso.source() == KEYBOARD_IRQ)
        .map(|iso| iso.gsi() as u8)
        .unwrap_or(KEYBOARD_IRQ);

    let _timer_source = overrides
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
        _timer_source,
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
    pub(super) _timer_source: u8,
}
