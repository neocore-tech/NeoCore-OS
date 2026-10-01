// kernel/src/fs/fat32.rs — FAT32 Read-Only Driver
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;
use alloc::string::String;
use core::str;
use crate::{serial_println, println};
use crate::drivers::ata::ATA_PRIMARY;

#[derive(Debug, Clone, Copy)]
pub struct Bpb {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_cluster: u32,
    pub sectors_per_fat: u32,
    // Calculated values
    pub fat_start_lba: u32,
    pub data_start_lba: u32,
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u32,
    pub first_cluster: u32,
}

pub struct Fat32Volume {
    partition_start: u32,
    pub bpb: Bpb,
}

impl Fat32Volume {
    /// Inisialisasi FAT32 dari sektor LBA pertama partisi
    pub fn new(partition_start: u32) -> Result<Self, &'static str> {
        let mut buf = [0u8; 512];
        {
            let mut bus = ATA_PRIMARY.lock();
            bus.read_sector(partition_start, &mut buf)?;
        }

        // Cek FAT32 signature di offset 82 (tidak selalu valid, tapi biasanya ada tulisan "FAT32   ")
        // Cara paling aman cek tipe FAT adalah dengan menghitung jumlah cluster, 
        // tapi untuk OS sederhana, kita asumsikan partisinya benar FAT32.

        let bytes_per_sector = (buf[11] as u16) | ((buf[12] as u16) << 8);
        if bytes_per_sector != 512 {
            return Err("FAT32: Unsupported sector size (hanya dukung 512 bytes)");
        }

        let sectors_per_cluster = buf[13];
        let reserved_sectors = (buf[14] as u16) | ((buf[15] as u16) << 8);
        let fat_count = buf[16];
        
        let sectors_per_fat = (buf[36] as u32)
            | ((buf[37] as u32) << 8)
            | ((buf[38] as u32) << 16)
            | ((buf[39] as u32) << 24);

        let root_cluster = (buf[44] as u32)
            | ((buf[45] as u32) << 8)
            | ((buf[46] as u32) << 16)
            | ((buf[47] as u32) << 24);

        let fat_start_lba = partition_start + reserved_sectors as u32;
        let data_start_lba = fat_start_lba + (fat_count as u32 * sectors_per_fat);

        let bpb = Bpb {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fat_count,
            root_cluster,
            sectors_per_fat,
            fat_start_lba,
            data_start_lba,
        };

        Ok(Self { partition_start, bpb })
    }

    /// Konversi nomor cluster ke absolute LBA
    fn cluster_to_lba(&self, cluster: u32) -> u32 {
        let cluster_index = cluster.saturating_sub(2);
        self.bpb.data_start_lba + (cluster_index * self.bpb.sectors_per_cluster as u32)
    }

    /// Baca 1 cluster penuh (mungkin lebih dari 1 sektor)
    fn read_cluster(&self, cluster: u32) -> Result<Vec<u8>, &'static str> {
        let lba = self.cluster_to_lba(cluster);
        let mut data = Vec::with_capacity((self.bpb.sectors_per_cluster as usize) * 512);
        let mut buf = [0u8; 512];

        let mut bus = ATA_PRIMARY.lock();
        for i in 0..self.bpb.sectors_per_cluster as u32 {
            bus.read_sector(lba + i, &mut buf)?;
            data.extend_from_slice(&buf);
        }
        Ok(data)
    }

    /// Baca entri FAT tabel untuk mencari next cluster
    fn get_next_cluster(&self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4;
        let fat_sector = self.bpb.fat_start_lba + (fat_offset / 512);
        let ent_offset = (fat_offset % 512) as usize;

        let mut buf = [0u8; 512];
        {
            let mut bus = ATA_PRIMARY.lock();
            bus.read_sector(fat_sector, &mut buf)?;
        }

        let next_cluster = (buf[ent_offset] as u32)
            | ((buf[ent_offset + 1] as u32) << 8)
            | ((buf[ent_offset + 2] as u32) << 16)
            | ((buf[ent_offset + 3] as u32) << 24);

        // Hanya gunakan 28 bit bawah
        Ok(next_cluster & 0x0FFFFFFF)
    }

    /// Parse isi direktori dari sebuah cluster (Basic 8.3 support, abaikan LFN sementara)
    pub fn read_dir(&self, start_cluster: u32) -> Result<Vec<DirEntry>, &'static str> {
        let mut entries = Vec::new();
        let mut current_cluster = start_cluster;

        loop {
            let data = self.read_cluster(current_cluster)?;
            
            // Parse setiap 32-byte entry
            for chunk in data.chunks(32) {
                if chunk[0] == 0x00 {
                    // Akhir dari list direktori
                    return Ok(entries);
                }
                if chunk[0] == 0xE5 {
                    // Entry dihapus
                    continue;
                }
                let attr = chunk[11];
                if attr == 0x0F {
                    // Long File Name (LFN) entry — di-skip dulu
                    continue;
                }

                // Parse 8.3 nama
                let mut name_buf = [0u8; 12];
                let mut j = 0;
                // Base 8 chars
                for i in 0..8 {
                    if chunk[i] != b' ' {
                        name_buf[j] = chunk[i];
                        j += 1;
                    }
                }
                // Extension 3 chars
                if chunk[8] != b' ' {
                    name_buf[j] = b'.';
                    j += 1;
                    for i in 8..11 {
                        if chunk[i] != b' ' {
                            name_buf[j] = chunk[i];
                            j += 1;
                        }
                    }
                }
                let name = str::from_utf8(&name_buf[..j]).unwrap_or("UNKNOWN").into();

                let is_dir = (attr & 0x10) != 0;
                
                let cluster_hi = (chunk[20] as u32) | ((chunk[21] as u32) << 8);
                let cluster_lo = (chunk[26] as u32) | ((chunk[27] as u32) << 8);
                let first_cluster = (cluster_hi << 16) | cluster_lo;

                let size = (chunk[28] as u32)
                    | ((chunk[29] as u32) << 8)
                    | ((chunk[30] as u32) << 16)
                    | ((chunk[31] as u32) << 24);

                entries.push(DirEntry {
                    name,
                    is_dir,
                    size,
                    first_cluster,
                });
            }

            // Ke next cluster
            current_cluster = self.get_next_cluster(current_cluster)?;
            if current_cluster >= 0x0FFFFFF8 {
                // End Of Chain (EOC)
                break;
            }
        }

        Ok(entries)
    }

    /// Baca seluruh isi file berdasarkan first_cluster
    pub fn read_file(&self, first_cluster: u32, size: u32) -> Result<Vec<u8>, &'static str> {
        let mut data = Vec::with_capacity(size as usize);
        let mut current_cluster = first_cluster;

        loop {
            let chunk = self.read_cluster(current_cluster)?;
            data.extend_from_slice(&chunk);

            if data.len() as u32 >= size {
                break;
            }

            current_cluster = self.get_next_cluster(current_cluster)?;
            if current_cluster >= 0x0FFFFFF8 {
                break;
            }
        }

        data.truncate(size as usize);
        Ok(data)
    }
}
