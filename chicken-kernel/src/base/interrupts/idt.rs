use core::cell::OnceCell;

use crate::{base::gdt::KERNEL_CS, scheduling::spin::SpinLock};

static IDT: SpinLock<OnceCell<InterruptDescriptorTable>> = SpinLock::new(OnceCell::new());

pub(in crate::base) fn initialize() {
    let mut idt_lock = IDT.lock();
    let _ = idt_lock.get_or_init(InterruptDescriptorTable::new);
    // can safely be unwrapped
    let idt = idt_lock.get_mut().unwrap();

    idt.setup_handlers();

    let idt_desc = IdtDescriptor {
        size: 0xFFF,
        offset: idt as *const _ as u64,
    };

    unsafe {
        load_idt(&idt_desc as *const IdtDescriptor);
    }
}

#[repr(align(16))]
#[derive(Debug)]
pub(in crate::base::interrupts) struct InterruptDescriptorTable([GateDescriptor; 256]);

impl InterruptDescriptorTable {
    fn new() -> Self {
        Self([GateDescriptor::default(); 256])
    }

    pub(in crate::base::interrupts) fn set_handler(
        &mut self,
        vector: u8,
        handler_address: u64,
        ist: u8,
        dpl: u8,
    ) {
        self.0[vector as usize] = GateDescriptor::new(
            handler_address,
            KERNEL_CS,
            ist,
            GateFlags::new(GateType::TrapGate, dpl, true),
        );
    }
}

extern "C" {
    fn load_idt(idt_ptr: *const IdtDescriptor);
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct IdtDescriptor {
    size: u16,
    offset: u64,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct GateDescriptor {
    offset_low: u16,
    segment_selector: u16,
    ist: u8,
    // type + 0 + dpl + present
    flags: GateFlags,
    offset_middle: u16,
    offset_high: u32,
    _reserved: u32,
}

impl GateDescriptor {
    fn new(offset: u64, segment_selector: u16, ist: u8, flags: GateFlags) -> Self {
        assert_eq!(ist & 0b11111000, 0, "IST must span within 3 bits.");

        let offset_low = (offset & 0xFFFF) as u16;
        let offset_middle = ((offset >> 16) & 0xFFFF) as u16;
        let offset_high = ((offset >> 32) & 0xFFFFFFFF) as u32;

        Self {
            offset_low,
            segment_selector,
            ist,
            flags,
            offset_middle,
            offset_high,
            _reserved: 0,
        }
    }
}

impl Default for GateDescriptor {
    fn default() -> Self {
        Self::new(0, 0, 0, GateFlags::new(GateType::TrapGate, 0, false))
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default)]
struct GateFlags(u8);

impl GateFlags {
    const fn new(r#type: GateType, dpl: u8, present: bool) -> Self {
        GateFlags(r#type.bits() | (dpl << 6) | ((present as u8) << 7))
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
enum GateType {
    /// Clears interrupt flag before calling handler
    _InterruptGate = 0,
    /// Does not clear interrupt flag before calling handler. Meaning interrupts can occur, while current one is being handled.
    TrapGate = 1,
}

impl GateType {
    /// Four type bits for GateFlags
    const fn bits(&self) -> u8 {
        match self {
            GateType::_InterruptGate => 0b1110,
            GateType::TrapGate => 0b1111,
        }
    }
}
