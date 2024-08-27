use bitflags::bitflags;
use chicken_util::memory::VirtualAddress;

/// Interrupt Request (IRQ) for PS/2 keyboard entry index
pub(super) const KEYBOARD_IRQ: u8 = 1;
/// Interrupt Request (IRQ) for pit entry index
pub(super) const TIMER_IRQ: u8 = 0;

// I/O APIC Registers for accessing other registers:
/// I/O Register Select: Is used to select the I/O Register to access
const IOREGSEL_OFFSET: usize = 0x00;
/// I/O Window (data): Used to access data selected by IOREGSEL
const IOWIN_OFFSET: usize = 0x10;

// I/O APIC Registers that are accessed using selection registers mentioned above:
/// I/O APIC Redirection tables: The redirection tables: 0x03 - 0x3f with registers starting from 0x10 (read/write)
const IOREDTBL_REGISTERS_OFFSET: u8 = 0x10;

/// Write to the IOAPIC control registers.
///
/// # Safety
/// The caller must ensure that the register specified by the address and offset is valid and can be written to.
unsafe fn write(io_apic_base: u64, offset: u8, value: u32) {
    let reg_select = (io_apic_base + IOREGSEL_OFFSET as u64) as *mut u32;
    let reg_window = (io_apic_base + IOWIN_OFFSET as u64) as *mut u32;

    // write to IOREGSEL to select the register
    reg_select.write_volatile(offset as u32);

    // write to IOWIN to set the new value
    reg_window.write_volatile(value);
}

/// Configure a new redirection entry to handle a hardware interrupt using the specified interrupt handler vector offset.
///
/// # Safety
/// The caller must ensure that the IO APIC address is valid and mapped.
pub(in crate::base::io) unsafe fn configure_redirection_entry(
    io_apic_base: VirtualAddress,
    index: u8,
    idt_vector_index: u8,
    destination_lapic_id: u8,
    enable: bool,
) {
    let low_index = IOREDTBL_REGISTERS_OFFSET + (index * 2);
    let high_index = low_index + 1;

    // construct lower register of redirection entry (delivery mode=000, destination mode=physical, pin polarity=active-high, trigger mode=edge
    let mut lvt = LocalVectorTableEntry::from_bits_truncate(idt_vector_index as u32);
    if !enable {
        lvt.insert(LocalVectorTableEntry::INTERRUPT_MASK);
    }

    // construct higher register of redirection entry
    let destination = (destination_lapic_id as u32) << 24;

    // write redirection entry
    write(io_apic_base, low_index, lvt.bits());
    write(io_apic_base, high_index, destination);
}

bitflags! {
    /// General structure of all LVT entries, except the timer entry (and the thermal sensor and performance entries ignore bits 15:13)
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct LocalVectorTableEntry: u32 {
        /// IDT entry that should be triggered for the specific interrupt.
        const INTERRUPT_VECTOR = 0xFF;
        /// Determines how the APIC should present the interrupt to the processor (default 0b000, 0b100 if NMI).
        const DELIVERY_MODE = 0b111 << 8;
        /// Either physical or logical.
        const DESTINATION_MODE = 0b1 << 11;
        /// Whether the interrupt has been served or not (read only).
        const DELIVERY_STATUS = 0b1 << 12;
        /// 0 is active-high, 1 is active-low.
        const PIN_POLARITY = 0b1 << 13;
        /// Used by the APIC for managing level-triggered interrupts (read only).
        const REMOTE_INTERRUPT_REQUEST_REGISTER = 0b1 << 14;
        /// 0 is edge-triggered, 1 is level-triggered.
        const TRIGGER_MODE = 0b1 << 15;
        /// If it is 1 the interrupt is disabled, if 0 is enabled.
        const INTERRUPT_MASK = 0b1 << 16;
    }

}
