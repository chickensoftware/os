#![no_std]
#![no_main]

extern crate alloc;

use core::{arch::asm, panic::PanicInfo};

use base::interrupts::without_interrupts;
use chicken_util::{
    memory::paging::{index::PageMapIndexer, PageEntryFlags},
    BootInfo, PAGE_SIZE,
};
use memory::paging::PTM;
use qemu_print::qemu_println;
use scheduling::task::{ProgramData, TaskEntry};

use crate::{
    base::io::timer::pit::get_current_uptime_ms,
    scheduling::{task, GlobalTaskScheduler},
};

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
    // is never reached, because task scheduler starts when interrupts are enabled.
    hlt_loop();
}

pub(crate) fn main_task() {
    println!("Hello, from main task!");

    fn hello() {
        println!("Hello, from new main task thread.");

        GlobalTaskScheduler::sleep(10000);

        println!("Main task thread sleep complete.");

        GlobalTaskScheduler::kill_active();
    }

    let thread_handle = task::spawn_thread(hello, None).unwrap();

    GlobalTaskScheduler::join(thread_handle);

    println!("{}", get_current_uptime_ms());

    println!("before");
    println!("now spawning");
    let virtual_addr = 0x1000000;

    without_interrupts(|| {
        let mut ptm = PTM.lock();
        let manager = ptm.get_mut().unwrap();
        println!("setting up user map");
        let physical = manager
            .manager()
            .get_physical(test_user as usize as u64)
            .unwrap();
        let flags =
            PageEntryFlags::READ_WRITE | PageEntryFlags::PRESENT | PageEntryFlags::USER_SUPER;
        println!(
            "mapping: virt: {:#x} to phys: {:#x} TEST",
            virtual_addr, physical
        );
        let indexer = PageMapIndexer::new(virtual_addr);
        println!("before mapping indexer p_i: {:#x}", indexer.p_i());
        manager.map_memory(virtual_addr, physical, flags).unwrap();
        println!(
            "mapped in manager pml4: {:?}, virtual: {:?}",
            manager.manager().pml4_physical(),
            manager.manager().pml4_virtual()
        );
    });
    task::spawn_process(
        TaskEntry::User(ProgramData {
            virt_start: virtual_addr,
            virt_end: virtual_addr + PAGE_SIZE as u64,
        }),
        None,
    )
    .unwrap();

    GlobalTaskScheduler::kill_active();
}

fn test_user() {
    loop {}
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
