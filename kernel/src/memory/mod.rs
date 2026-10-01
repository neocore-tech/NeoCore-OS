// kernel/src/memory/mod.rs — Memory Management Module

use crate::{println, serial_println};
pub mod frame_allocator;
pub mod heap;
pub mod paging;

use bootloader_api::BootInfo;

/// Tampilkan info memori saat boot
pub fn print_info(boot_info: &BootInfo) {
    use bootloader_api::info::MemoryRegionKind;

    let mut total_bytes: u64 = 0;
    let mut usable_bytes: u64 = 0;

    for region in boot_info.memory_regions.iter() {
        total_bytes += region.end - region.start;
        if region.kind == MemoryRegionKind::Usable {
            usable_bytes += region.end - region.start;
        }
    }

    let total_mb  = total_bytes  / (1024 * 1024);
    let usable_mb = usable_bytes / (1024 * 1024);

    println!("mem0: {} MB total, {} MB usable", total_mb, usable_mb);
    serial_println!("mem0: total={}MB usable={}MB", total_mb, usable_mb);
}
