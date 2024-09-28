use core::arch::asm;

use crate::{
    base::{
        interrupts::{idt::InterruptDescriptorTable, CpuState},
        io,
        io::{
            inb,
            keyboard::KEYBOARD,
            timer::{
                pit::{ProgrammableIntervalTimer, PIT},
                Timer,
            },
        },
    },
    println,
};

extern "C" {
    fn vector_0_handler();
}

impl InterruptDescriptorTable {
    pub(super) fn setup_handlers(&mut self) {
        let initial_handler_address = vector_0_handler as *const u8;
        for vector_number in 0..=255u8 {
            let dpl = if vector_number < 32 { 0 } else { 3 };
            self.set_handler(
                vector_number,
                unsafe { initial_handler_address.add(16 * vector_number as usize) } as u64,
                0,
                dpl,
            );
        }
    }
}

#[no_mangle]
pub fn interrupt_dispatch(mut state_ptr: *const CpuState) -> *const CpuState {
    qemu_print::qemu_println!("a");
    let state = unsafe { *state_ptr };
    match state.vector_number {
        0 => {
            println!("exception: DIV BY 0");
        }
        // gpf
        13 => {
            let rip: u64;

            unsafe {
                asm!("lea {0}, [rip]", out(reg) rip);
            }
            panic!(
                "exception: GENERAL PROTECTION FAULT. Error code: {:?}. RIP: {:#x}",
                error_code::ErrorCode::from_bits_truncate(state.error_code as u32),
                rip
            );
        }
        // page fault
        14 => {
            println!(
                "exception: PAGE FAULT. Error code: {:?}",
                error_code::PageFaultErrorCode::from_bits_truncate(state.error_code as u32)
            );
            // get register containing address of faulting page
            let cr2: u64;
            unsafe {
                asm!("mov {}, cr2", out(reg) cr2);
            }
            panic!("Faulting page address: {:#x}", cr2);
        }
        32 => {
            state_ptr = pit_handler(state_ptr);
        }
        33 => keyboard_handler(),
        _ => {
            println!(
                "Interrupt handler has not been set up. vector: {:#x}, error code (if set): {:?}",
                state.vector_number,
                error_code::ErrorCode::from_bits_truncate(state.error_code as u32)
            );
        }
    }

    state_ptr
}

fn keyboard_handler() {
    // parse keyboard scancode from port 0x60
    let scancode = unsafe { inb(0x60) };

    let mut binding = KEYBOARD.lock();
    binding.handle(scancode);

    // send end of interrupt signal to lapic that sent the interrupt
    io::apic::lapic::eoi();
}

fn pit_handler(context: *const CpuState) -> *const CpuState {
    super::disable();
    // increment tick counter
    ProgrammableIntervalTimer::tick();

    // context switch
    let binding = PIT.lock();
    let context = binding.perform_context_switch(context);

    // send end of interrupt signal to lapic that sent the interrupt
    io::apic::lapic::eoi();
    context
}

mod error_code {
    use bitflags::bitflags;

    bitflags! {
        /// Error code for page faults. In addition, the value of the CR2 register is set to the virtual address that causes the fault
        #[repr(C)]
        #[derive(Copy, Clone, Debug)]
        pub(super) struct PageFaultErrorCode: u32 {
            /// Present: When set, the page fault was caused by a page-protection violation. When not set, it was caused by a non-present page.
            const PRESENT = 1 << 0;
            /// Write: When set, the page fault was caused by a write access. When not set, it was caused by a read access.
            const WRITE = 1 << 1;
            /// User: When set, the page fault was caused while CPL = 3. This does not necessarily mean that the page fault was a privilege violation.
            const USER = 1 << 2;
            /// Reserved Write: When set, one or more page directory entries contain reserved bits which are set to 1. This only applies when the PSE or PAE flags in CR4 are set to 1.
            const RESERVED_WRITE = 1 << 3;
            /// Instruction Fetch: When set, the page fault was caused by an instruction fetch. This only applies when the No-Execute bit is supported and enabled.
            const INSTRUCTION_FETCH = 1 << 4;
            /// Protection Key: When set, the page fault was caused by a protection-key violation. PKRU register (user-mode accesses) or PKRS MSR (supervisor-mode accesses) specifies protection key rights.
            const PROTECTION_KEY = 1 << 5;
            /// Shadow Stack: When set, the page fault was caused by a shadow stack access.
            const SHADOW_STACK = 1 << 6;
            // bits 7 - 14 reserved
            /// Software Guard Extension: When set, the fault was due to an SGX violation. The fault is unrelated to ordinary paging.
            const SGX = 1 << 15;
            // bits 16 - 31 reserved
        }

        #[repr(C)]
        #[derive(Copy, Clone, Debug)]
        pub(super) struct ErrorCode: u32 {
            /// External: If set, means it was a hardware interrupt. Cleared for software interrupts.
            const EXTERNAL = 1 << 0;
            /// IDT: Set if this error code refers to the IDT. If cleared it refers to the GDT or LDT.
            const IDT = 1 << 1;
            /// Table Index: Set if the error code refers to the LDT, cleared if referring to the GDT.
            const TABLE_INDEX = 1 << 2;
            /// Index: The index into the table this error code refers to. This can be seen as a byte offset into the table, much like a GDT selector would.
            const INDEX = 0b11111111111111111111111111111 << 3;
        }

    }
}
