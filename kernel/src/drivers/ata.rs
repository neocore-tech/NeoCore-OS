// kernel/src/drivers/ata.rs — ATA/IDE PIO Mode Driver
// Driver sederhana untuk read/write disk block (LBA28)
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;
use lazy_static::lazy_static;
use spinning_top::Spinlock;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use core::hint::spin_loop;
use crate::{print, println, serial_println};

// ── ATA Register Ports (Primary Bus) ──────────────────────────────────────

const ATA_PRIMARY_DATA:         u16 = 0x1F0;
const ATA_PRIMARY_ERR_FEAT:     u16 = 0x1F1;
const ATA_PRIMARY_SECTOR_COUNT: u16 = 0x1F2;
const ATA_PRIMARY_LBA_LO:       u16 = 0x1F3;
const ATA_PRIMARY_LBA_MID:      u16 = 0x1F4;
const ATA_PRIMARY_LBA_HI:       u16 = 0x1F5;
const ATA_PRIMARY_DRV_HEAD:     u16 = 0x1F6;
const ATA_PRIMARY_CMD_STATUS:   u16 = 0x1F7;
const ATA_PRIMARY_ALT_STATUS:   u16 = 0x3F6;

// ── Status Register Bits ──────────────────────────────────────────────────

const STATUS_ERR: u8 = 0x01; // Error
const STATUS_DRQ: u8 = 0x08; // Data Request Ready
const STATUS_SRV: u8 = 0x10; // Overlapped Mode Service Request
const STATUS_DF:  u8 = 0x20; // Drive Fault
const STATUS_RDY: u8 = 0x40; // Device Ready
const STATUS_BSY: u8 = 0x80; // Busy

// ── Commands ──────────────────────────────────────────────────────────────

const CMD_READ_PIO:     u8 = 0x20;
const CMD_WRITE_PIO:    u8 = 0x30;
const CMD_IDENTIFY:     u8 = 0xEC;

// ── ATA Bus Controller ────────────────────────────────────────────────────

pub struct AtaBus {
    data:         Port<u16>,
    error:        PortReadOnly<u8>,
    features:     PortWriteOnly<u8>,
    sector_count: Port<u8>,
    lba_lo:       Port<u8>,
    lba_mid:      Port<u8>,
    lba_hi:       Port<u8>,
    drive_head:   Port<u8>,
    status:       PortReadOnly<u8>,
    command:      PortWriteOnly<u8>,
    alt_status:   PortReadOnly<u8>,
    is_master:    bool,
}

lazy_static! {
    /// Global instance untuk ATA Primary Bus
    pub static ref ATA_PRIMARY: Spinlock<AtaBus> = Spinlock::new(AtaBus::new_primary());
}

impl AtaBus {
    pub fn new_primary() -> Self {
        Self {
            data:         Port::new(ATA_PRIMARY_DATA),
            error:        PortReadOnly::new(ATA_PRIMARY_ERR_FEAT),
            features:     PortWriteOnly::new(ATA_PRIMARY_ERR_FEAT),
            sector_count: Port::new(ATA_PRIMARY_SECTOR_COUNT),
            lba_lo:       Port::new(ATA_PRIMARY_LBA_LO),
            lba_mid:      Port::new(ATA_PRIMARY_LBA_MID),
            lba_hi:       Port::new(ATA_PRIMARY_LBA_HI),
            drive_head:   Port::new(ATA_PRIMARY_DRV_HEAD),
            status:       PortReadOnly::new(ATA_PRIMARY_CMD_STATUS),
            command:      PortWriteOnly::new(ATA_PRIMARY_CMD_STATUS),
            alt_status:   PortReadOnly::new(ATA_PRIMARY_ALT_STATUS),
            is_master:    true,
        }
    }

