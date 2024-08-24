#![no_std]

use crate::graphics::font::Font;
use crate::graphics::framebuffer::FrameBufferMetadata;
use crate::memory::{MemoryMap, PhysicalAddress};

pub mod memory;
pub mod graphics;

pub const PAGE_SIZE: usize = 4096;

#[derive(Clone, Debug)]
pub struct BootInfo {
    pub memory_map: MemoryMap,
    pub framebuffer_metadata: FrameBufferMetadata,
    pub font: Font,
    pub pmm_address: PhysicalAddress,
    pub rsdp: u64,
}
