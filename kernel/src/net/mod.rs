// kernel/src/net/mod.rs — Network Subsystem (Placeholder)
// Akan diisi saat Phase 3

use crate::serial_println;

pub mod netlink;

/// Inisialisasi network stack
/// (diaktifkan setelah VirtIO-net driver selesai)
pub fn init() {
    crate::serial_println!("net0: initializing network subsystem...");
    crate::println!("net0: initializing network subsystem...");

    // Cari RTL8139 di daftar PCI (Vendor: 10EC, Device: 8139)
    let devices = crate::drivers::pci::PCI_DEVICES.lock();
    let mut rtl_dev = None;
    for dev in devices.iter() {
        if dev.vendor_id == 0x10EC && dev.device_id == 0x8139 {
            rtl_dev = Some(*dev);
            break;
        }
    }

    if let Some(dev) = rtl_dev {
        crate::serial_println!("net0: probe RTL8139 PCI device (10EC:8139) at Bus {} Slot {} Func {}", dev.bus, dev.slot, dev.func);
        crate::println!("net0: probe RTL8139 PCI device (10EC:8139) at Bus {} Slot {} Func {}", dev.bus, dev.slot, dev.func);
        
        // Dapatkan BAR0 (I/O Base Address)
        let bar0 = crate::drivers::pci::get_bar0(dev.bus, dev.slot, dev.func);
        // Bit terbawah BAR0 untuk I/O port selalu 1, mask bit itu untuk dapat port sebenarnya
        let io_base = (bar0 & !0x3) as u16;

        crate::serial_println!("net0: RTL8139 I/O Base Address: 0x{:X}", io_base);
        
        // Hidupkan Bus Mastering dengan mengaktifkan bit di PCI Command Register
        // (Offset 0x04 di ruang konfigurasi PCI)
        let cmd_reg = crate::drivers::pci::read_config_32(dev.bus, dev.slot, dev.func, 0x04);
        crate::drivers::pci::write_config_32(dev.bus, dev.slot, dev.func, 0x04, cmd_reg | 0x0004);

        // Turn on the RTL8139 (keluar dari sleep mode) dengan menulis 0x00 ke port CONFIG1 (offset 0x52)
        use x86_64::instructions::port::{PortWriteOnly, PortReadOnly};
        unsafe {
            let mut config1 = PortWriteOnly::<u8>::new(io_base + 0x52);
            config1.write(0x00);

            // Software Reset (tulis 0x10 ke port COMMAND offset 0x37)
            let mut cmd = PortWriteOnly::<u8>::new(io_base + 0x37);
            cmd.write(0x10);
            
            // Tunggu sampai reset selesai (bit 0x10 jadi 0)
            let mut cmd_read = PortReadOnly::<u8>::new(io_base + 0x37);
            while (cmd_read.read() & 0x10) != 0 {
                core::hint::spin_loop();
            }
            
            // Baca MAC Address dari offset 0x00 sampai 0x05
            let mut mac = [0u8; 6];
            for i in 0..6 {
                let mut mac_port = PortReadOnly::<u8>::new(io_base + i as u16);
                mac[i as usize] = mac_port.read();
            }

            crate::serial_println!("net0: MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} (REAL HARDWARE!)", 
                                   mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
            crate::println!("net0: MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} (REAL HARDWARE!)", 
                            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
        }

        crate::serial_println!("net0: smoltcp IPv4 stack attached");
        crate::println!("net0: smoltcp IPv4 stack attached");
    } else {
        crate::serial_println!("net0: RTL8139 not found on PCI bus!");
        crate::println!("net0: RTL8139 not found on PCI bus!");
    }
}
