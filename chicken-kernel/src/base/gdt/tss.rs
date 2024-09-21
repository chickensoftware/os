use core::cell::OnceCell;

use crate::{
    memory::vmm::{object::VmFlags, AllocationType, VMM},
    scheduling::spin::SpinLock,
};

/// Size of stack that is used when an interrupt occurres and the cpu is not in ring0.
const KERNEL_INTERRUPT_STACK_SIZE: usize = 0x5000;
/// 0x9: 64-bit TSS (Available) [System Segment Access Byte](https://wiki.osdev.org/Global_Descriptor_Table)
pub(in crate::base::gdt) const TSS_AVAILABLE_FLAGS: u8 = 0x9;

pub(in crate::base::gdt) static TSS: SpinLock<OnceCell<TaskStateSegment>> =
    SpinLock::new(OnceCell::new());

#[repr(C, packed(4))]
#[derive(Debug, Copy, Clone, Default)]
pub(in crate::base::gdt) struct TaskStateSegment {
    _reserved0: u32,
    /// The first stack pointer used to load the stack when a privilege level change occurs from a lower privilege level to a higher one.
    pub rsp0: u64,
    _rsp1: u64,
    _rsp2: u64,
    _reserved1: u64,
    pub ist: [u64; 7],
    _reserved_2: u64,
    _reserved_3: u16,
    /// I/O Map Base Address Field. Contains a 16-bit offset from the base of the TSS to the I/O Permission Bit Map.
    pub iopb: u16,
}

impl TaskStateSegment {
    /// Creates a new task state segment and allocates a kernel stack.
    ///
    /// # Safety
    /// The caller must ensure that this function get called AFTER the initialization of the kernel virtual memory manager.
    pub(in crate::base::gdt) unsafe fn create() -> Self {
        // must be called after memory has been set up.
        let mut vmm_binding = VMM.lock();
        let vmm = vmm_binding.get_mut();
        assert!(
            vmm.is_some(),
            "Global descriptor table setup must occur after memory setup."
        );
        let vmm = vmm.unwrap();

        let stack_bottom = vmm
            .alloc(
                KERNEL_INTERRUPT_STACK_SIZE + 1,
                VmFlags::WRITE,
                AllocationType::AnyPages,
            )
            .unwrap();

        let rsp0 = stack_bottom + KERNEL_INTERRUPT_STACK_SIZE as u64;

        Self {
            // effectively disable IO map => no longer used in modern systems.
            iopb: size_of::<TaskStateSegment>() as u16,
            rsp0,
            ..Default::default()
        }
    }
}
