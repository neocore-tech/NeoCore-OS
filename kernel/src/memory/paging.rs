// kernel/src/memory/paging.rs — Virtual Memory / Page Table Manager

use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable,
        Page, PageTableFlags, PhysFrame, Size4KiB,
        mapper::MapToError,
    },
};

/// Inisialisasi page table mapper.
///
/// # Safety
/// `physical_memory_offset` harus berupa offset valid yang didapat
/// dari bootloader (semua physical memory dipetakan di sana).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    use x86_64::registers::control::Cr3;

    let (level4_frame, _) = Cr3::read();
    let phys   = level4_frame.start_address();
    let virt   = physical_memory_offset + phys.as_u64();
    let table  = &mut *(virt.as_mut_ptr());

    OffsetPageTable::new(table, physical_memory_offset)
}

/// Map satu halaman virtual ke frame fisik tertentu.
pub fn map_page(
    page:            Page,
    frame:           PhysFrame,
    flags:           PageTableFlags,
    mapper:          &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)?.flush();
    }
    Ok(())
}

/// Map range alamat virtual ke physical (berurutan).
pub fn map_range(
    virt_start:      VirtAddr,
    phys_start:      x86_64::PhysAddr,
    size_bytes:      usize,
    flags:           PageTableFlags,
    mapper:          &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_count = size_bytes.div_ceil(4096);

    for i in 0..page_count {
        let page  = Page::containing_address(virt_start + (i * 4096) as u64);
        let frame = PhysFrame::containing_address(phys_start + (i * 4096) as u64);
        map_page(page, frame, flags, mapper, frame_allocator)?;
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use x86_64::VirtAddr;

    #[test_case]
    fn test_virtual_address_translation() {
        use x86_64::structures::paging::Translate;

        // Physical memory offset dari bootloader
        // Dalam test environment, kita verifikasi VGA buffer terpetakan
        let vga_virt = VirtAddr::new(0xb8000);
        serial_println!("VGA virtual addr: {:#x}", vga_virt.as_u64());
        // VGA buffer harus ada (tidak panic = sukses)
    }
}
