#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use qemu_print::qemu_println;
use chicken_util::BootInfo;
use chicken_util::graphics::Color;
use crate::video::framebuffer::RawFrameBuffer;

mod base;
mod scheduling;
mod video;
mod memory;

#[no_mangle]
pub extern "sysv64" fn kernel_main(boot_info: &BootInfo) -> ! {
    let boot_info = *boot_info;

    memory::setup(boot_info);
    video::setup(boot_info);
    base::setup();

    RawFrameBuffer::from(boot_info.framebuffer_metadata).fill(Color::yellow());

    qemu_println!("It did not crash.");
    hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    qemu_println!("panic: {}", info);
    hlt_loop();
}

#[inline]
fn hlt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}