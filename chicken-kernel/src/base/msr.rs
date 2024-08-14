use core::arch::x86_64::__cpuid;

use bitflags::bitflags;

const IA32_EFER: u32 = 0xC000_0080;

extern "C" {
    fn cpu_has_msr() -> bool;
    fn get_msr(index: u32) -> u64;

    fn set_msr(index: u32, value: u64);
}

bitflags! {
    /// Extended Feature Enable Register
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct Efer: u64 {
        /// System call extensions
        const SCE = 1 << 0;
        // bits  1-7 reserved
        /// Long mode enable (indicated long mode can be used but is not necessarily active)
        const LME = 1 << 8;
        // bit 9 reserved
        /// Long mode active (indicates long mode is active)
        const LMA = 1 << 10;
        /// No-Execute Enable (activates feature that allows to mark pages as NX)
        const NXE = 1 << 11;
        /// Secure Virtual Machine Enable
        const SVME = 1 << 12;
        /// Secure Virtual Machine Enable
        const LMSLE = 1 << 13;
        /// Fast FXSAVE/FXRSTOR
        const FFXSR = 1 << 14;
        /// Translation Cache Extension
        const TCE = 1 << 15;
        // bits 16-63 reserved
    }
}

impl Efer {
    /// Reads EFER register if MSR feature is available to CPU.
    pub fn read() -> Option<Self> {
        if unsafe { cpu_has_msr() } {
            Some(Self::from_bits_truncate(unsafe { get_msr(IA32_EFER) }))
        } else {
            None
        }
    }

    /// Writes EFER register if MSR feature is available to CPU. Returns whether it is available.
    pub fn write(self) -> bool {
        if unsafe { cpu_has_msr() } && (!self.contains(Self::NXE) || Self::nx_available()) {
            unsafe { set_msr(IA32_EFER, self.bits()) }
            true
        } else {
            false
        }
    }

    /// Whether the NX feature is available to the CPU
    pub fn nx_available() -> bool {
        unsafe { __cpuid(0x80000001).edx & (1 << 20) != 0 }
    }
}
