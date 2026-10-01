// kernel/src/fs/mbr.rs — Master Boot Record Parser
// SPDX-License-Identifier: MIT

use crate::drivers::ata::ATA_PRIMARY;

#[derive(Debug, Clone, Copy)]
pub struct Partition {
    pub status: u8,
    pub partition_type: u8,
    pub start_lba: u32,
    pub sector_count: u32,
}

pub fn read_partitions() -> Result<[Option<Partition>; 4], &'static str> {
    let mut buf = [0u8; 512];
    
    // Baca MBR di LBA 0
    {
        let mut bus = ATA_PRIMARY.lock();
        bus.read_sector(0, &mut buf)?;
    }

    // Validasi boot signature
    if buf[510] != 0x55 || buf[511] != 0xAA {
        return Err("Invalid MBR signature (not 0x55 0xAA)");
    }

    let mut partitions = [None; 4];
    
    // Partition table mulai di offset 0x1BE (446)
    for i in 0..4 {
        let offset = 446 + (i * 16);
        let pt_type = buf[offset + 4];
        
        // Lewati partisi kosong
        if pt_type == 0x00 {
            continue;
        }

        let status = buf[offset];
        
        let lba_start = (buf[offset + 8] as u32)
            | ((buf[offset + 9] as u32) << 8)
            | ((buf[offset + 10] as u32) << 16)
            | ((buf[offset + 11] as u32) << 24);
            
        let sec_count = (buf[offset + 12] as u32)
            | ((buf[offset + 13] as u32) << 8)
            | ((buf[offset + 14] as u32) << 16)
            | ((buf[offset + 15] as u32) << 24);

        partitions[i] = Some(Partition {
            status,
            partition_type: pt_type,
            start_lba: lba_start,
            sector_count: sec_count,
        });
    }

    Ok(partitions)
}
