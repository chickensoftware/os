use alloc::alloc::dealloc;
use core::{
    alloc::Layout,
    cell::OnceCell,
    error::Error,
    fmt::{Debug, Display, Formatter},
    ptr::NonNull,
};

use chicken_util::{
    memory::{paging::PageEntryFlags, pmm::PageFrameAllocatorError, VirtualAddress},
    PAGE_SIZE,
};

use crate::{
    memory::{
        align_up,
        paging::{PagingError, PTM},
        vmm::object::{VmFlags, VmObject},
    },
    scheduling::spin::{Guard, SpinLock},
};

pub(in crate::memory) const VIRTUAL_VMM_BASE: u64 = 0xFFFF_FFFF_C000_0000;
/// Maximum amount of pages allowed for vmm objects' memory
pub(in crate::memory) const VMM_PAGE_COUNT: usize = PAGE_SIZE * 256; // 1 MiB

pub(crate) mod object;

pub(crate) static VMM: GlobalVirtualMemoryManager = GlobalVirtualMemoryManager::new();

#[derive(Debug)]
pub(crate) struct GlobalVirtualMemoryManager {
    inner: SpinLock<OnceCell<VirtualMemoryManager>>,
}

unsafe impl Send for GlobalVirtualMemoryManager {}
unsafe impl Sync for GlobalVirtualMemoryManager {}

impl GlobalVirtualMemoryManager {
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(OnceCell::new()),
        }
    }

    pub(super) fn init(vmm_start: VirtualAddress, vmm_page_count: usize) {
        let vmm = VMM.inner.lock();
        vmm.get_or_init(|| VirtualMemoryManager::new(vmm_start, vmm_page_count));
    }

    pub(crate) fn lock(&self) -> Guard<OnceCell<VirtualMemoryManager>> {
        self.inner.lock()
    }
}

#[allow(dead_code)] // otherwise, clippy complains about the flags field being 'unused'
/// Uses global page table manager and kernel heap to keep track of allocated virtual memory objects with specific permissions.
#[derive(Debug)]
pub(crate) struct VirtualMemoryManager {
    head: Option<NonNull<VmObject>>,
    vmm_start: VirtualAddress,
    vmm_page_count: usize,
    pages_allocated: usize,
}

impl VirtualMemoryManager {
    pub(super) fn new(vmm_start: VirtualAddress, vmm_page_count: usize) -> Self {
        Self {
            vmm_start,
            vmm_page_count,
            head: None,
            pages_allocated: 0,
        }
    }
}

impl VirtualMemoryManager {
    /// Allocates a new virtual memory object according to the given arguments, returns either a virtual address pointing to the object or a PagingError in case of an invalid length or allocation type.
    pub(crate) fn alloc(
        &mut self,
        length: usize,
        flags: VmFlags,
        allocation_type: AllocationType,
    ) -> Result<VirtualAddress, VmmError> {
        let mut ptm = PTM.lock();
        if let Some(ptm) = ptm.get_mut() {
            // align length to next valid page size
            let length = align_up(length as u64, PAGE_SIZE) as usize;
            let mut base = 0;
            let mut current = self.head;

            // check if there is enough space for vmm object
            if self.pages_allocated + (length / PAGE_SIZE) > self.vmm_page_count {
                return Err(VmmError::OutOfMemory);
            }

            // allocate first object
            if current.is_some() {
                // allocate new vm object struct on heap
                while let Some(mut object) = current {
                    let current_ref = unsafe { object.as_mut() };

                    if let Some(mut prev) = current_ref.prev {
                        let prev_ref = unsafe { prev.as_mut() };
                        let new_base = prev_ref.base + prev_ref.length as u64;

                        // allocate between previous object and current one
                        if new_base + (length as u64) < current_ref.base {
                            base = new_base;
                            let new_object = unsafe {
                                VmObject::alloc_new(base, length, flags, current, current_ref.prev)
                            };

                            prev_ref.next = Some(new_object);
                            current_ref.prev = Some(new_object);
                            break;
                        }
                    } else {
                        // allocate new object before the first one, if possible
                        if (length as u64) < current_ref.base {
                            base = 0;
                            let new_object =
                                unsafe { VmObject::alloc_new(base, length, flags, current, None) };
                            current_ref.prev = Some(new_object);
                            break;
                        }
                    }

                    // allocate after last object
                    if current_ref.next.is_none() {
                        base = current_ref.base + current_ref.length as u64;
                        let new_object =
                            unsafe { VmObject::alloc_new(base, length, flags, None, current) };
                        current_ref.next = Some(new_object);
                        break;
                    }
                    // continue with new object
                    current = current_ref.next;
                }
            } else {
                let new_object = unsafe { VmObject::alloc_new(base, length, flags, None, None) };
                self.head = Some(new_object);
            }

            // map pages for newly allocated vm object
            let page_count = length / PAGE_SIZE;
            self.pages_allocated += page_count;
            // immediate backing
            for page in 0..page_count {
                let physical_address = match allocation_type {
                    AllocationType::AnyPages => ptm.pmm().request_page().map_err(VmmError::from)?,
                    AllocationType::Address(address) => address + (page * PAGE_SIZE) as u64,
                };
                let virtual_address = self.vmm_start + base + (page * PAGE_SIZE) as u64;
                ptm.map_memory(
                    virtual_address,
                    physical_address,
                    PageEntryFlags::from(flags),
                )
                .map_err(VmmError::from)?;
                // clear newly allocated region
                if !flags.contains(VmFlags::MMIO) && flags.contains(VmFlags::WRITE) {
                    unsafe {
                        (virtual_address as *mut u8).write_bytes(0, PAGE_SIZE);
                    }
                }
            }

            Ok(self.vmm_start + base)
        } else {
            Err(VmmError::PageTableManagerError(
                PagingError::GlobalPageTableManagerUninitialized,
            ))
        }
    }

