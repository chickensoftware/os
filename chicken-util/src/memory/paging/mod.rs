use bitflags::bitflags;

pub mod index;
pub mod manager;

pub const KERNEL_MAPPING_OFFSET: u64 = 0xFFFF_FFFF_8000_0000;
pub const KERNEL_STACK_MAPPING_OFFSET: u64 = 0xFFFF_FFFF_6000_0000;

bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct PageEntryFlags: u64 {
        /// Present: Page is actually in physical memory at the moment
        const PRESENT        = 1 << 0;
        /// Read/Write permission
        const READ_WRITE     = 1 << 1;
        /// Controls access to page based on privilege level
        const USER_SUPER     = 1 << 2;
        /// Page Write Though: Enables write-though caching
        const WRITE_THROUGH  = 1 << 3;
        /// Page Cache Disabled: Disables cache entirely
        const CACHE_DISABLED = 1 << 4;
        /// Accessed: Used to discover whether a PDE or PTE was read during virtual address translation
        const ACCESSED       = 1 << 5;
        /// For Page Directory (Pointer) Entry / PML4: Available for use
        ///
        /// For Page Table Entry: Dirty: Determine whether a page has been written to
        const DIRTY_AVL        = 1 << 6;
        /// For Page Directory (Pointer) Entry / PML4: Page Size: Turns next entry into huge page of size of page table it would have been.
        ///
        /// For Page Table Entry: PAT: If PAT is supported, then PAT along with PCD and PWT shall indicate the memory caching type. Otherwise reserved-
        const PAT_PAGE_SIZE   = 1 << 7;
        /// For Page Directory (Pointer) Entry / PML4: Available for use
        ///
        /// For Page Table Entry: Global: Tells the processor not to invalidate the TLB entry corresponding to the page upon a MOV to CR3 instruction.
        const GLOBAL_AVL        = 1 << 8;
        const AVAILABLE_MASK = 0b111 << 9;
        /// For Page Directory (Pointer) Entry / PML4: Available for use
        ///
        /// For Page Table Entry: Protection Key: The protection key is a 4-bit corresponding to each virtual address that is used to control user-mode and supervisor-mode memory accesses.
        const PROTECTION_KEY_AVL = 0b1111 << 59;
        /// Execute Disable: If the NXE bit (bit 11) is set in the EFER register, then instructions are not allowed to be executed at addresses within the page whenever XD is set. If EFER.NXE bit is 0, then the XD bit is reserved and should be set to 0.
        const EXECUTE_DISABLE = 1 << 63;
    }
}

impl Default for PageEntryFlags {
    fn default() -> Self {
        PageEntryFlags::PRESENT | PageEntryFlags::READ_WRITE
    }
}

impl PageEntryFlags {
    pub fn default_nx() -> Self {
        PageEntryFlags::PRESENT | PageEntryFlags::READ_WRITE | PageEntryFlags::EXECUTE_DISABLE
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