    /// Tunggu sampai BSY (Busy) bit bersih
    fn wait_bsy(&mut self) -> Result<(), &'static str> {
        for _ in 0..100000 {
            let status = unsafe { self.status.read() };
            if (status & STATUS_BSY) == 0 {
                return Ok(());
            }
            spin_loop();
        }
        Err("ATA: Drive busy timeout")
    }

    /// Tunggu sampai DRQ (Data Request) bit set, dan BSY bersih
    fn wait_drq(&mut self) -> Result<(), &'static str> {
        for _ in 0..100000 {
            let status = unsafe { self.status.read() };
            if (status & STATUS_ERR) != 0 {
                return Err("ATA: Error bit set");
            }
            if (status & STATUS_DF) != 0 {
                return Err("ATA: Drive fault");
            }
            if (status & STATUS_BSY) == 0 && (status & STATUS_DRQ) != 0 {
                return Ok(());
            }
            spin_loop();
        }
        Err("ATA: Data request timeout")
    }

    /// Software reset untuk bus ATA
    pub fn reset(&mut self) {
        unsafe {
            let mut ctl: PortWriteOnly<u8> = PortWriteOnly::new(ATA_PRIMARY_ALT_STATUS);
            ctl.write(0x04); // Set SRST bit
            for _ in 0..1000 { spin_loop(); }
            ctl.write(0x00); // Clear SRST bit
        }
        let _ = self.wait_bsy();
    }

    /// Mengirim command Identify (mendapatkan info disk)
    pub fn identify(&mut self, master: bool) -> Result<(), &'static str> {
        self.wait_bsy()?;

        unsafe {
            let select = if master { 0xA0 } else { 0xB0 };
            self.drive_head.write(select);
            self.sector_count.write(0);
            self.lba_lo.write(0);
            self.lba_mid.write(0);
            self.lba_hi.write(0);
            self.command.write(CMD_IDENTIFY);
        }

        // Cek apakah drive ada (jika status 0 berarti tidak ada drive)
        if unsafe { self.status.read() } == 0 {
            return Err("ATA: Drive does not exist");
        }

        self.wait_bsy()?;

        // Validasi signature (jika ini ATAPI/SATA, bukan ATA, LBA mid/hi akan ter-set)
        let lba1 = unsafe { self.lba_mid.read() };
        let lba2 = unsafe { self.lba_hi.read() };
        if lba1 != 0 || lba2 != 0 {
            return Err("ATA: Not an ATA drive (could be ATAPI)");
        }

        self.wait_drq()?;

        // Baca 256 words (512 bytes) dari data port
        let mut data = [0u16; 256];
        for i in 0..256 {
            data[i] = unsafe { self.data.read() };
        }

        // Parse jumlah sector (LBA28 cap ada di word 60 dan 61)
        let sectors_28 = (data[60] as u32) | ((data[61] as u32) << 16);
        let size_mb = (sectors_28 as u64 * 512) / (1024 * 1024);

        self.is_master = master;
        let name = if master { "Master" } else { "Slave" };
        serial_println!("ata0: Primary {} identified. Size: {} MB ({} sectors)", name, size_mb, sectors_28);
        Ok(())
    }

    /// Membaca satu sektor (512 bytes) menggunakan LBA28
    pub fn read_sector(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), &'static str> {
        if lba >= 0x0FFFFFFF {
            return Err("ATA: LBA out of bounds (LBA28 max)");
        }

        self.wait_bsy()?;

        unsafe {
            let select = if self.is_master { 0xE0 } else { 0xF0 };
            self.drive_head.write(select | ((lba >> 24) & 0x0F) as u8);
            self.features.write(0x00);
            self.sector_count.write(1);
            self.lba_lo.write(lba as u8);
            self.lba_mid.write((lba >> 8) as u8);
            self.lba_hi.write((lba >> 16) as u8);
            self.command.write(CMD_READ_PIO);
        }

        self.wait_drq()?;

        unsafe {
            // Baca 256 words
            for i in 0..256 {
                let word = self.data.read();
                buf[i * 2] = (word & 0xFF) as u8;
                buf[i * 2 + 1] = (word >> 8) as u8;
            }
        }

        // Delay sedikit agar bus stabil
        unsafe { let _ = self.alt_status.read(); }
        
        Ok(())
    }

    /// Menulis satu sektor (512 bytes) menggunakan LBA28
    pub fn write_sector(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), &'static str> {
        if lba >= 0x0FFFFFFF {
            return Err("ATA: LBA out of bounds (LBA28 max)");
        }

        self.wait_bsy()?;

        unsafe {
            let select = if self.is_master { 0xE0 } else { 0xF0 };
            self.drive_head.write(select | ((lba >> 24) & 0x0F) as u8);
            self.features.write(0x00);
            self.sector_count.write(1);
            self.lba_lo.write(lba as u8);
            self.lba_mid.write((lba >> 8) as u8);
            self.lba_hi.write((lba >> 16) as u8);
            self.command.write(CMD_WRITE_PIO);
        }

        self.wait_drq()?;

        unsafe {
            // Tulis 256 words
            for i in 0..256 {
                let word = (buf[i * 2] as u16) | ((buf[i * 2 + 1] as u16) << 8);
                self.data.write(word);
            }
        }

        // Flush (Opsional untuk IDE lama, tapi disarankan)
        unsafe { self.command.write(0xE7); } // Cache Flush
        self.wait_bsy()?;

        Ok(())
    }
}

pub fn init() {
    serial_println!("ata0: Initializing ATA PIO driver...");
    let mut bus = ATA_PRIMARY.lock();
    
    // Disable interrupts on primary ATA bus (kita pakai polling/PIO mode)
    unsafe {
        let mut ctl: PortWriteOnly<u8> = PortWriteOnly::new(ATA_PRIMARY_ALT_STATUS);
        ctl.write(0x02); // Set nIEN (no Interrupt Enable)
    }

    // Coba identifikasi Slave dulu (karena boot image ada di Master)
    if bus.identify(false).is_ok() {
        serial_println!("ata0: Primary Slave drive ready (Using as default).");
    } else if bus.identify(true).is_ok() {
        serial_println!("ata0: Primary Master drive ready (Using as default).");
    } else {
        serial_println!("ata0: Initialization failed: No drive found.");
    }
}
