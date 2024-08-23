use alloc::alloc::GlobalAlloc;
use core::{alloc::Layout, cell::OnceCell, ptr, ptr::NonNull};

use chicken_util::{memory::VirtualAddress, PAGE_SIZE};

use crate::{
    memory::{
        align_up,
        kheap::{HeapError, MAX_KERNEL_HEAP_PAGE_COUNT},
    },
    scheduling::spin::SpinLock,
};

#[derive(Debug)]
struct ListNode {
    size: usize,
    free: bool,
    next: Option<NonNull<ListNode>>,
    prev: Option<NonNull<ListNode>>,
}

#[derive(Clone, Debug)]
pub(super) struct LinkedListAllocator {
    heap_size: usize,
    head: Option<NonNull<ListNode>>,
}

impl LinkedListAllocator {
    /// Attempts to initialize new linked list allocator. May return None if the size is insufficient.
    pub(super) fn try_new(heap_start: VirtualAddress, heap_size: usize) -> Result<Self, HeapError> {
        if heap_size < size_of::<ListNode>() {
            Err(HeapError::InvalidBlockSize(heap_size))
        } else {
            let start_node = unsafe { NonNull::new_unchecked(heap_start as *mut ListNode) };
            // initialize start node that spans over the entire heap size
            unsafe {
                start_node.write(ListNode {
                    size: heap_size - size_of::<ListNode>(),
                    free: true,
                    next: None,
                    prev: None,
                });
            }
            Ok(Self {
                heap_size,
                head: Some(start_node),
            })
        }
    }
}

impl LinkedListAllocator {

    /// Tries to find a fitting list node in the linked list to home a new block of allocated memory.
    fn find_fit(&mut self, size: usize) -> Result<NonNull<ListNode>, HeapError> {
        let mut current = self.head;
        while let Some(node) = current {
            unsafe {
                if node.as_ref().free && node.as_ref().size >= size {
                    return Ok(node);
                }
                current = node.as_ref().next;
            }
        }
        // no fit can be found (OOM)
        Err(HeapError::OutOfMemory)
    }

    /// Splits a list node into two in order to allocate new memory on the heap. May fail if the size if too large.
    fn split_block(&mut self, mut node: NonNull<ListNode>, size: usize) -> Result<(), HeapError> {
        unsafe {
            let node_ref = node.as_mut();
            let remaining_size = node_ref
                .size
                .checked_sub(size)
                .ok_or(HeapError::InvalidBlockSize(node_ref.size))?;
            if remaining_size >= size_of::<ListNode>() {
                let new_node_ptr = align_up(
                    node.as_ptr() as u64 + (size_of::<ListNode>() + size) as u64,
                    align_of::<ListNode>(),
                ) as *mut ListNode;

                let new_node = NonNull::new_unchecked(new_node_ptr);

                new_node.write(ListNode {
                    size: remaining_size - size_of::<ListNode>(),
                    free: true,
                    next: node_ref.next,
                    prev: Some(node),
                });

                if let Some(mut next_node) = node_ref.next {
                    next_node.as_mut().prev = Some(new_node);
                }

                node_ref.next = Some(new_node);
                node_ref.size = size;
            } else {
                // if remaining size is too small to split, just use the whole block
                node_ref.size = remaining_size + size;
            }

            node_ref.free = false;
        }

        Ok(())
    }

    /// Merges two list nodes. Used when freeing memory.
    ///
    /// # Safety
    /// Caller has to ensure that `node` points to a valid `ListNode`.
    unsafe fn merge_blocks(&mut self, mut node: NonNull<ListNode>) {
        let node_ref = node.as_mut();

        // merge with next node if it's free
        if let Some(next_node) = node_ref.next {
            if next_node.as_ref().free {
                node_ref.size += next_node.as_ref().size + size_of::<ListNode>();
                node_ref.next = next_node.as_ref().next;

                if let Some(mut next_next_node) = next_node.as_ref().next {
                    next_next_node.as_mut().prev = Some(node);
                }
            }
        }

        // merge with previous node if it's free
        if let Some(mut prev_node) = node_ref.prev {
            if prev_node.as_ref().free {
                prev_node.as_mut().size += node_ref.size + size_of::<ListNode>();
                prev_node.as_mut().next = node_ref.next;

                if let Some(mut next_node) = node_ref.next {
                    next_node.as_mut().prev = Some(prev_node);
                }
            }
        }
    }

    /// Attempts to expand the memory mapped for the heap allocator.
    fn expand(&mut self, size: usize) -> Result<(), HeapError> {
        let total_size = align_up(
            (size + size_of::<ListNode>()) as u64,
            align_of::<ListNode>(),
        );

        let old_heap_page_count = (self.heap_size + PAGE_SIZE - 1) / PAGE_SIZE;
        let new_heap_page_count =
            (total_size as usize + PAGE_SIZE - 1) / PAGE_SIZE + old_heap_page_count;
        // check if expansion is valid
        if new_heap_page_count > MAX_KERNEL_HEAP_PAGE_COUNT {
            return Err(HeapError::OutOfMemory);
        }

        // todo: expand heap
        unimplemented!("heap expansion");
    }
}

unsafe impl GlobalAlloc for SpinLock<OnceCell<LinkedListAllocator>> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap = &mut self.lock();

        if let Some(heap) = heap.get_mut() {
            let size = align_up(layout.size() as u64, layout.align()) as usize;
            if let Ok(fit_node) = heap.find_fit(size) {
                if heap.split_block(fit_node, size).is_ok() {
                    return fit_node.as_ptr().add(1) as *mut u8;
                }
            } else {
                // expand heap
                if heap.expand(size).is_ok() {
                    self.alloc(layout);
                }
            }
        }
        // heap has not been initialized or OOM
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let mut heap = self.lock();
        if let Some(heap) = heap.get_mut() {
            let node_ptr = (ptr as *mut ListNode).sub(1);

            let mut node = NonNull::new_unchecked(node_ptr);
            node.as_mut().free = true;
            heap.merge_blocks(node);
        }
    }
}
