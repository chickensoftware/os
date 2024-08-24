use core::arch::asm;

use crate::memory::{
    paging::{index::PageMapIndexer, PageEntryFlags, PageTable},
    PhysicalAddress,
    pmm::{PageFrameAllocator, PageFrameAllocatorError}, VirtualAddress,
};

/// Manages page tables
#[derive(Debug)]
pub struct PageTableManager<'a> {
    page_map_level4: *mut PageTable,
    pub(in crate::memory) page_frame_allocator: PageFrameAllocator<'a>,
    /// Used to make page table entries accessible after enabling the new paging scheme (direct mapping with offset)
    offset: VirtualAddress,
}

impl<'a> PageTableManager<'a> {
    /// Creates new page table manager instance. By default, a virtual `offset` of 0 is used. This can be changed manually using [`PageTableManager::update()`].
    pub fn new(
        page_map_level4: *mut PageTable,
        page_frame_allocator: PageFrameAllocator<'a>,
    ) -> Self {
        Self {
            page_map_level4,
            page_frame_allocator,
            offset: 0,
        }
    }

    /// Returns mutable reference of physical page frame allocator owned by page table manager.
    pub fn pmm(&mut self) -> &mut PageFrameAllocator<'a> {
        &mut self.page_frame_allocator
    }

    /// Returns pointer to root page table.
    pub fn pml4(&self) -> *mut PageTable {
        self.page_map_level4
    }

    /// Used to make page table manager accessible after enabling direct mapping paging scheme with offset. Updates page table manager to use offset when traversing page tables.
    ///
    /// # Safety
    /// The caller must ensure that the offset is valid.
    pub unsafe fn update(&mut self, offset: VirtualAddress) {
        self.offset = offset;
    }

    /// Maps given virtual address to physical address
    pub fn map_memory(
        &mut self,
        virtual_memory: VirtualAddress,
        physical_memory: PhysicalAddress,
        flags: PageEntryFlags,
    ) -> Result<(), PageFrameAllocatorError> {
        let indexer = PageMapIndexer::new(virtual_memory);
        let page_map_level4 = (self.page_map_level4 as u64 + self.offset) as *mut PageTable;
        // Map Level 3
        let page_map_level3 = self.get_or_create_next_table(page_map_level4, indexer.pdp_i())?;
        // Map Level 2
        let page_map_level2 = self.get_or_create_next_table(page_map_level3, indexer.pd_i())?;
        // Map Level 1
        let page_map_level1 = self.get_or_create_next_table(page_map_level2, indexer.pt_i())?;

        let page_entry = &mut unsafe { &mut *page_map_level1 }.entries[indexer.p_i() as usize];

        page_entry.set_address(physical_memory);
        page_entry.set_flags(flags);

        Ok(())
    }

    /// Removes the mapping for given virtual address. Returns the physical address the virtual address previously pointed to.
    pub fn unmap(
        &mut self,
        virtual_memory: VirtualAddress,
    ) -> Result<PhysicalAddress, PageFrameAllocatorError> {
        let indexer = PageMapIndexer::new(virtual_memory);
        let page_map_level4 = (self.page_map_level4 as u64 + self.offset) as *mut PageTable;
        // Map Level 3
        let page_map_level3 = self.get_or_create_next_table(page_map_level4, indexer.pdp_i())?;
        // Map Level 2
        let page_map_level2 = self.get_or_create_next_table(page_map_level3, indexer.pd_i())?;
        // Map Level 1
        let page_map_level1 = self.get_or_create_next_table(page_map_level2, indexer.pt_i())?;

        let page_entry = &mut unsafe { &mut *page_map_level1 }.entries[indexer.p_i() as usize];
        let physical_address = page_entry.address();

        page_entry.set_address(0);
        page_entry.set_flags(PageEntryFlags::empty());

        unsafe { self.invalidate_tlb_entry(physical_address) };

        Ok(physical_address)
    }

    /// Used to update cache when unmapping addresses
    ///
    /// # Safety
    ///
    /// The caller has to ensure that the address is the appropriate one and no longer mapped.
    unsafe fn invalidate_tlb_entry(&self, virtual_address: VirtualAddress) {
        asm!("invlpg [{}]", in(reg) virtual_address as *const u8);
    }

    /// Gets pointer to next table or creates it if it does not exist yet.
    fn get_or_create_next_table(
        &mut self,
        current_table: *mut PageTable,
        index: u64,
    ) -> Result<*mut PageTable, PageFrameAllocatorError> {
        let entry = &mut unsafe { &mut *current_table }.entries[index as usize];

        if entry.flags().contains(PageEntryFlags::PRESENT) {
            Ok((entry.address() + self.offset) as *mut PageTable)
        } else {
            let new_page = self.page_frame_allocator.request_page()?;
            let new_table = (new_page + self.offset) as *mut PageTable;
            unsafe {
                // Zero out the new table
                core::ptr::write_bytes(new_table, 0, 1);
            }

            entry.set_address(new_page);
            entry.set_flags(PageEntryFlags::PRESENT | PageEntryFlags::READ_WRITE);

            Ok(new_table)
        }
    }
}
