// kernel/src/memory/heap.rs — Kernel Heap Allocator

use linked_list_allocator::LockedHeap;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
        mapper::MapToError,
    },
};

/// Posisi awal heap kernel di virtual address space
/// (higher-half, di bawah kernel binary)
pub const HEAP_START: usize = 0xFFFF_C000_0000_0000;

/// Ukuran heap awal: 4MB
/// Bisa diperluas nanti dengan grow_heap()
pub const HEAP_SIZE:  usize = 4 * 1024 * 1024;

/// Global allocator — dipakai oleh `alloc` crate (Box, Vec, String, dll)
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Inisialisasi kernel heap.
/// Harus dipanggil sekali setelah page mapper siap.
pub fn init_heap(
    mapper:          &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    let heap_start = VirtAddr::new(HEAP_START as u64);
    let heap_end   = heap_start + HEAP_SIZE as u64 - 1u64;

    let start_page = Page::containing_address(heap_start);
    let end_page   = Page::containing_address(heap_end);

    // Map setiap halaman heap ke frame fisik baru
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    // Inisialisasi linked-list allocator
    unsafe {
        ALLOCATOR.lock().init(heap_start.as_mut_ptr(), HEAP_SIZE);
    }

    Ok(())
}

/// Statistik heap saat ini
pub struct HeapStats {
    pub used:  usize,
    pub free:  usize,
    pub total: usize,
}

pub fn stats() -> HeapStats {
    let alloc = ALLOCATOR.lock();
    let used  = alloc.used();
    let free  = alloc.free();
    HeapStats {
        used,
        free,
        total: used + free,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{boxed::Box, vec::Vec, string::String};

    #[test_case]
    fn test_box_allocation() {
        let v = Box::new(42u64);
        assert_eq!(*v, 42u64);
    }

    #[test_case]
    fn test_vec_growth() {
        let mut v: Vec<u32> = Vec::new();
        for i in 0..1000 {
            v.push(i);
        }
        assert_eq!(v.len(), 1000);
        assert_eq!(v[500], 500);
    }

    #[test_case]
    fn test_string_heap() {
        let s = String::from("NeoCore OS");
        assert_eq!(s, "NeoCore OS");
        assert_eq!(s.len(), 10);
    }

    #[test_case]
    fn test_large_allocation() {
        // 1MB allocation
        let v = alloc::vec![0u8; 1024 * 1024];
        assert_eq!(v.len(), 1024 * 1024);
    }

    #[test_case]
    fn test_multiple_alloc_dealloc() {
        // Buat banyak object, pastikan tidak OOM
        for _ in 0..100 {
            let b = Box::new([0u64; 32]); // 256 bytes
            drop(b); // Deallocate
        }
    }
}
