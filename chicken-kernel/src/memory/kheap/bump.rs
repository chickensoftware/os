// note: for now using a simple bump allocator, will be a more sophisticated design later
/// Heap used by the kernel itself. Provides dynamic allocations for VMM
/// User Applications have their own user heap that depends on the VMM
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};

use chicken_util::memory::VirtualAddress;

use crate::{memory::align_up, scheduling::spin::SpinLock};

#[derive(Copy, Clone, Debug)]
pub struct BumpAllocator {
    heap_start: VirtualAddress,
    heap_end: VirtualAddress,
    next: VirtualAddress,
    allocations: usize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: VirtualAddress, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_size as VirtualAddress + heap_start;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for SpinLock<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock();

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size() as VirtualAddress) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            // out of memory :(
            ptr::null_mut()
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock();

        bump.allocations -= 1;

        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
