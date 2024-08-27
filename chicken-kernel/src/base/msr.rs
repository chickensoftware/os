use core::arch::x86_64::__cpuid;

use bitflags::{bitflags, Flags};

const IA32_EFER: u32 = 0xC000_0080;
const IA32_APIC: u32 = 0x1B;

extern "C" {
    fn cpu_has_msr() -> bool;
    fn get_msr(index: u32) -> u64;

    fn set_msr(index: u32, value: u64);
}

pub(crate) trait ModelSpecificRegister: Sized + Flags<Bits = u64> {
    const MSR_INDEX: u32;

    /// Reads specific register if MSR feature is available to CPU.
    fn read() -> Option<Self> {
        if unsafe { cpu_has_msr() } {
            Some(Self::from_bits_truncate(unsafe {
                get_msr(Self::MSR_INDEX)
            }))
        } else {
            None
        }
    }

    /// Writes specific register if MSR feature is available to CPU. Returns whether it is available.
    fn write(self) -> bool {
        if unsafe { cpu_has_msr() } {
            unsafe { set_msr(Self::MSR_INDEX, self.bits()) }
            true
        } else {
            false
        }
    }
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
impl ModelSpecificRegister for Efer {
    const MSR_INDEX: u32 = IA32_EFER;

    fn write(self) -> bool {
        if unsafe { cpu_has_msr() } && (!self.contains(Self::NXE) || Self::nx_available()) {
            unsafe { set_msr(IA32_EFER, self.bits()) }
            true
        } else {
            false
        }
    }
}
impl Efer {
    /// Whether the NX feature is available to the CPU
    pub fn nx_available() -> bool {
        unsafe { __cpuid(0x80000001).edx & (1 << 20) != 0 }
    }
}

bitflags! {
     /// Status and Location of the local APIC
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct Apic: u64 {
        // bits 0-7 reserved
        /// Indicates if the current processor is the bootstrap processor (BSP)
        const BSP = 1 << 8;
        // bits 9-10 reserved
        /// Enables or disables the local APIC
        const LAPIC_ENABLE = 1 << 11;
        /// Specifies the base address of the APIC registers. This 24-bit value is extended by 12 bits at the low end to form the base address.
        const APIC_REGISTERS_BASE = 0b111111111111111111111111 << 12;
        // bits 36-63 reserved
    }
}

impl ModelSpecificRegister for Apic {
    const MSR_INDEX: u32 = IA32_APIC;
}

impl Apic {
    /// Returns the address of the apic registers
    pub(crate) fn address(&self) -> u64 {
        self.bits() & 0b11111111111111111111000000000000
    }
}
