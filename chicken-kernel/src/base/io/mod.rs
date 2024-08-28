use core::{
    arch::asm,
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use chicken_util::{BootInfo, PAGE_SIZE};

use crate::{
    base::io::apic::ioapic,
    memory::vmm::{AllocationType, object::VmFlags, VMM, VmmError},
};

pub(in crate::base) mod apic;
mod pic;
pub(in crate::base) mod keyboard;

pub(super) fn initialize(boot_info: &BootInfo) {
    // remap and disable pics, so they don't influence apic.
    unsafe {
        pic::remap();
        pic::disable();
    }
    let apic_config = apic::set_up(boot_info).unwrap();

    // map mmio for io apic register interactions
    let mut binding = VMM.lock();
    let vmm = binding.get_mut().unwrap();
    let io_apic_virtual_address = vmm
        .alloc(
            PAGE_SIZE,
            VmFlags::WRITE | VmFlags::MMIO,
            AllocationType::Address(apic_config.io_apic_address),
        )
        .unwrap();

    // reconfigure entry for keyboard input
    unsafe {
        ioapic::configure_redirection_entry(
            io_apic_virtual_address,
            apic_config.keyboard_source,
            0x21,
            apic_config.lapic_id,
            true,
        );
    }
}

pub(in crate::base::io) type Port = u16;

/// Write 8 bits to the specified port.
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub(in crate::base::io) unsafe fn outb(port: Port, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value);
    }
}

/// Read 8 bits from the specified port.
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub(in crate::base) unsafe fn inb(port: Port) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port);
    value
}

/// Older machines may require to wait a cycle before continuing the io pic communication.
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn io_wait() {
    asm!("out 0x80, al", in("al") 0u8);
}

#[derive(Copy, Clone)]
pub(in crate::base::io) enum IOError {
    ModelSpecificRegisterUnavailable,
    MemoryMappingFailed(VmmError),
    MadtNotFound,
    IOApicEntryNotFound,
}

impl Debug for IOError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            IOError::ModelSpecificRegisterUnavailable => {
                write!(f, "IOError: MSR necessary for APIC is unavailable.")
            }
            IOError::MemoryMappingFailed(value) => {
                write!(f, "IOError: Memory Mapping failed: {}", value)
            }
            IOError::MadtNotFound => {
                write!(
                    f,
                    "IOError: System Descriptor Table with APIC information could not be found."
                )
            }
            IOError::IOApicEntryNotFound => {
                write!(f, "IOError: System Descriptor Table with APIC information could be found, but does not contain valid IO APIC entry.")
            }
        }
    }
}

impl Display for IOError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for IOError {}

impl From<VmmError> for IOError {
    fn from(value: VmmError) -> Self {
        Self::MemoryMappingFailed(value)
    }
}
