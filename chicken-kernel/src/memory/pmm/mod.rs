use core::{
    error::Error,
    fmt::{Display, Formatter},
    ptr::slice_from_raw_parts_mut,
    write,
};

use chicken_util::{
    memory::{
        paging::{manager::PageFrameAllocator, KERNEL_STACK_MAPPING_OFFSET},
        MemoryMap, MemoryType, PhysicalAddress,
    },
    PAGE_SIZE,
};

use crate::memory::pmm::bit_map::BitMap;

mod bit_map;

#[derive(Debug)]
pub(in crate::memory) struct BitMapAllocator<'a> {
    memory_map: MemoryMap,
    bit_map: BitMap<'a>,
    current_descriptor_index: usize,
    current_address: PhysicalAddress,
    free_memory: u64,
    used_memory: u64,
    reserved_memory: u64,
}

impl BitMapAllocator<'_> {
    /// Tries to initialize new bit map allocator with given memory map. May fail if memory map is empty or the setup of the bitmap failed.
    pub(in crate::memory) fn try_new(
        memory_map: MemoryMap,
    ) -> Result<Self, PageFrameAllocatorError> {
        // find memory region to store bitmap in
        let largest_memory_area = memory_map
            .descriptors()
            .iter()
            .filter(|area| area.r#type == MemoryType::Available)
            .max_by(|a, b| a.size().cmp(&b.size()))
            .ok_or(PageFrameAllocatorError::InvalidMemoryMap)?;
        let largest_memory_area_ptr = largest_memory_area.phys_start as *mut u8;

        // total memory size in bytes => / PAGE_SIZE is the amount of pages. In the bitmap each page is one bit => /8 gives out the amount of bits
        let total_pages = (memory_map.last_addr as usize + PAGE_SIZE - 1) / PAGE_SIZE;
        let bit_map_size = (total_pages + 7) / 8;

        let bit_map_buffer = unsafe {
            slice_from_raw_parts_mut(largest_memory_area_ptr, bit_map_size)
                .as_mut()
                .ok_or(PageFrameAllocatorError::InvalidMemoryMap)?
        };

        // clear any preexisting data
        bit_map_buffer.fill(0);

        let bit_map = BitMap {
            buffer: bit_map_buffer,
        };
        let free_memory = total_available_memory(&memory_map);

        let mut instance = Self {
            memory_map,
            bit_map,
            current_descriptor_index: 0,
            current_address: 0,
            free_memory,
            used_memory: 0,
            reserved_memory: 0,
        };

        // reserve frames for bitmap (subtract kernel mapping offset to get physical address (based on previously set up paging by bootloader))
        instance.reserve_frames(
            (&instance.bit_map) as *const BitMap as u64 - KERNEL_STACK_MAPPING_OFFSET,
            instance.bit_map.pages(),
        )?;

        // reserve reserved memory descriptors (including kernel code, data, stack)
        let mmap = instance.memory_map;
        mmap.descriptors()
            .iter()
            .filter(|desc| desc.r#type != MemoryType::Available)
            .try_for_each(|desc| {
                instance.reserve_frames(desc.phys_start, desc.num_pages as usize)
            })?;

        Ok(instance)
    }

    /// Returns the amount of free memory in bytes
    pub(in crate::memory) fn free_memory(&self) -> u64 {
        self.free_memory
    }
    /// Returns the amount of used memory in bytes
    pub(in crate::memory) fn used_memory(&self) -> u64 {
        self.used_memory
    }

    /// Returns the amount of reserved memory in bytes
    pub(in crate::memory) fn reserved_memory(&self) -> u64 {
        self.reserved_memory
    }
}

impl<'a> PageFrameAllocator<'a, PageFrameAllocatorError> for BitMapAllocator<'a> {
    /// Returns any available free page
    fn request_page(&mut self) -> Result<PhysicalAddress, PageFrameAllocatorError> {
        for desc_index in self.current_descriptor_index..self.memory_map.descriptors().len() {
            let desc = &self.memory_map.descriptors()[desc_index];
            if desc.r#type == MemoryType::Available {
                for addr in
                    (self.current_address.max(desc.phys_start)..desc.phys_end).step_by(PAGE_SIZE)
                {
                    let index = addr / PAGE_SIZE as u64;
                    if !self.bit_map.get(index)? {
                        self.allocate_frame(addr)?;
                        self.current_descriptor_index = desc_index;
                        self.current_address = addr + PAGE_SIZE as u64;
                        return Ok(addr);
                    }
                }
            }
            self.current_address = desc.phys_start;
        }
        // If no free page is found, start from the beginning next time
        self.current_descriptor_index = 0;
        self.current_address = 0;
        // todo: page frame swap
        Err(PageFrameAllocatorError::NoMoreFreePages)
    }
}

