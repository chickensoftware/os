#![allow(dead_code)] // keeping all variants of MADT entries, for possible use in the future.
use bitflags::bitflags;

use chicken_util::memory::VirtualAddress;

/// Marker trait for MADT entries
pub(in crate::base) trait MadtEntry {
    const ENTRY_TYPE: u8;
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub(in crate::base) struct MadtEntryHeader {
    pub(super) entry_type: u8,
    pub(super) record_length: u8,
}

/// Madt entry for external device io
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub(in crate::base) struct IOApic {
    header: MadtEntryHeader,
    /// IO Apic's ID
    io_apic_id: u8,
    reserved: u8,
    /// Physical address used to access IOApic
    io_apic_address: u32,
    /// Global system interrupt number where this IO Apic's interrupts start
    global_system_interrupt_base: u32,
}

impl IOApic {
    /// Returns the physical of the IO APIC.
    pub(in crate::base) fn io_apic_address(&self) -> VirtualAddress {
        self.io_apic_address as u64
    }
}

impl MadtEntry for IOApic {
    const ENTRY_TYPE: u8 = 1;
}

/// Madt entry for each local processor's apic
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub(in crate::base) struct LApic {
    header: MadtEntryHeader,
    ///## ACPI ID ##
    /// which the OS associates with the processor's device object
    acpi_processor_id: u8,
    /// ## Processor's local APIC ID ##
    apic_id: u8,
    /// ## Local APIC flags ##
    /// * bit 0 = enabled: processor is ready for use,
    /// * bit 1 = online capable: if the Enabled bit is set, this bit is reserved and must be zero. Otherwise, if this bit is set, system hardware supports enabling this processor during OS runtime
    /// * remaining 30 bits = reserved: must be 0
    flags: u32,
}

impl MadtEntry for LApic {
    const ENTRY_TYPE: u8 = 0;
}
/// Madt entry that is used to describe exceptions where the platform's implementation differs from the standard dual 8259 interrupt definition
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub(in crate::base) struct InterruptSourceOverride {
    header: MadtEntryHeader,
    /// Specifies the type of bus that the interrupt source belongs to. In ISA (Industry Standard Architecture) context a constant value of 0.
    bus: u8,
    /// Bus relative interrupt source (IRQ)
    source: u8,
    /// The Global System Interrupt that this bus-relative interrupt source will signal
    global_system_interrupt: u32,
    flags: MpsInitFlags,
}

impl InterruptSourceOverride {
    /// Returns IRQ of ISO
    pub(in crate::base) fn source(&self) -> u8 {
        self.source
    }
    /// Returns global_system_interrupt of ISO.
    pub(in crate::base) fn gsi(&self) -> u32 {
        self.global_system_interrupt
    }
}

impl MadtEntry for InterruptSourceOverride {
    const ENTRY_TYPE: u8 = 2;
}

/// Madt entry for local APIC Non-Maskable Interrupts
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub(in crate::base) struct LApicNmi {
    header: MadtEntryHeader,
    /// Value corresponding to the _UID listed in the processor’s device object, or the Processor ID corresponding to the ID listed in the processor object.
    acpi_processor_uid: u8,
    flags: MpsInitFlags,
    /// Local APIC LINT pin (0 for LINT0, 1 for LINT1)
    lint: u8,
}

impl MadtEntry for LApicNmi {
    const ENTRY_TYPE: u8 = 4;
}

bitflags! {
    /// ## Multi-Processor Specification Interrupt Type Information Flags ##
    /// * bits 0,1 = polarity:
    ///     * 00 Conforms to the specifications of the bus
    ///     * 01 Active high
    ///     * 10 Reserved
    ///     * 11 Active low
    /// * bits 2,3 = trigger mode:
    ///     * 00 Conforms to the specifications of the bus
    ///     * 01 Edge-triggered
    ///     * 10 Reserved
    ///     * 11 Level-triggered
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub(in crate::base) struct MpsInitFlags: u16 {
        const POLARITY_CONFORMS = 0b00;
        const POLARITY_ACTIVE_HIGH = 0b01;
        const POLARITY_RESERVED = 0b10;
        const POLARITY_ACTIVE_LOW = 0b11;
        const TRIGGER_CONFORMS = 0b00 << 2;
        const TRIGGER_EDGE = 0b01 << 2;
        const TRIGGER_RESERVED = 0b10 << 2;
        const TRIGGER_LEVEL = 0b11 << 2;
    }
}
