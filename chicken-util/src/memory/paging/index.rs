use crate::memory::VirtualAddress;

/// Used to convert virtual address to page map indices
#[derive(Copy, Clone, Debug)]
pub struct PageMapIndexer {
    page_directory_pointer_index: u64, // level 4
    page_directory_index: u64,         // level 3
    page_table_index: u64,             // level 2
    page_index: u64,                   // level 1
}

impl PageMapIndexer {
    pub fn new(mut virtual_address: VirtualAddress) -> Self {
        virtual_address >>= 12;
        let page_index = virtual_address & 0x1ff;
        virtual_address >>= 9;
        let page_table_index = virtual_address & 0x1ff;
        virtual_address >>= 9;
        let page_directory_index = virtual_address & 0x1ff;
        virtual_address >>= 9;
        let page_directory_pointer_index = virtual_address & 0x1ff;

        Self {
            page_directory_pointer_index,
            page_directory_index,
            page_table_index,
            page_index,
        }
    }

    /// Returns Page Index
    pub fn p_i(&self) -> u64 {
        self.page_index
    }
    /// Returns Page Table Index
    pub fn pt_i(&self) -> u64 {
        self.page_table_index
    }

    /// Returns Page Directory Index
    pub fn pd_i(&self) -> u64 {
        self.page_directory_index
    }

    /// Returns Page Directory Pointer Index
    pub fn pdp_i(&self) -> u64 {
        self.page_directory_pointer_index
    }
}
