use core::arch::asm;

use crate::memory::{
    paging::{index::PageMapIndexer, PageEntryFlags, PageTable},
    pmm::{PageFrameAllocator, PageFrameAllocatorError},
    PhysicalAddress, VirtualAddress,
};

/// Manages page tables
#[derive(Debug)]
pub struct OwnedPageTableManager<'a> {
    pub(in crate::memory) page_frame_allocator: PageFrameAllocator<'a>,
    page_table_manager: PageTableManager,
}

impl<'a> OwnedPageTableManager<'a> {
    /// Creates new page table manager instance. By default, a virtual `offset` of 0 is used. This can be changed manually using [`PageTableManager::update_offset()`]. The virtual address of the root page table defaults to the same address as the physical address.
    pub fn new(
        page_map_level4_physical: *mut PageTable,
        page_frame_allocator: PageFrameAllocator<'a>,
    ) -> Self {
        Self {
            page_table_manager: PageTableManager::new(page_map_level4_physical),
            page_frame_allocator,
        }
    }

    /// Returns mutable reference of physical page frame allocator owned by page table manager.
    pub fn pmm(&mut self) -> &mut PageFrameAllocator<'a> {
        &mut self.page_frame_allocator
    }

    pub fn get(&mut self) -> (&mut PageTableManager, &mut PageFrameAllocator<'a>) {
        (&mut self.page_table_manager, &mut self.page_frame_allocator)
    }

    pub fn manager(&mut self) -> &mut PageTableManager {
        &mut self.page_table_manager
    }

    /// Maps given virtual address to physical address
    pub fn map_memory(
        &mut self,
        virtual_memory: VirtualAddress,
        physical_memory: PhysicalAddress,
        flags: PageEntryFlags,
    ) -> Result<(), PageFrameAllocatorError> {
        let (manager, pmm) = self.get();
        manager.map_memory(virtual_memory, physical_memory, flags, pmm)
    }
}

/// Independent PageTableManager that does not own pmm.
#[derive(Debug)]
pub struct PageTableManager {
    page_map_level4_physical: *mut PageTable,
    page_map_level4_virtual: *mut PageTable,
    offset: VirtualAddress,
}

impl PageTableManager {
    /// Creates new page table manager instance. By default, a virtual `offset` of 0 is used. This can be changed manually using [`PageTableManager::update_offset()`]. The virtual address of the root page table defaults to the same address as the physical address.
    pub fn new(page_map_level4_physical: *mut PageTable) -> Self {
        Self {
            page_map_level4_physical,
            page_map_level4_virtual: page_map_level4_physical,
            offset: 0,
        }
    }

    /// Returns pointer to root page table physical address.
    pub fn pml4_physical(&self) -> *mut PageTable {
        self.page_map_level4_physical
    }

    /// Returns pointer to root page table virtual address.
    pub fn pml4_virtual(&self) -> *mut PageTable {
        self.page_map_level4_virtual
    }

    /// Returns the physical address associated with the provided virtual address. May return None if the mapping is not available.
    pub fn get_physical(&self, virtual_address: VirtualAddress) -> Option<PhysicalAddress> {
        self.get_entry_data(virtual_address)
            .map(|(addr, _flags)| addr)
    }

    /// Returns the physical address and page entry flags associated with the provieded virtual address. May return None if mapping is not available.
    pub fn get_entry_data(
        &self,
        virtual_address: VirtualAddress,
    ) -> Option<(PhysicalAddress, PageEntryFlags)> {
        let indexer = PageMapIndexer::new(virtual_address);
        let page_map_level4 = self.pml4_virtual();
        // Map Level 3
        let page_map_level3 = self.get_next_table(page_map_level4, indexer.pdp_i())?;
        // Map Level 2
        let page_map_level2 = self.get_next_table(page_map_level3, indexer.pd_i())?;
        // Map Level 1
        let page_map_level1 = self.get_next_table(page_map_level2, indexer.pt_i())?;

        let page_entry = &mut unsafe { &mut *page_map_level1 }.entries[indexer.p_i() as usize];

        Some((page_entry.address(), page_entry.flags()))
    }

    /// Used to switch to a different page table mapping.
    ///
    /// # Safety
    /// The caller must ensure that the new address is valid.
    pub unsafe fn update_pml4_physical(&mut self, new_address: PhysicalAddress) {
        self.page_map_level4_physical = new_address as *mut PageTable;
    }

    /// Used to switch to a different page table mapping.
    ///
    /// # Safety
    /// The caller must ensure that the new address is mapped and valid.
    pub unsafe fn update_pml4_virtual(&mut self, new_address: VirtualAddress) {
        self.page_map_level4_virtual = new_address as *mut PageTable;
    }

