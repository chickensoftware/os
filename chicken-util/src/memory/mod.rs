use core::fmt::{Debug, Display, Formatter};
use core::slice;

pub mod paging;
pub type VirtualAddress = u64;
pub type PhysicalAddress = u64;
#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct MemoryMap {
    /// Pointer to memory map descriptors
    pub descriptors: *mut MemoryDescriptor,
    /// Length of memory that descriptors occupy in bytes
    pub descriptors_len: u64,
    /// First valid address of physical address space
    pub first_addr: PhysicalAddress,
    /// Last valid address of physical address space
    pub last_addr: PhysicalAddress,

}

impl MemoryMap {
    pub fn descriptors(&self) -> &[MemoryDescriptor] {
        unsafe { slice::from_raw_parts(self.descriptors, self.descriptors_len as usize) }
    }
}


#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryDescriptor {
    pub phys_start: PhysicalAddress,
    pub phys_end: PhysicalAddress,
    pub num_pages: u64,
    pub r#type: MemoryType,
}

impl MemoryDescriptor {
    /// Size of memory of descriptor in bytes
    pub fn size(&self) -> u64 {
        self.phys_end - self.phys_start
    }
}

impl Debug for MemoryDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", format_args!("Memory Descriptor {{ phys_start: {:#x}, phys_end: {:#x}, num_pages: {}, type: {:?} }}", self.phys_start, self.phys_end, self.num_pages, self.r#type))
    }
}


impl Display for MemoryDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[repr(u8)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum MemoryType {
    Available = 0,
    Reserved = 1,
    /// kernel code file
    KernelCode = 2,
    /// kernel stack
    KernelStack = 3,
    /// boot info, memory map
    KernelData = 4,
}