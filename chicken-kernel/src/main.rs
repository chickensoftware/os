#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

use chicken_util::BootInfo;

mod base;
mod scheduling;
mod video;
mod memory;

#[no_mangle]
pub extern "sysv64" fn kernel_main(boot_info: &BootInfo) -> ! {
    video::setup();
    base::setup();
    memory::setup(boot_info);

    println!("It did not crash.");
    hlt_loop();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
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