use core::marker::PhantomData;

use crate::memory::{PhysicalAddress, VirtualAddress};
use crate::memory::paging::{PageEntryFlags, PageTable};
use crate::memory::paging::index::PageMapIndexer;

pub trait PageFrameAllocator<'a, E> {
    fn request_page(&mut self) -> Result<PhysicalAddress, E>;
}

/// Manages page tables
#[derive(Debug)]
pub struct PageTableManager<A, E> {
    page_map_level4: *mut PageTable,
    page_frame_allocator: A,
    _marker: PhantomData<E>,
}

impl<'a, A: PageFrameAllocator<'a, E>, E> PageTableManager<A, E> {
    pub fn new(page_map_level4: *mut PageTable, page_frame_allocator: A) -> Self {
        Self {
            page_map_level4,
            page_frame_allocator,
            _marker: PhantomData,
        }
    }

    pub fn pml4(&self) -> *mut PageTable {
        self.page_map_level4
    }

    /// Maps given virtual address to physical address
    pub fn map_memory(
        &mut self,
        virtual_memory: VirtualAddress,
        physical_memory: PhysicalAddress,
        flags: PageEntryFlags
    ) -> Result<(), E> {
        let indexer = PageMapIndexer::new(virtual_memory);
        let page_map_level4 = self.page_map_level4;

        // Map Level 3
        let page_map_level3 =
            self.get_or_create_next_table(page_map_level4, indexer.pdp_i())?;
        // Map Level 2
        let page_map_level2 = self.get_or_create_next_table(page_map_level3, indexer.pd_i())?;
        // Map Level 1
        let page_map_level1 = self.get_or_create_next_table(page_map_level2, indexer.pt_i())?;

        let page_entry = &mut unsafe { &mut *page_map_level1 }.entries[indexer.p_i() as usize];

        page_entry.set_address(physical_memory);
        page_entry.set_flags(flags);

        Ok(())
    }

    pub fn frame_allocator(&mut self) -> &mut A {
        &mut self.page_frame_allocator
    }


    fn get_or_create_next_table(
        &mut self,
        current_table: *mut PageTable,
        index: u64,
    ) -> Result<*mut PageTable, E> {
        let entry = &mut unsafe { &mut *current_table }.entries[index as usize];

        if entry.flags().contains(PageEntryFlags::PRESENT) {
            Ok(entry.address() as *mut PageTable)
        } else {
            let new_page = self.page_frame_allocator.request_page()?;
            let new_table = new_page as *mut PageTable;
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