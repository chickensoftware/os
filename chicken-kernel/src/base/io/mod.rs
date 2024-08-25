use core::arch::asm;

use chicken_util::BootInfo;

mod pic;

pub(super) fn initialize(_boot_info: &BootInfo) {
    // remap and disable pics, so they don't influence apic.
    unsafe {
        pic::remap();
        pic::disable();
    }

    // todo: set up apic io
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
pub(in crate::base::io) unsafe fn inb(port: Port) -> u8 {
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
