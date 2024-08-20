use crate::memory::pmm::PageFrameAllocatorError;
use crate::PAGE_SIZE;

#[repr(transparent)]
#[derive(Debug)]
pub struct BitMap<'a> {
    pub buffer: &'a mut [u8],
}

impl<'a> BitMap<'a> {
    /// Gets the bit on a certain index (in bits)
    pub fn get(&self, index: u64) -> Result<bool, PageFrameAllocatorError> {
        let byte_index = index / 8;
        if byte_index >= self.buffer.len() as u64 {
            return Err(PageFrameAllocatorError::InvalidBitMapIndex);
        }
        let bit_index = index % 8;
        let bit_indexer = 0b10000000 >> bit_index;
        Ok((self.buffer[byte_index as usize] & bit_indexer) != 0)
    }

    /// Sets the bit on a certain index (in bits), returns whether the action succeeds
    pub fn set(&mut self, index: u64, value: bool) -> Result<(), PageFrameAllocatorError> {
        let byte_index = index / 8;
        if byte_index >= self.buffer.len() as u64 {
            return Err(PageFrameAllocatorError::InvalidBitMapIndex);
        }
        let bit_index = index % 8;

        let bit_indexer = 0b10000000 >> bit_index;
        // set index to false
        self.buffer[byte_index as usize] &= !bit_indexer;

        if value {
            self.buffer[byte_index as usize] |= bit_indexer;
        }

        Ok(())
    }

    pub fn pages(&self) -> usize {
        (size_of::<BitMap>() + PAGE_SIZE - 1) / PAGE_SIZE
    }
}
