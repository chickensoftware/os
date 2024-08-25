use alloc::vec::Vec;
use chicken_util::BootInfo;
use crate::base::acpi::{rsd, sdt};
use crate::base::acpi::madt::entry::{MadtEntry, MadtEntryHeader};
use crate::base::acpi::sdt::SDTHeader;
use crate::println;
pub(in crate::base) mod entry;

#[repr(C)]
#[derive(Debug)]
pub struct Madt {
    header: SDTHeader,
    /// Base address of LAPIC registers
    local_apic_address: u32,
    flags: u32,
}

impl Madt {
    /// Returns pointer to MADT
    pub fn get(boot_info: &BootInfo) -> *const Madt {
        let rsd = rsd::Rsd::get(boot_info.rsdp).expect("Could not get RSD");
        let signature = ['A', 'P', 'I', 'C'];
        sdt::get(signature, rsd.rsd_table_address(), &boot_info.memory_map).expect("Could not get MADT")
            as *const Madt
    }

    /// Prints all entries of Madt Table
    pub fn print_entries(&self) {
        let madt_start = self as *const _ as *const u8;
        let mut pointer = unsafe { madt_start.add(size_of::<Madt>()) };
        // pointer to first byte after madt
        let madt_end = unsafe { madt_start.add(self.header.length as usize) };
        let mut counter = 0;
        while pointer < madt_end {
            let entry = unsafe { *(pointer as *const MadtEntryHeader) };

            println!("found entry {}:{:?}", counter, entry);

            pointer = unsafe { pointer.add(entry.record_length as usize) };
            counter += 1;
        }
    }

    /// Returns the first Madt entry in the Madt or None if there is none present
    pub fn parse_entry_first<T: Copy + MadtEntry>(&self) -> Option<T> {
        let madt_start = self as *const _ as *const u8;
        let mut pointer = unsafe { madt_start.add(size_of::<Madt>()) };
        // pointer to first byte after madt
        let madt_end = unsafe { madt_start.add(self.header.length as usize) };
        let mut result = None;
        while pointer < madt_end {
            let entry = unsafe { *(pointer as *const MadtEntryHeader) };

            // io apic has type 1
            if entry.entry_type == T::ENTRY_TYPE {
                result = Some(unsafe { *(pointer as *const T) });
                break;
            }

            pointer = unsafe { pointer.add(entry.record_length as usize) };
        }

        result
    }

    /// Returns all Madt entries specified by T in the Madt or an empty slice
    pub fn parse_entries<T: Copy + MadtEntry>(&self) -> Vec<T> {
        let mut entries = Vec::default();
        let madt_start = self as *const _ as *const u8;
        let mut pointer = unsafe { madt_start.add(size_of::<Madt>()) };
        // pointer to first byte after madt
        let madt_end = unsafe { madt_start.add(self.header.length as usize) };
        while pointer < madt_end {
            let entry = unsafe { *(pointer as *const MadtEntryHeader) };

            // io apic has type 1
            if entry.entry_type == T::ENTRY_TYPE {
                entries.push(unsafe { (pointer as *const T).read_unaligned() });
            }

            pointer = unsafe { pointer.add(entry.record_length as usize) };
        }

        entries
    }
}
