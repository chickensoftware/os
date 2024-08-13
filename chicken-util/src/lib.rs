#![no_std]

use crate::memory::MemoryMap;

pub mod memory;
pub const PAGE_SIZE: usize = 4096;

#[derive(Clone, Debug)]
pub struct BootInfo {
    pub memory_map: MemoryMap
}
