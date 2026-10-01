// kernel/src/memory/frame_allocator.rs — Physical Frame Allocator

use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

/// Frame allocator berbasis memory map dari bootloader.
/// Sederhana: hanya bisa allocate, belum bisa free.
/// (Production: ganti dengan bitmap allocator)
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next:       usize, // Index frame berikutnya yang akan dialokasikan
}

impl BootInfoFrameAllocator {
    /// Inisialisasi dari memory map bootloader.
    ///
    /// # Safety
    /// Caller harus memastikan memory_map valid dan
    /// frame-frame yang ada belum dipakai.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Iterator semua frame yang tersedia (Usable regions)
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.memory_map
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| r.start..r.end)
            .flat_map(|r| r.step_by(4096)) // 4096 bytes per frame
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    /// Jumlah frame yang tersedia (untuk statistik)
    pub fn available_frames(&self) -> usize {
        self.usable_frames().count().saturating_sub(self.next)
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // Tests dijalankan di QEMU dengan memory map nyata
    // Lihat integration tests
}