    pub(crate) fn free(&mut self, address: VirtualAddress) -> Result<(), VmmError> {
        assert!(address >= self.vmm_start, "Invalid VMM object address");
        let mut ptm = PTM.lock();
        if let Some(ptm) = ptm.get_mut() {
            let mut current = self.head;
            while let Some(current_ref) = current {
                let current_ref = unsafe { current_ref.as_ref() };

                // check for requested object
                if current_ref.base == address - self.vmm_start {
                    let page_count = current_ref.length / PAGE_SIZE;
                    // free regions in vmm memory segment
                    for page in 0..page_count {
                        // unmap virtual address
                        let physical_address = ptm
                            .unmap(address + (page * PAGE_SIZE) as u64)
                            .map_err(VmmError::from)?;

                        // free physical page frames
                        if !current_ref.flags.contains(VmFlags::MMIO) {
                            ptm.pmm()
                                .free_frame(physical_address)
                                .map_err(VmmError::from)?;
                        }
                    }

                    self.pages_allocated -= page_count;

                    // remove object from linked list
                    let heap_ptr = if let Some(mut prev) = current_ref.prev {
                        let prev_ref = unsafe { prev.as_mut() };
                        let heap_ptr = prev_ref.next.unwrap().as_ptr();
                        prev_ref.next = current_ref.next;
                        heap_ptr
                    } else {
                        let heap_ptr = self.head.unwrap().as_ptr();
                        self.head = current_ref.next;

                        heap_ptr
                    };

                    if let Some(mut next) = current_ref.next {
                        let next_ref = unsafe { next.as_mut() };
                        next_ref.prev = current_ref.prev;
                    }

                    // deallocate vmm struct from heap
                    unsafe {
                        dealloc(heap_ptr as *mut u8, Layout::new::<VmObject>());
                    }

                    return Ok(());
                }

                current = current_ref.next;
            }

            Err(VmmError::RequestedVmObjectIsNotAllocated(address))
        } else {
            Err(VmmError::PageTableManagerError(
                PagingError::GlobalPageTableManagerUninitialized,
            ))
        }
    }
}

/// Specifies the type of allocation for the virtual memory object
#[derive(Copy, Clone, Debug)]
pub(crate) enum AllocationType {
    AnyPages,
    Address(VirtualAddress),
}

#[derive(Copy, Clone)]
pub(crate) enum VmmError {
    PageTableManagerError(PagingError),
    PageFrameAllocatorError(PageFrameAllocatorError),
    RequestedVmObjectIsNotAllocated(VirtualAddress),
    OutOfMemory,
    GlobalVirtualMemoryManagerUninitialized,
}

impl Debug for VmmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            VmmError::OutOfMemory => write!(f, "VmmError: Out of memory."),
            VmmError::GlobalVirtualMemoryManagerUninitialized => write!(
                f,
                "VmmError: Global virtual memory manager has not been initialized."
            ),
            VmmError::PageTableManagerError(value) => {
                write!(f, "VmmError: {}.", value)
            }
            VmmError::PageFrameAllocatorError(value) => {
                write!(f, "VmmError: {}.", value)
            }
            VmmError::RequestedVmObjectIsNotAllocated(address) => {
                write!(
                    f,
                    "VmmError: Requested VmObject is not allocated. Address: {:#x}.",
                    address
                )
            }
        }
    }
}

impl Display for VmmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for VmmError {}

impl From<PagingError> for VmmError {
    fn from(value: PagingError) -> Self {
        Self::PageTableManagerError(value)
    }
}

impl From<PageFrameAllocatorError> for VmmError {
    fn from(value: PageFrameAllocatorError) -> Self {
        Self::PageFrameAllocatorError(value)
    }
}
