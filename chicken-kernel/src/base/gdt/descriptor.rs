use super::{
    tss::{TaskStateSegment, TSS_AVAILABLE_FLAGS},
    AccessByte, SegmentDescriptorFlags,
};

#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
pub(super) struct SegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: AccessByte,
    /// limit_high + flags
    granularity: u8,
    base_high: u8,
}

impl SegmentDescriptor {
    fn new(base: u32, limit: u32, access: AccessByte, flags: SegmentDescriptorFlags) -> Self {
        Self {
            limit_low: (limit & 0xFFFF) as u16,
            base_low: (base & 0xFFFF) as u16,
            base_middle: ((base >> 16) & 0xFF) as u8,
            access,
            granularity: ((limit >> 16) & 0x0F) as u8 | (flags.bits() & 0xF0),
            base_high: ((base >> 24) & 0xFF) as u8,
        }
    }

    pub(super) fn kernel_code() -> Self {
        SegmentDescriptor::new(
            0,
            0xFFFFF,
            AccessByte::PRESENT
                | AccessByte::DESCRIPTOR_TYPE
                | AccessByte::EXECUTABLE
                | AccessByte::READABLE_WRITEABLE
                | AccessByte::ACCESSED,
            SegmentDescriptorFlags::LONG_MODE | SegmentDescriptorFlags::GRANULARITY,
        )
    }
    pub(super) fn kernel_data() -> Self {
        SegmentDescriptor::new(
            0,
            0xFFFFF,
            AccessByte::PRESENT
                | AccessByte::DESCRIPTOR_TYPE
                | AccessByte::READABLE_WRITEABLE
                | AccessByte::ACCESSED,
            SegmentDescriptorFlags::GRANULARITY | SegmentDescriptorFlags::SIZE,
        )
    }

    pub(super) fn user_code() -> Self {
        SegmentDescriptor::new(
            0,
            0xFFFFF,
            AccessByte::PRESENT
                | AccessByte::DPL
                | AccessByte::DESCRIPTOR_TYPE
                | AccessByte::EXECUTABLE
                | AccessByte::READABLE_WRITEABLE
                | AccessByte::ACCESSED,
            SegmentDescriptorFlags::LONG_MODE | SegmentDescriptorFlags::GRANULARITY,
        )
    }

    pub(super) fn user_data() -> Self {
        SegmentDescriptor::new(
            0,
            0xFFFFF,
            AccessByte::PRESENT
                | AccessByte::DPL
                | AccessByte::DESCRIPTOR_TYPE
                | AccessByte::READABLE_WRITEABLE
                | AccessByte::ACCESSED,
            SegmentDescriptorFlags::GRANULARITY | SegmentDescriptorFlags::SIZE,
        )
    }
    /// Return the low and high segment descriptors pointing to the specified tss.
    ///
    /// # Safety
    /// Caller must ensure that tss lives long enough.
    pub(super) unsafe fn tss(tss: &TaskStateSegment) -> (Self, Self) {
        let tss_address = tss as *const TaskStateSegment as u64;
        let low = SegmentDescriptor::new(
            tss_address as u32,
            (size_of::<TaskStateSegment>() - 1) as u32,
            AccessByte::from_bits_truncate(AccessByte::PRESENT.bits() | TSS_AVAILABLE_FLAGS),
            SegmentDescriptorFlags::empty(),
        );

        let high = SegmentDescriptor::new(
            (tss_address >> 32) as u32,
            0,
            AccessByte::empty(),
            SegmentDescriptorFlags::empty(),
        );

        (low, high)
    }
}
