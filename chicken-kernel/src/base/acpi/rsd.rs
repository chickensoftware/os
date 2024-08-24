use chicken_util::memory::PhysicalAddress;

use super::ACPIError;

const RSDP_SIGNATURE: [char; 8] = ['R', 'S', 'D', ' ', 'P', 'T', 'R', ' '];

/// Root System Description Pointer version 1
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub(in crate::base) struct Rsd1 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsd_addr: u32, // deprecated since rsd version 2
}

/// Root System Description Pointer version 2 or later
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub(in crate::base) struct RsdX {
    rsd1: Rsd1,
    length: u32,
    rsd_addr: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub(in crate::base) enum Rsd {
    V1(Rsd1),
    V2OrLater(RsdX),
}

impl Rsd {
    pub(in crate::base) fn get(rsdp: u64) -> Result<Self, ACPIError> {
        let rsdp = rsdp as *const u8;
        // validate rsd pointer
        for (index, character) in RSDP_SIGNATURE.iter().enumerate() {
            if (unsafe { rsdp.add(index).read() } as char) != *character {
                return Err(ACPIError::InvalidRSDAddress);
            }
        }

        // parse rsdp
        let rsd = unsafe { &*(rsdp as *const Rsd1) };
        Ok(if rsd.checksum == 0 {
            Rsd::V1(*rsd)
        } else {
            Rsd::V2OrLater(unsafe { *(rsd as *const Rsd1 as *const RsdX) })
        })
    }

    pub(in crate::base) fn rsd_table_address(&self) -> PhysicalAddress {
        match self {
            Rsd::V1(rsd) => rsd.rsd_addr as u64,
            Rsd::V2OrLater(rsd) => rsd.rsd_addr,
        }
    }
}
