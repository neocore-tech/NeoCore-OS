# 🧠 MEMORY — Memory Management NeoCore OS

---

## 1. Overview

Memory management adalah salah satu komponen paling kritis dalam kernel. NeoCore OS mengimplementasikan:

1. **Physical Memory Manager** — Frame allocator (mengelola halaman fisik 4KB)
2. **Virtual Memory Manager** — Page table (mapping virtual → physical)
3. **Kernel Heap** — Alokasi dinamis untuk kernel (`alloc` crate)

---

## 2. Konsep Dasar

### 2.1 Ukuran Halaman
```
Page Size: 4 KB (4096 bytes) = standar x86_64
Huge Page:  2 MB (menggunakan 2-level paging, untuk performa)
Giant Page: 1 GB (menggunakan 1-level paging)
```

### 2.2 4-Level Paging (x86_64)

```
Virtual Address 48-bit:
┌──────┬──────┬──────┬──────┬───────────────┐
│ PML4 │ PDPT │  PD  │  PT  │    Offset     │
│ 9bit │ 9bit │ 9bit │ 9bit │    12bit      │
└──────┴──────┴──────┴──────┴───────────────┘
  [47:39] [38:30] [29:21] [20:12]   [11:0]

PML4 (Page Map Level 4)  → 512 entries × 8 bytes = 4KB
  └── PDPT (Page Dir Pointer Table) → 512 entries × 8 bytes = 4KB
        └── PD (Page Directory) → 512 entries × 8 bytes = 4KB
              └── PT (Page Table) → 512 entries × 8 bytes = 4KB
                    └── Physical Page (4KB data)
```

### 2.3 Virtual Address Space Layout

```
0x0000_0000_0000_0000 ┬─ User Space Start
                       │  (0 - 128TB untuk user processes)
                       │
0x0000_7FFF_FFFF_FFFF ┘─ User Space End

──── NON-CANONICAL GAP ────

0xFFFF_8000_0000_0000 ┬─ Kernel Space Start
                       │
0xFFFF_8000_0000_0000 │  Physical Memory Direct Map
  + physical_mem_size  │  (semua RAM dipetakan di sini)
                       │
0xFFFF_C000_0000_0000 │  Kernel Heap
                       │  (4TB reserved untuk heap)
                       │
0xFFFF_E000_0000_0000 │  vmalloc area (opsional)
                       │
0xFFFF_FFFF_8000_0000 │  Kernel Code & Static Data
                       │  (.text, .rodata, .data, .bss)
                       │
0xFFFF_FFFF_FFFF_FFFF ┘─ Kernel Space End
```

---

## 3. Physical Frame Allocator

```rust
// src/memory/frame_allocator.rs
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};

/// Frame allocator menggunakan bitmap atau linked-list
/// Implementasi sederhana: iterasi memory map dari bootloader
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// SAFETY: Caller harus memastikan memory_map valid
    /// dan frame-frame yang digunakan belum dialokasikan
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Iterator semua usable frames
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| {
            r.kind == MemoryRegionKind::Usable
        });
        let addr_ranges = usable_regions.map(|r| {
            r.start..r.end
        });
        let frame_addrs = addr_ranges.flat_map(|r| {
            r.step_by(4096) // 4096 bytes per frame
        });
        frame_addrs.map(|addr| {
            PhysFrame::containing_address(PhysAddr::new(addr))
        })
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

// === Bitmap Frame Allocator (Production-grade) ===
// Lebih efisien untuk free frame support

pub struct BitmapFrameAllocator {
    bitmap: &'static mut [u8],
    total_frames: usize,
    free_frames: usize,
    next_free: usize,
}

impl BitmapFrameAllocator {
    pub fn new(bitmap: &'static mut [u8], total_frames: usize) -> Self {
        BitmapFrameAllocator {
            bitmap,
            total_frames,
            free_frames: total_frames,
            next_free: 0,
        }
    }

    fn set_used(&mut self, frame_idx: usize) {
        self.bitmap[frame_idx / 8] |= 1 << (frame_idx % 8);
        self.free_frames -= 1;
    }

    fn set_free(&mut self, frame_idx: usize) {
        self.bitmap[frame_idx / 8] &= !(1 << (frame_idx % 8));
        self.free_frames += 1;
    }

    fn is_free(&self, frame_idx: usize) -> bool {
        self.bitmap[frame_idx / 8] & (1 << (frame_idx % 8)) == 0
    }

    pub fn free_frame(&mut self, frame: PhysFrame) {
        let idx = frame.start_address().as_u64() as usize / 4096;
        self.set_free(idx);
        if idx < self.next_free {
            self.next_free = idx;
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if self.free_frames == 0 {
            return None;
        }
        // Cari frame bebas mulai dari next_free
        for i in self.next_free..self.total_frames {
            if self.is_free(i) {
                self.set_used(i);
                self.next_free = i + 1;
                let addr = PhysAddr::new((i * 4096) as u64);
                return Some(PhysFrame::containing_address(addr));
            }
        }
        None
    }
}
```

---

## 4. Virtual Memory / Paging

