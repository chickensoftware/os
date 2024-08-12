// note: for now just using qemu_println, will later be changed to custom implementation.

use qemu_print::qemu_print;

use crate::base::interrupts::without_interrupts;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::video::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    without_interrupts(|| {
        qemu_print!("{}", args);
    })
}