use core::sync::atomic::Ordering;
use chicken_util::{memory::VirtualAddress, PAGE_SIZE};

use crate::{
    base::{
        io::{apic::EOI_POINTER, IOError},
        msr,
        msr::ModelSpecificRegister,
    },
    memory::vmm::{AllocationType, object::VmFlags, VMM, VmmError},
};

const SPURIOUS_INTERRUPT_VECTOR_OFFSET: usize = 0xF0;
const EOI_OFFSET: usize = 0xB0;
const TASK_PRIORITY_OFFSET: usize = 0x80;
const LOCAL_APIC_ID_OFFSET: usize = 0x20;

/// Control struct for Local Apic of Boot Strap Processor
pub(in crate::base) struct LocalApicControl {
    lapic_address: VirtualAddress,
}

impl LocalApicControl {
    /// Enable Local Apic using Spurious Vector register. Returns the virtual address mapped to the LAPIC registers MMIO.
    pub(super) fn enable() -> Result<Self, IOError> {
        let lapic_address = msr::Apic::read()
            .ok_or(IOError::ModelSpecificRegisterUnavailable)?
            .address();

        // allocate apic control registers as MMIO
        // this is never freed, since the mapping is necessary for the interrupt handlers of the LAPIC as well.
        let mut vmm = VMM.lock();
        if let Some(vmm) = vmm.get_mut() {
            let virtual_address = vmm.alloc(
                PAGE_SIZE,
                VmFlags::MMIO | VmFlags::WRITE,
                AllocationType::Address(lapic_address),
            )?;

            unsafe {
                // more info: https://wiki.osdev.org/APIC#Local_APIC_configuration
                let lapic_registers = virtual_address as *const u8;
                let spurious_vector_register =
                    lapic_registers.add(SPURIOUS_INTERRUPT_VECTOR_OFFSET) as *mut u32;

                // spurious vector value of 0xFF and enable apic software
                spurious_vector_register.write_volatile(0xFF | (1 << 8));

                let task_priority_register = lapic_registers.add(TASK_PRIORITY_OFFSET) as *mut u32;

                // set priority to 0 so no interrupts are blocked
                task_priority_register.write_volatile(0x0);
            }
            Ok(Self {
                lapic_address: virtual_address,
            })
        } else {
            Err(IOError::MemoryMappingFailed(
                VmmError::GlobalVirtualMemoryManagerUninitialized,
            ))
        }
    }

    pub(super) fn eoi_pointer(&self) -> *mut u32 {
        unsafe { (self.lapic_address as *mut u8).add(EOI_OFFSET) as *mut u32 }
    }

    /// Returns the ID of the local apic.
    ///
    pub(in crate::base::io::apic) fn lapic_id(&self) -> u8 {
        unsafe {
            let id_reigster = (self.lapic_address as *const u8).add(LOCAL_APIC_ID_OFFSET);
            *id_reigster
        }
    }
}

/// Send the lapic the signal that an interrupt has been handled. Only sends the signal if the EOI_POINTER has been initialized
pub(in crate::base) fn eoi() {
    let eoi = EOI_POINTER.load(Ordering::Relaxed);
    if !eoi.is_null() {
        unsafe {
            // signal end of interrupt
            eoi.write_volatile(0);
        }
    }
}
