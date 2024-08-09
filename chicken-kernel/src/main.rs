#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

use qemu_print::qemu_println;

#[no_mangle]
pub extern "sysv64" fn kernel_main() -> ! {
    qemu_println!("Hello, Chicken OS :)");
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