    /// Used to make page table manager accessible after enabling direct mapping paging scheme with offset. Updates page table manager to use offset when traversing page tables.
    ///
    /// # Safety
    /// The caller must ensure that the offset is valid.
    pub unsafe fn update_offset(&mut self, offset: VirtualAddress) {
        self.offset = offset;
    }

    /// Returns the virtual offset used to access page tables after enabling paging.
    pub fn offset(&self) -> VirtualAddress {
        self.offset
    }

    /// Maps given virtual address to physical address
    pub fn map_memory(
        &mut self,
        virtual_memory: VirtualAddress,
        physical_memory: PhysicalAddress,
        flags: PageEntryFlags,
        pmm: &mut PageFrameAllocator,
    ) -> Result<(), PageFrameAllocatorError> {
        let indexer = PageMapIndexer::new(virtual_memory);
        let page_map_level4 = self.pml4_virtual();
        let user = flags.contains(PageEntryFlags::USER_SUPER);
        // Map Level 3
        let page_map_level3 =
            self.get_or_create_next_table(page_map_level4, indexer.pdp_i(), pmm, user)?;
        // Map Level 2
        let page_map_level2 =
            self.get_or_create_next_table(page_map_level3, indexer.pd_i(), pmm, user)?;
        // Map Level 1
        let page_map_level1 =
            self.get_or_create_next_table(page_map_level2, indexer.pt_i(), pmm, user)?;

        let page_entry = &mut unsafe { &mut *page_map_level1 }.entries[indexer.p_i() as usize];

        page_entry.set_address(physical_memory);
        page_entry.set_flags(flags);

        Ok(())
    }

    /// Removes the mapping for given virtual address. Returns the physical address the virtual address previously pointed to.
    pub fn unmap(&mut self, virtual_memory: VirtualAddress) -> Option<PhysicalAddress> {
        let indexer = PageMapIndexer::new(virtual_memory);
        let page_map_level4 = self.pml4_virtual();
        // Map Level 3
        let page_map_level3 = self.get_next_table(page_map_level4, indexer.pdp_i())?;
        // Map Level 2
        let page_map_level2 = self.get_next_table(page_map_level3, indexer.pd_i())?;
        // Map Level 1
        let page_map_level1 = self.get_next_table(page_map_level2, indexer.pt_i())?;

        let page_entry = &mut unsafe { &mut *page_map_level1 }.entries[indexer.p_i() as usize];
        let physical_address = page_entry.address();

        page_entry.set_address(0);
        page_entry.set_flags(PageEntryFlags::empty());

        unsafe { self.invalidate_tlb_entry(physical_address) };

        Some(physical_address)
    }

    /// Used to update cache when unmapping addresses
    ///
    /// # Safety
    ///
    /// The caller has to ensure that the address is the appropriate one and no longer mapped.
    pub unsafe fn invalidate_tlb_entry(&self, virtual_address: VirtualAddress) {
        asm!("invlpg [{}]", in(reg) virtual_address as *const u8);
    }

    fn get_next_table(&self, current_table: *mut PageTable, index: u64) -> Option<*mut PageTable> {
        let entry = &mut unsafe { &mut *current_table }.entries[index as usize];
        if entry.flags().contains(PageEntryFlags::PRESENT) {
            Some((entry.address() + self.offset) as *mut PageTable)
        } else {
            None
        }
    }

    /// Gets pointer to next table or creates it if it does not exist yet.
    fn get_or_create_next_table(
        &mut self,
        current_table: *mut PageTable,
        index: u64,
        pmm: &mut PageFrameAllocator,
        user: bool,
    ) -> Result<*mut PageTable, PageFrameAllocatorError> {
        let entry = &mut unsafe { &mut *current_table }.entries[index as usize];

        if entry.flags().contains(PageEntryFlags::PRESENT) {
            // path to entry user accessible as well
            if user && !entry.flags().contains(PageEntryFlags::USER_SUPER) {
                entry.set_flags(entry.flags() | PageEntryFlags::USER_SUPER);
            }
            Ok((entry.address() + self.offset) as *mut PageTable)
        } else {
            let new_page = pmm.request_page()?;
            let new_table = (new_page + self.offset) as *mut PageTable;
            unsafe {
                // Zero out the new table
                core::ptr::write_bytes(new_table, 0, 1);
            }

            entry.set_address(new_page);
            entry.set_flags(
                PageEntryFlags::PRESENT
                    | PageEntryFlags::READ_WRITE
                    | if user {
                        PageEntryFlags::USER_SUPER
                    } else {
                        PageEntryFlags::empty()
                    },
            );

            Ok(new_table)
        }
    }
}
