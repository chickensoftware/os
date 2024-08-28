#![no_std]
#![no_main]

extern crate alloc;

use core::{arch::asm, panic::PanicInfo};

use qemu_print::qemu_println;

use chicken_util::BootInfo;

mod base;
mod memory;
mod scheduling;
mod video;

#[no_mangle]
pub extern "sysv64" fn kernel_main(boot_info: &BootInfo) -> ! {
    let boot_info = memory::set_up(boot_info);
    video::set_up(&boot_info);
    println!("kernel: Memory Management has been set up successfully.");
    println!("kernel: Video output has been set up successfully.");
    base::set_up(&boot_info);
    println!("kernel: Base Architecture has been set up successfully.");
    scheduling::set_up();
    println!("kernel: Scheduler set up.");
    base::interrupts::enable();
    println!("kernel: Enabled interrupts.");
    println!("It did not crash.");
    hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    qemu_println!("panic: {}", info);
    println!("panic: {}", info);

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