```rust
// src/memory/paging.rs
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable,
        Page, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

/// Inisialisasi page mapper
/// SAFETY: physical_memory_offset harus valid (dari bootloader)
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr = virt.as_mut_ptr();

    OffsetPageTable::new(&mut *page_table_ptr, physical_memory_offset)
}

/// Map satu halaman virtual ke frame fisik
pub fn map_page(
    page: Page,
    frame: PhysFrame,
    flags: PageTableFlags,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)?.flush();
    }
    Ok(())
}

/// Map range alamat fisik ke virtual (identity-ish mapping)
pub fn map_physical_range(
    phys_start: PhysAddr,
    size: usize,
    virt_start: VirtAddr,
    flags: PageTableFlags,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let pages = size.div_ceil(4096);
    for i in 0..pages {
        let page = Page::containing_address(virt_start + (i * 4096) as u64);
        let frame = PhysFrame::containing_address(phys_start + (i * 4096) as u64);
        map_page(page, frame, flags, mapper, frame_allocator)
            .expect("Failed to map page");
    }
}

/// Unmap halaman (untuk free memori user space)
pub fn unmap_page(
    page: Page,
    mapper: &mut impl Mapper<Size4KiB>,
) -> Option<PhysFrame> {
    use x86_64::structures::paging::mapper::UnmapError;
    mapper.unmap(page).ok().map(|(frame, flush)| {
        flush.flush();
        frame
    })
}
```

---

## 5. Kernel Heap Allocator

```rust
// src/memory/heap.rs
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper,
        Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

// Posisi dan ukuran kernel heap
pub const HEAP_START: usize = 0xFFFF_C000_0000_0000;
pub const HEAP_SIZE:  usize = 4 * 1024 * 1024; // 4MB awal (bisa grow)

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end   = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page   = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    // Inisialisasi linked list allocator
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

/// Grow heap jika habis (opsional)
pub fn grow_heap(
    additional_size: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    // TODO: Implementasi dynamic heap growth
    todo!("Heap growth not yet implemented")
}
```

---

## 6. Memory Statistics

```rust
// src/memory/stats.rs
use core::sync::atomic::{AtomicUsize, Ordering};

static TOTAL_PHYSICAL_BYTES: AtomicUsize = AtomicUsize::new(0);
static USED_PHYSICAL_BYTES: AtomicUsize  = AtomicUsize::new(0);
static HEAP_USED_BYTES: AtomicUsize      = AtomicUsize::new(0);

pub struct MemStats {
    pub total_physical: usize,
    pub used_physical: usize,
    pub free_physical: usize,
    pub heap_used: usize,
    pub heap_total: usize,
}

pub fn get_stats() -> MemStats {
    let total = TOTAL_PHYSICAL_BYTES.load(Ordering::Relaxed);
    let used  = USED_PHYSICAL_BYTES.load(Ordering::Relaxed);
    MemStats {
        total_physical: total,
        used_physical:  used,
        free_physical:  total.saturating_sub(used),
        heap_used:      HEAP_USED_BYTES.load(Ordering::Relaxed),
        heap_total:     super::heap::HEAP_SIZE,
    }
}

pub fn print_stats() {
    let stats = get_stats();
    println!("=== Memory Statistics ===");
    println!("Physical RAM : {:.1} MB total", 
             stats.total_physical as f64 / 1_048_576.0);
    println!("             : {:.1} MB used", 
             stats.used_physical as f64 / 1_048_576.0);
    println!("             : {:.1} MB free", 
             stats.free_physical as f64 / 1_048_576.0);
    println!("Kernel Heap  : {} KB / {} KB", 
             stats.heap_used / 1024, 
             stats.heap_total / 1024);
}
```

---

## 7. Contoh Penggunaan di Kernel

```rust
// Setelah heap diinisialisasi, kita bisa pakai alloc

extern crate alloc;
use alloc::{vec, vec::Vec, string::String, boxed::Box};

fn example_allocations() {
    // Vec dinamis
    let mut v: Vec<u32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);

    // String di heap
    let s = String::from("Hello, kernel heap!");

    // Box untuk trait objects
    let b: Box<dyn Fn()> = Box::new(|| println!("Closure on heap!"));
    b();

    // Struct di heap
    let node = Box::new(ListNode {
        value: 42,
        next: None,
    });
}

struct ListNode {
    value: u32,
    next: Option<Box<ListNode>>,
}
```

---

## 8. Page Fault Handler

```rust
// Ditangani di src/interrupts/handlers.rs
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    let fault_addr = Cr2::read().unwrap();

    // Cek apakah ini stack overflow
    if fault_addr.as_u64() < 0x1000 {
        panic!("Null pointer dereference at {:#x}", fault_addr.as_u64());
    }

    // Cek apakah halaman perlu di-swap (demand paging)
    if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
        // Copy-on-Write atau demand allocation
        // TODO: Implementasi CoW
    }

    // Jika tidak bisa ditangani, panic
    panic!(
        "PAGE FAULT at {:#x}\nError: {:?}\n{:#?}",
        fault_addr, error_code, stack_frame
    );
}
```

---

## 9. TLB (Translation Lookaside Buffer)

```rust
// TLB flush otomatis via page table flush
// Manual flush jika diperlukan:

pub fn flush_tlb_all() {
    use x86_64::registers::control::Cr3;
    let (frame, flags) = Cr3::read();
    unsafe { Cr3::write(frame, flags) }; // Reload CR3 = flush semua TLB
}

pub fn flush_tlb_page(addr: VirtAddr) {
    unsafe {
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) addr.as_u64(),
            options(nostack)
        );
    }
}
```

---

## 10. Referensi

- [Writing an OS in Rust — Memory Management](https://os.phil-opp.com/paging-introduction/)
- [OSDev — Paging](https://wiki.osdev.org/Paging)
- [Intel Manual — Volume 3A: System Programming Guide](https://software.intel.com/content/www/us/en/develop/articles/intel-sdm.html)
- [linked_list_allocator crate](https://docs.rs/linked_list_allocator)
- [x86_64 Paging](https://docs.rs/x86_64/latest/x86_64/structures/paging/index.html)
