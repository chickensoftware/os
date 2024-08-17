// note: for now just using qemu_println, will later be changed to custom implementation.

use qemu_print::qemu_print;

use chicken_util::BootInfo;
use crate::base::interrupts::without_interrupts;

pub(super) mod framebuffer;

const CHICKEN_OS: &str = r#"
   _____ _     _      _               ____   _____
  / ____| |   (_)    | |             / __ \ / ____|
 | |    | |__  _  ___| | _____ _ __ | |  | | (___
 | |    | '_ \| |/ __| |/ / _ \ '_ \| |  | |\___ \
 | |____| | | | | (__|   <  __/ | | | |__| |____) |
  \_____|_| |_|_|\___|_|\_\___|_| |_|\____/|_____/
                                                   "#;

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

pub(super) fn setup(_boot_info: BootInfo) {
    // todo: initialize global writer

    println!("{}", CHICKEN_OS);
}