impl BitMapAllocator<'_> {
    // either allocates frame or does nothing if it is already free
    pub(in crate::memory) fn allocate_frame(
        &mut self,
        address: PhysicalAddress,
    ) -> Result<(), PageFrameAllocatorError> {
        let index = address / PAGE_SIZE as u64;
        if self.bit_map.get(index)? {
            return Ok(());
        }

        self.bit_map.set(index, true)?;
        self.free_memory -= PAGE_SIZE as u64;
        self.used_memory += PAGE_SIZE as u64;

        Ok(())
    }

    pub(in crate::memory) fn allocate_frames(
        &mut self,
        start_address: PhysicalAddress,
        page_count: usize,
    ) -> Result<(), PageFrameAllocatorError> {
        for i in 0..page_count {
            self.allocate_frame(start_address + (i * PAGE_SIZE) as u64)?;
        }

        Ok(())
    }

    // either frees frame or does nothing if it is already free
    pub(in crate::memory) fn free_frame(
        &mut self,
        address: PhysicalAddress,
    ) -> Result<(), PageFrameAllocatorError> {
        let index = address / PAGE_SIZE as u64;
        if !self.bit_map.get(index)? {
            return Ok(());
        }

        self.bit_map.set(index, false)?;
        self.free_memory += PAGE_SIZE as u64;
        self.used_memory -= PAGE_SIZE as u64;

        Ok(())
    }

    pub(in crate::memory) fn free_frames(
        &mut self,
        start_address: PhysicalAddress,
        page_count: usize,
    ) -> Result<(), PageFrameAllocatorError> {
        for i in 0..page_count {
            self.free_frame(start_address + (i * PAGE_SIZE) as u64)?;
        }

        Ok(())
    }

    // either reserves frame or does nothing if it is already free
    pub(in crate::memory) fn reserve_frame(
        &mut self,
        address: PhysicalAddress,
    ) -> Result<(), PageFrameAllocatorError> {
        let index = address / PAGE_SIZE as u64;
        if self.bit_map.get(index)? {
            return Ok(());
        }

        self.bit_map.set(index, true)?;
        self.free_memory -= PAGE_SIZE as u64;
        self.reserved_memory += PAGE_SIZE as u64;

        Ok(())
    }

    pub(in crate::memory) fn reserve_frames(
        &mut self,
        start_address: PhysicalAddress,
        page_count: usize,
    ) -> Result<(), PageFrameAllocatorError> {
        for i in 0..page_count {
            self.reserve_frame(start_address + (i * PAGE_SIZE) as u64)?;
        }

        Ok(())
    }

    // either frees reserved frame or does nothing if it is already free
    pub(in crate::memory) fn free_reserved_frame(
        &mut self,
        address: PhysicalAddress,
    ) -> Result<(), PageFrameAllocatorError> {
        let index = address / PAGE_SIZE as u64;
        if !self.bit_map.get(index)? {
            return Ok(());
        }

        self.bit_map.set(index, false)?;
        self.free_memory += PAGE_SIZE as u64;
        self.reserved_memory -= PAGE_SIZE as u64;

        Ok(())
    }

    pub(in crate::memory) fn free_reserved_frames(
        &mut self,
        start_address: PhysicalAddress,
        page_count: usize,
    ) -> Result<(), PageFrameAllocatorError> {
        for i in 0..page_count {
            self.free_reserved_frame(start_address + (i * PAGE_SIZE) as u64)?;
        }

        Ok(())
    }
}

/// Returns total amount of available memory in bytes based on memory map.
fn total_available_memory(mmap: &MemoryMap) -> u64 {
    mmap.descriptors()
        .iter()
        .filter(|desc| desc.r#type == MemoryType::Available)
        .map(|desc| desc.size())
        .sum()
}

#[derive(Copy, Clone, Debug)]
pub(in crate::memory) enum PageFrameAllocatorError {
    InvalidBitMapIndex,
    InvalidMemoryMap,
    NoMoreFreePages,
}

impl Display for PageFrameAllocatorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for PageFrameAllocatorError {}
