use qemu_print::qemu_println;

use chicken_util::BootInfo;

use crate::memory::pmm::BitMapAllocator;

pub(in crate::memory) mod pmm;

pub(super) fn setup(boot_info: &BootInfo) {
    let pmm = BitMapAllocator::try_new(boot_info.memory_map).unwrap();
    qemu_println!("pmm:");
    qemu_println!("free: {} bytes", pmm.free_memory());
    qemu_println!("used: {} bytes", pmm.used_memory());
    qemu_println!("reserved: {} bytes", pmm.reserved_memory());
}
