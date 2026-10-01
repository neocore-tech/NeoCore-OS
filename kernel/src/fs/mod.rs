// kernel/src/fs/mod.rs — Filesystem Subsystem
// SPDX-License-Identifier: MIT

pub mod mbr;
pub mod fat32;

use crate::serial_println;
use alloc::sync::Arc;
use spinning_top::Spinlock;
use lazy_static::lazy_static;

lazy_static! {
    /// Global instance untuk ROOT FAT32 filesystem
    pub static ref ROOT_FS: Spinlock<Option<Arc<fat32::Fat32Volume>>> = Spinlock::new(None);
}

/// Inisialisasi Sistem File
pub fn init() {
    serial_println!("vfs0: Initializing filesystem...");

    // 1. Baca partisi dari MBR
    let partitions = match mbr::read_partitions() {
        Ok(p) => p,
        Err(e) => {
            serial_println!("vfs0: Failed to read MBR: {}", e);
            return;
        }
    };

    // 2. Cari partisi FAT32 (Tipe 0x0B atau 0x0C)
    let mut fat32_start_lba = None;
    for (i, p_opt) in partitions.iter().enumerate() {
        if let Some(p) = p_opt {
            serial_println!("vfs0: Partition {}: Type=0x{:02X}, Start={}, Count={}", 
                i, p.partition_type, p.start_lba, p.sector_count);
            
            if p.partition_type == 0x0B || p.partition_type == 0x0C {
                fat32_start_lba = Some(p.start_lba);
                break;
            }
        }
    }

    if let Some(lba) = fat32_start_lba {
        serial_println!("vfs0: Found FAT32 partition at LBA {}", lba);
        match fat32::Fat32Volume::new(lba) {
            Ok(vol) => {
                serial_println!("vfs0: FAT32 Volume mounted successfully!");
                serial_println!("vfs0: Root cluster: {}", vol.bpb.root_cluster);
                *ROOT_FS.lock() = Some(Arc::new(vol));
            }
            Err(e) => serial_println!("vfs0: Failed to mount FAT32: {}", e),
        }
    } else {
        serial_println!("vfs0: No FAT32 partition found.");
    }
}
