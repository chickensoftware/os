use core::ptr::read_unaligned;
use chicken_util::memory::{MemoryMap, MemoryType, PhysicalAddress};
use crate::base::acpi::ACPIError;
use crate::memory::get_virtual_offset;
use crate::println;

const XSDT_SIGNATURE: [char; 4] = ['X', 'S', 'D', 'T'];

/// system descriptor table header
#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct SDTHeader {
    signature: [u8; 4],
    pub(crate) length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

/// Returns instance of SDTHeader, if the address is valid, or None, if the signature of the header does not match.
pub fn get_xsdt(xsdt_header_address: PhysicalAddress, memory_map: &MemoryMap) -> Result<SDTHeader, ACPIError> {

    // adapt to virtual address
    let xsdt_header_address = (xsdt_header_address + get_virtual_offset(MemoryType::AcpiData, memory_map).ok_or(ACPIError::InvalidMemoryMap)?) as *const u8;

    // validate main system descriptor table address
    for (index, character) in XSDT_SIGNATURE.iter().enumerate() {
        if unsafe { xsdt_header_address.add(index).read() as char } != *character { return Err(ACPIError::InvalidXSDTAddress); }
    }
    Ok(unsafe { *(xsdt_header_address as *const SDTHeader) })
}

/// Returns either a valid pointer to the system descriptor table matching the given signature or an error, if the retrieving of the table fails.
pub fn get(signature: [char; 4], xsdt_header_address: u64, memory_map: &MemoryMap) -> Result<*const SDTHeader, ACPIError> {
    let xsdt = get_xsdt(xsdt_header_address, memory_map)?;
    let xsdt_header_address = (xsdt_header_address + get_virtual_offset(MemoryType::AcpiData, memory_map).ok_or(ACPIError::InvalidMemoryMap)?) as *const u8;
    println!("got xsdt, getting sigature: {:?}", signature);
    // amount of remaining u64 pointers to the other tables that fit into the total size of the XSDT
    let entries = (xsdt.length as usize - size_of::<SDTHeader>()) / 8;
    println!("entries: {}", entries);

    let pointer_base = unsafe { xsdt_header_address.add(size_of::<SDTHeader>()) };
    println!("pointer base: {:?}", pointer_base);
    for i in 0..entries {
        let entry_ptr = unsafe { read_unaligned(pointer_base.add(i * 8) as *const u64) };
        println!("entry pointer old: {:#x}", entry_ptr);

        let entry_ptr = (entry_ptr + get_virtual_offset(MemoryType::AcpiData, memory_map).ok_or(ACPIError::InvalidMemoryMap)?) as *const SDTHeader;
        println!("entry pointer new: {:?}", entry_ptr);
        let sdt_header = unsafe { &*entry_ptr };
        println!("found header: {:?}", sdt_header);


        let mut sdt_header_signature: [char; 4] = [0u8 as char; 4];

        for (index, character) in sdt_header.signature.iter().enumerate() {
            sdt_header_signature[index] = *character as char;
        }

        if signature == sdt_header_signature {
            println!("success");
            return Ok(sdt_header);
        }
    }
    Err(ACPIError::TableNotFound(signature))
}