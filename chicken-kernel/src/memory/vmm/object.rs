use alloc::boxed::Box;
use core::ptr::NonNull;

use bitflags::bitflags;

use chicken_util::memory::{paging::PageEntryFlags, VirtualAddress};

#[allow(dead_code)] // otherwise, clippy complains about the flags field being 'unused'
#[derive(Debug)]
pub(super) struct VmObject {
    pub(super) base: VirtualAddress,
    pub(super) length: usize,
    pub(super) flags: VmFlags,
    pub(super) next: Option<NonNull<VmObject>>,
    pub(super) prev: Option<NonNull<VmObject>>,
}

impl VmObject {
    /// Allocates new `VmObject` struct on the heap. Returns a non-null pointer to the object.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the new allocated vm object is valid.
    pub(super) unsafe fn alloc_new(
        base: VirtualAddress,
        length: usize,
        flags: VmFlags,
        next: Option<NonNull<VmObject>>,
        prev: Option<NonNull<VmObject>>,
    ) -> NonNull<VmObject> {
        let new_object = Box::into_raw(Box::new(VmObject {
            base,
            length,
            flags,
            next,
            prev,
        }));
        NonNull::new_unchecked(new_object)
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub(crate) struct VmFlags: u8 {
        /// If set, the object can be written to
        const WRITE = 1 << 0;
        /// If set, the object can be executed
        const EXECUTABLE = 1 << 1;
        /// If set, the object can be accessed by the lowest privilege ring, otherwise, just the kernel
        const USER = 1 << 2;
        /// If set, the objects is mapped to MMIO and therefore does not need to request pages when allocated.
        const MMIO = 1 << 3;
    }
}

impl From<VmFlags> for PageEntryFlags {
    fn from(value: VmFlags) -> Self {
        let mut flags = PageEntryFlags::PRESENT;

        if value.contains(VmFlags::WRITE) {
            flags |= PageEntryFlags::READ_WRITE;
        }
        if !value.contains(VmFlags::EXECUTABLE) {
            flags |= PageEntryFlags::EXECUTE_DISABLE;
        }
        if value.contains(VmFlags::USER) {
            flags |= PageEntryFlags::USER_SUPER;
        }
        flags
    }
}
