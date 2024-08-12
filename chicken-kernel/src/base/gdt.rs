use core::cell::OnceCell;

use bitflags::bitflags;

use crate::scheduling::spin::SpinLock;

pub(crate) const CS: u16 = 0x08;

static GDT: SpinLock<OnceCell<GlobalDescriptorTable>> = SpinLock::new(OnceCell::new());

extern "C" {
    fn load_gdt(gdt: *const GdtDescriptor);
}

pub(super) fn initialize() {
    let gdt_lock = GDT.lock();
    let gdt = gdt_lock.get_or_init(GlobalDescriptorTable::new);

    let gdt_desc = GdtDescriptor {
        size: (size_of::<GlobalDescriptorTable>() - 1) as u16,
        offset: gdt as *const _ as u64,
    };

    unsafe { load_gdt(&gdt_desc as *const GdtDescriptor); }
}


#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct GdtDescriptor {
    size: u16,
    offset: u64,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
struct SegmentDescriptor {
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

    fn kernel_code() -> Self {
        SegmentDescriptor::new(0, 0xFFFFF, AccessByte::PRESENT | AccessByte::DESCRIPTOR_TYPE | AccessByte::EXECUTABLE | AccessByte::READABLE_WRITEABLE, SegmentDescriptorFlags::LONG_MODE | SegmentDescriptorFlags::GRANULARITY)
    }
    fn kernel_data() -> Self {
        SegmentDescriptor::new(0, 0xFFFFF, AccessByte::PRESENT | AccessByte::DESCRIPTOR_TYPE | AccessByte::READABLE_WRITEABLE, SegmentDescriptorFlags::LONG_MODE | SegmentDescriptorFlags::GRANULARITY)
    }

    fn user_code() -> Self {
        SegmentDescriptor::new(0, 0xFFFFF, AccessByte::PRESENT | AccessByte::DPL | AccessByte::DESCRIPTOR_TYPE | AccessByte::EXECUTABLE | AccessByte::CONFORMING_DIRECTION, SegmentDescriptorFlags::LONG_MODE | SegmentDescriptorFlags::GRANULARITY)
    }

    fn user_data() -> Self {
        SegmentDescriptor::new(0, 0xFFFFF, AccessByte::PRESENT | AccessByte::DPL | AccessByte::DESCRIPTOR_TYPE | AccessByte::CONFORMING_DIRECTION, SegmentDescriptorFlags::LONG_MODE | SegmentDescriptorFlags::GRANULARITY)
    }
}

#[allow(dead_code)]
#[repr(align(0x1000))]
#[derive(Copy, Clone, Debug)]
struct GlobalDescriptorTable {
    null: SegmentDescriptor,
    kernel_code: SegmentDescriptor,
    kernel_data: SegmentDescriptor,
    user_code: SegmentDescriptor,
    user_data: SegmentDescriptor,
}

impl GlobalDescriptorTable {
    fn new() -> Self {
        GlobalDescriptorTable {
            null: SegmentDescriptor::default(),
            kernel_code: SegmentDescriptor::kernel_code(),
            kernel_data: SegmentDescriptor::kernel_data(),
            user_code: SegmentDescriptor::user_code(),
            user_data: SegmentDescriptor::user_data(),
        }
    }
}
bitflags! {
    #[derive(Copy, Clone, Debug, Default)]
    struct AccessByte: u8 {
        /// The CPU will set it when the segment is accessed unless set to 1 in advance.
        const ACCESSED              = 1 << 0;
        /// * Code Segments: Readable (write access is never allowed on code segments)
        /// * Data Segments: Writeable (read access is always allowed on data segments)
        const READABLE_WRITEABLE    = 1 << 1;
        /// * Code Selectors: Conforming (If clear (0) code in this segment can only be executed from the ring set in DPL. If set (1) code in this segment can be executed from an equal or lower privilege level.)
        /// * Data Selectors: Direction (If clear (0) the segment grows up. If set (1) the segment grows down, ie. the Offset has to be greater than the Limit.)
        const CONFORMING_DIRECTION  = 1 << 2;
        /// If clear (0) the descriptor defines a data segment. If set (1) it defines a code segment which can be executed from.
        const EXECUTABLE            = 1 << 3;
        /// If clear (0) the descriptor defines a system segment (eg. a Task State Segment). If set (1) it defines a code or data segment.
        const DESCRIPTOR_TYPE       = 1 << 4;
        /// Descriptor privilege level field. Contains the CPU Privilege level of the segment. 0 = highest privilege (kernel), 3 = lowest privilege (user applications).
        const DPL                   = 0b11 << 5;
        /// Allows an entry to refer to a valid segment. Must be set (1) for any valid segment.
        const PRESENT               = 1 << 7;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Default)]
    struct SegmentDescriptorFlags: u8 {
        /// Long-mode code flag. If set (1), the descriptor defines a 64-bit code segment. When set, DB should always be clear. For any other type of segment (other code types or any data segment), it should be clear (0).
        const LONG_MODE             = 1 << 5;
        /// Size flag. If clear (0), the descriptor defines a 16-bit protected mode segment. If set (1) it defines a 32-bit protected mode segment. A GDT can have both 16-bit and 32-bit selectors at once.
        const SIZE                  = 1 << 6;
        /// Granularity flag, indicates the size the Limit value is scaled by. If clear (0), the Limit is in 1 Byte blocks (byte granularity). If set (1), the Limit is in 4 KiB blocks (page granularity).
        const GRANULARITY           = 1 << 7;
    }
}
