// kernel/src/drivers/pci.rs — PCI Bus Enumerator
// Mendeteksi device yang terpasang di PCI bus menggunakan I/O Ports
// SPDX-License-Identifier: MIT

use crate::serial_println;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spinning_top::Spinlock;
use x86_64::instructions::port::Port;

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u8,
    pub subclass_id: u8,
    pub prog_if: u8,
    pub header_type: u8,
}

lazy_static! {
    /// Daftar semua perangkat PCI yang terdeteksi
    pub static ref PCI_DEVICES: Spinlock<Vec<PciDevice>> = Spinlock::new(Vec::new());
}

/// Membaca 32-bit (double word) dari PCI Configuration Space
pub fn read_config_32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let address = 1u32 << 31                     // Enable Bit
        | ((bus as u32) << 16)                   // Bus Number
        | ((slot as u32 & 0x1F) << 11)           // Device/Slot Number
        | ((func as u32 & 0x07) << 8)            // Function Number
        | (offset as u32 & 0xFC);                // Register Offset (must be 4-byte aligned)

    let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
    let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

    unsafe {
        addr_port.write(address);
        data_port.read()
    }
}

/// Menulis 32-bit (double word) ke PCI Configuration Space
pub fn write_config_32(bus: u8, slot: u8, func: u8, offset: u8, data: u32) {
    let address = 1u32 << 31
        | ((bus as u32) << 16)
        | ((slot as u32 & 0x1F) << 11)
        | ((func as u32 & 0x07) << 8)
        | (offset as u32 & 0xFC);

    let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
    let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

    unsafe {
        addr_port.write(address);
        data_port.write(data);
    }
}

pub fn get_vendor_id(bus: u8, slot: u8, func: u8) -> u16 {
    let r = read_config_32(bus, slot, func, 0);
    (r & 0xFFFF) as u16
}

pub fn get_device_id(bus: u8, slot: u8, func: u8) -> u16 {
    let r = read_config_32(bus, slot, func, 0);
    (r >> 16) as u16
}

pub fn get_class_id(bus: u8, slot: u8, func: u8) -> u8 {
    let r = read_config_32(bus, slot, func, 0x08);
    (r >> 24) as u8
}

pub fn get_subclass_id(bus: u8, slot: u8, func: u8) -> u8 {
    let r = read_config_32(bus, slot, func, 0x08);
    (r >> 16) as u8
}

pub fn get_prog_if(bus: u8, slot: u8, func: u8) -> u8 {
    let r = read_config_32(bus, slot, func, 0x08);
    (r >> 8) as u8
}

pub fn get_header_type(bus: u8, slot: u8, func: u8) -> u8 {
    let r = read_config_32(bus, slot, func, 0x0C);
    (r >> 16) as u8
}

pub fn get_bar0(bus: u8, slot: u8, func: u8) -> u32 {
    read_config_32(bus, slot, func, 0x10)
}

/// Cek apakah ada function di (bus, slot, func). Jika ada, catat.
fn check_function(bus: u8, slot: u8, func: u8) {
    let vendor_id = get_vendor_id(bus, slot, func);
    if vendor_id == 0xFFFF {
        return; // Device tidak ada
    }

    let device_id = get_device_id(bus, slot, func);
    let class_id = get_class_id(bus, slot, func);
    let subclass_id = get_subclass_id(bus, slot, func);
    let prog_if = get_prog_if(bus, slot, func);
    let header_type = get_header_type(bus, slot, func);

    let dev = PciDevice {
        bus,
        slot,
        func,
        vendor_id,
        device_id,
        class_id,
        subclass_id,
        prog_if,
        header_type,
    };

    PCI_DEVICES.lock().push(dev);

    serial_println!(
        "pci0: Ditemukan: Bus {:02X} Slot {:02X} Func {} — Vendor: {:04X}, Device: {:04X} (Class: {:02X}, Subclass: {:02X})",
        bus, slot, func, vendor_id, device_id, class_id, subclass_id
    );
}

/// Scan sebuah device pada bus & slot. Jika multi-function, scan semua function.
fn check_device(bus: u8, slot: u8) {
    let vendor_id = get_vendor_id(bus, slot, 0);
    if vendor_id == 0xFFFF {
        return; // Tidak ada device di slot ini
    }

    check_function(bus, slot, 0);

    let header_type = get_header_type(bus, slot, 0);
    if (header_type & 0x80) != 0 {
        // Multi-function device
        for func in 1..8 {
            let vid = get_vendor_id(bus, slot, func);
            if vid != 0xFFFF {
                check_function(bus, slot, func);
            }
        }
    }
}

/// Mulai scan semua bus (Brute force scan PCI)
pub fn init() {
    serial_println!("pci0: Memulai enumerasi bus PCI...");
    let mut count = 0;
    
    // Periksa host bridge di Bus 0, Slot 0
    let header_type = get_header_type(0, 0, 0);
    if (header_type & 0x80) == 0 {
        // Single PCI host controller
        for slot in 0..32 {
            check_device(0, slot);
        }
    } else {
        // Multiple PCI host controllers (hingga 8 bus utama)
        for func in 0..8 {
            if get_vendor_id(0, 0, func) != 0xFFFF {
                // Untuk kesederhanaan, kita fallback ke scan brute force bus 0-255
                break;
            }
        }
    }

    // Pendekatan sederhana: Brute-force 256 buses x 32 slots
    // (Bisa dioptimasi dengan read secondary bus number untuk bridge)
    PCI_DEVICES.lock().clear(); // Reset sebelum scan
    for bus in 0..=255 {
        for slot in 0..32 {
            check_device(bus as u8, slot);
        }
    }

    count = PCI_DEVICES.lock().len();
    serial_println!("pci0: Enumerasi selesai. Menemukan {} device.", count);
}
