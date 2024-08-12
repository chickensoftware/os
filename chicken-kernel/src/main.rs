#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

mod base;
mod scheduling;
mod video;

#[no_mangle]
pub extern "sysv64" fn kernel_main() -> ! {
    base::setup();

    println!("Hello, Chicken OS :)");
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