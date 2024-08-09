use bitflags::bitflags;

pub mod index;
pub mod manager;


bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct PageEntryFlags: u64 {
        /// Page is actually in physical memory at the moment
        const PRESENT        = 1 << 0;
        /// Read/Write permission
        const READ_WRITE     = 1 << 1;
        /// Controls access to page based on privilege level
        const USER_SUPER     = 1 << 2;
        /// Enables write-though caching
        const WRITE_THROUGH  = 1 << 3;
        /// Disables cache entirely
        const CACHE_DISABLED = 1 << 4;
        /// Used to discover whether a PDE or PTE was read during virtual address translation
        const ACCESSED       = 1 << 5;
        /// Determine whether a page has been written to
        const DIRTY        = 1 << 6;
        /// Turns next entry into huge page of size of page table it would have been
        const LARGER_PAGES   = 1 << 7;
        /// Global bit
        const GLOBAL        = 1 << 8;
        const AVAILABLE_MASK = 0b111 << 9;

    }
}

/// Page Directory or Page Table
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct PageEntry(u64);

impl PageEntry {
    /// Create new page entry based on address and flags
    pub fn new(address: u64, flags: PageEntryFlags) -> Self {
        let address_shifted = address & 0x000f_ffff_ffff_f000;
        let flags_bits = flags.bits();
        PageEntry(address_shifted | flags_bits)
    }

    /// Set address of page entry
    pub fn set_address(&mut self, address: u64) {
        let address = address & 0x000f_ffff_ffff_f000;
        self.0 = (self.0 & 0xfff) | address;
    }

    /// Set flags of page entry
    pub fn set_flags(&mut self, flags: PageEntryFlags) {
        let flags_bits = flags.bits() & 0xfff; // only use lower 12 bits
        self.0 = (self.0 & !0xfff) | flags_bits;
    }

    /// Get address of page entry
    pub fn address(&self) -> u64 {
        self.0 & 0x000f_ffff_ffff_f000
    }

    /// Get address of page entry
    pub fn flags(&self) -> PageEntryFlags {
        PageEntryFlags::from_bits_truncate(self.0 & 0xfff) // Mask to get only the lower 12 bits for flags
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageEntry; 512],
}
