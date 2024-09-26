use core::{arch::asm, fmt::Debug};

use bitflags::bitflags;

pub(super) mod idt;
mod isr;
// control state of interrupts

bitflags! {
    /// Stores current state of CPU
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub(crate) struct RFlags : u64 {
        const CARRY = 1 << 0;
        // bit 1 reserved and always set to 1
        const RESERVED_1 = 1 << 1;
        const PARITY = 1 << 2;
        // bit 3 reserved
        const AUXILIARY_CARRY = 1 << 4;
        // bit 5 reserved
        const ZERO = 1 << 6;
        const SIGN = 1 << 7;
        const TRAP = 1 << 8;
        const INTERRUPTS_ENABLED = 1 << 9;
        const DIRECTION = 1 << 10;
        const OVERFLOW = 1 << 11;
        const IO_PRIVILEGE_LEVEL = 0b11 << 12;
        const NESTED_TASK = 1 << 14;
        // bit 15 reserved
        const RESUME = 1 << 16;
        const VIRTUAL_8086_MODE = 1 << 17;
        const ACCESS_CONTROL = 1 << 18;
        const VIRTUAL_INTERRUPT = 1 << 19;
        const VIRTUAL_INTERRUPT_PENDING = 1 << 20;
        const ID = 1 << 21;
        // 22 - 63 are reserved
    }
}

impl RFlags {
    pub(crate) fn read() -> RFlags {
        let rflags: u64;
        unsafe {
            asm!(
            "pushfq",
            "pop {0}",
            out(reg) rflags,
            );
        }
        RFlags::from_bits_truncate(rflags)
    }
}

#[inline]
pub(crate) fn enable() {
    unsafe { asm!("sti", options(preserves_flags, nostack)) }
}
#[inline]
pub(crate) fn disable() {
    unsafe { asm!("cli", options(preserves_flags, nostack)) }
}

#[inline]
pub(crate) fn are_enabled() -> bool {
    RFlags::read().contains(RFlags::INTERRUPTS_ENABLED)
}

#[inline]
pub(crate) fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let were_enabled_flag = are_enabled();

    if were_enabled_flag {
        disable();
    }

    let ret = f();

    if were_enabled_flag {
        enable();
    }

    ret
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub(crate) struct CpuState {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,

    vector_number: u64,
    error_code: u64,

    iretq_rip: u64,
    iretq_cs: u64,
    iretq_flags: RFlags,
    iretq_rsp: u64,
    iretq_ss: u64,
}

impl CpuState {
    pub(crate) fn basic(
        iretq_ss: u64,
        iretq_rsp: u64,
        iretq_flags: RFlags,
        iretq_cs: u64,
        iretq_rip: u64,
        rbp: u64,
    ) -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            vector_number: 0,
            error_code: 0,
            iretq_rip,
            iretq_cs,
            iretq_flags,
            iretq_rsp,
            iretq_ss,
        }
    }
}
