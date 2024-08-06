#![no_std]
#![no_main]

use core::panic::PanicInfo;
use log::{error, info};
use uefi::{
    entry,
    table::{Boot, SystemTable},
    Handle, Status,
};

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut system_table).unwrap();

    info!("Hello, uefi world!");
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occurred: \n{:#?}", info);
    loop {}
}
