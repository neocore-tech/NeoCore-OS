# 🔌 DRIVERS — Hardware Drivers NeoCore OS

---

## 1. Overview

Driver hardware adalah lapisan terbawah kernel yang berinteraksi langsung dengan perangkat keras. Semua driver ditulis dalam **Rust** menggunakan memory-mapped I/O (MMIO) dan port I/O.

---

## 2. Daftar Driver

| Driver | Device | Status |
|--------|--------|--------|
| VGA Text Mode | 80×25 char display | ✅ Implemented |
| Serial UART 16550 | COM1 (debugging) | ✅ Implemented |
| PS/2 Keyboard | Keyboard controller | ✅ Implemented |
| PS/2 Mouse | Mouse controller | 🔄 Planned |
| VirtIO-net | QEMU network (testing) | 🔄 In Progress |
| Intel e1000 | Real NIC | 🔄 Planned |
| ATA/IDE | Disk (PIO mode) | 🔄 Planned |
| AHCI/SATA | Disk (DMA mode) | 📅 Future |
| PCI Bus | PCI device enumeration | 🔄 In Progress |
| ACPI | Power management | 📅 Future |
| USB HID | USB keyboard/mouse | 📅 Future |

---

## 3. PCI Bus Enumeration

```rust
// src/drivers/pci.rs

// PCI Configuration Space Ports
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA:    u16 = 0xCFC;

#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus:       u8,
    pub device:    u8,
    pub function:  u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class:     u8,
    pub subclass:  u8,
    pub bar:       [u32; 6], // Base Address Registers
    pub irq:       u8,
}

pub fn config_read32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address: u32 =
        (1 << 31)           |  // Enable bit
        ((bus as u32) << 16) |
        ((device as u32) << 11) |
        ((func as u32) << 8) |
        ((offset as u32) & 0xFC);

    unsafe {
        use x86_64::instructions::port::Port;
        Port::<u32>::new(PCI_CONFIG_ADDRESS).write(address);
        Port::<u32>::new(PCI_CONFIG_DATA).read()
    }
}

pub fn config_write32(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    let address: u32 =
        (1 << 31) |
        ((bus as u32) << 16) |
        ((device as u32) << 11) |
        ((func as u32) << 8) |
        ((offset as u32) & 0xFC);

    unsafe {
        use x86_64::instructions::port::Port;
        Port::<u32>::new(PCI_CONFIG_ADDRESS).write(address);
        Port::<u32>::new(PCI_CONFIG_DATA).write(value);
    }
}

/// Scan semua PCI bus/device/function
pub fn enumerate() -> alloc::vec::Vec<PciDevice> {
    let mut devices = alloc::vec::Vec::new();

    for bus in 0u8..=255 {
        for device in 0u8..32 {
            for func in 0u8..8 {
                let vendor_device = config_read32(bus, device, func, 0x00);

                let vendor_id = (vendor_device & 0xFFFF) as u16;
                if vendor_id == 0xFFFF {
                    continue; // No device
                }

                let device_id = (vendor_device >> 16) as u16;
                let class_info = config_read32(bus, device, func, 0x08);
                let class    = (class_info >> 24) as u8;
                let subclass = (class_info >> 16) as u8;

                // Baca BAR registers
                let mut bar = [0u32; 6];
                for i in 0..6 {
                    bar[i] = config_read32(bus, device, func, 0x10 + i as u8 * 4);
                }

                let irq_line = config_read32(bus, device, func, 0x3C);
                let irq = (irq_line & 0xFF) as u8;

                devices.push(PciDevice {
                    bus, device, function: func,
                    vendor_id, device_id,
                    class, subclass, bar, irq,
                });

                // Jika single-function, skip function 1-7
                let header_type = (config_read32(bus, device, func, 0x0C) >> 16) as u8;
                if func == 0 && (header_type & 0x80) == 0 {
                    break;
                }
            }
        }
    }

    devices
}

/// Cari device berdasarkan vendor + device ID
pub fn find_device(vendor: u16, device_id: u16) -> Option<PciDevice> {
    enumerate().into_iter().find(|d| {
        d.vendor_id == vendor && d.device_id == device_id
    })
}

// Well-known PCI device IDs
pub mod ids {
    // Network
    pub const VIRTIO_VENDOR: u16 = 0x1AF4;
    pub const VIRTIO_NET:    u16 = 0x1000;
    pub const INTEL_E1000:   u16 = 0x100E; // QEMU e1000
    pub const INTEL_VENDOR:  u16 = 0x8086;
    
    // Display
    pub const VGA_VENDOR: u16 = 0x1234;
    pub const VMWARE_SVGA: u16 = 0x0405;
    
    // Storage
    pub const PIIX4_IDE:  u16 = 0x7010;
    pub const AHCI_CLASS: u8  = 0x01;
    pub const AHCI_SUB:   u8  = 0x06;
}
```

---

## 4. Serial UART Driver (COM1)

```rust
// src/drivers/serial.rs
use lazy_static::lazy_static;
use spinning_top::Spinlock;
use uart_16550::SerialPort;

lazy_static! {
    pub static ref SERIAL1: Spinlock<SerialPort> = {
        let mut serial = unsafe { SerialPort::new(0x3F8) }; // COM1
        serial.init();
        Spinlock::new(serial)
    };
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args)
            .expect("Serial write failed");
    });
}
```

---

## 5. PS/2 Keyboard Driver

```rust
// src/drivers/keyboard.rs
use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use pc_keyboard::{
    layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1,
};
use spinning_top::Spinlock;
use lazy_static::lazy_static;

// Queue untuk scancode (ISR → task)
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

lazy_static! {
    static ref KEYBOARD: Spinlock<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Spinlock::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ));
}

/// Dipanggil dari interrupt handler ISR
pub fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if queue.push(scancode).is_err() {
            // Queue penuh, drop scancode
        }
    }
}

/// Inisialisasi keyboard
pub fn init() {
    SCANCODE_QUEUE.init_once(|| ArrayQueue::new(100));
}

/// Baca karakter berikutnya (blocking)
pub fn read_char() -> Option<char> {
    let scancode = SCANCODE_QUEUE.try_get().ok()?.pop()?;

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(c)   => return Some(c),
                DecodedKey::RawKey(code) => {
                    // Handle special keys
                    match code {
                        KeyCode::ArrowUp    => return Some('\x1B'), // TODO: full escape seq
                        KeyCode::ArrowDown  => return Some('\x1B'),
                        KeyCode::Delete     => return Some('\x7F'),
                        KeyCode::Backspace  => return Some('\x08'),
                        KeyCode::Enter      => return Some('\n'),
                        _ => {}
                    }
                }
            }
        }
    }
    None
}

/// Task handler untuk keyboard (berjalan sebagai kernel task)
pub fn handle_input() {
    init();
    loop {
        // Keyboard input di-handle oleh shell melalui read_char()
        x86_64::instructions::hlt();
    }
}
```

---

## 6. ATA/IDE Disk Driver (PIO Mode)

```rust
// src/drivers/disk/ata.rs
use x86_64::instructions::port::Port;

// ATA Primary bus ports
const ATA_PRIMARY_DATA:         u16 = 0x1F0;
const ATA_PRIMARY_ERROR:        u16 = 0x1F1;
const ATA_PRIMARY_SECTOR_COUNT: u16 = 0x1F2;
const ATA_PRIMARY_LBA_LOW:      u16 = 0x1F3;
const ATA_PRIMARY_LBA_MID:      u16 = 0x1F4;
const ATA_PRIMARY_LBA_HIGH:     u16 = 0x1F5;
const ATA_PRIMARY_DRIVE_HEAD:   u16 = 0x1F6;
const ATA_PRIMARY_STATUS:       u16 = 0x1F7;
const ATA_PRIMARY_COMMAND:      u16 = 0x1F7;

// Status bits
const ATA_STATUS_BSY: u8 = 0x80; // Busy
const ATA_STATUS_DRQ: u8 = 0x08; // Data request
const ATA_STATUS_ERR: u8 = 0x01; // Error

// Commands
const ATA_CMD_READ_PIO:  u8 = 0x20;
const ATA_CMD_WRITE_PIO: u8 = 0x30;
const ATA_CMD_IDENTIFY:  u8 = 0xEC;

pub struct AtaDrive {
    primary: bool, // true = primary, false = secondary
    master:  bool, // true = master, false = slave
}

impl AtaDrive {
    pub fn new(primary: bool, master: bool) -> Option<Self> {
        let drive = AtaDrive { primary, master };
        if drive.identify() {
            Some(drive)
        } else {
            None
        }
    }

    fn data_port(&self)   -> Port<u16> { Port::new(ATA_PRIMARY_DATA) }
    fn status_port(&self) -> Port<u8>  { Port::new(ATA_PRIMARY_STATUS) }
    fn cmd_port(&self)    -> Port<u8>  { Port::new(ATA_PRIMARY_COMMAND) }

    fn wait_ready(&self) {
        unsafe {
            let mut status_port = self.status_port();
            while status_port.read() & ATA_STATUS_BSY != 0 {}
        }
    }

    fn wait_drq(&self) -> bool {
        unsafe {
            let mut status_port = self.status_port();
            loop {
                let status = status_port.read();
                if status & ATA_STATUS_ERR != 0 { return false; }
                if status & ATA_STATUS_DRQ != 0 { return true; }
            }
        }
    }

    fn identify(&self) -> bool {
        unsafe {
            self.wait_ready();
            
            // Select drive
            Port::<u8>::new(ATA_PRIMARY_DRIVE_HEAD).write(
                if self.master { 0xA0 } else { 0xB0 }
            );
            
            // Clear ports
            Port::<u8>::new(ATA_PRIMARY_SECTOR_COUNT).write(0);
            Port::<u8>::new(ATA_PRIMARY_LBA_LOW).write(0);
            Port::<u8>::new(ATA_PRIMARY_LBA_MID).write(0);
            Port::<u8>::new(ATA_PRIMARY_LBA_HIGH).write(0);
            
            // Send IDENTIFY
            self.cmd_port().write(ATA_CMD_IDENTIFY);
            
            let status = self.status_port().read();
            if status == 0 { return false; } // Drive tidak ada
            
            self.wait_drq()
        }
    }

    /// Baca sektor dari disk (LBA28)
    pub fn read_sectors(&self, lba: u32, count: u8, buffer: &mut [u16]) {
        assert!(buffer.len() >= count as usize * 256);

        unsafe {
            self.wait_ready();

            // Setup
            Port::<u8>::new(ATA_PRIMARY_DRIVE_HEAD).write(
                (if self.master { 0xE0 } else { 0xF0 }) | ((lba >> 24) as u8 & 0x0F)
            );
            Port::<u8>::new(ATA_PRIMARY_SECTOR_COUNT).write(count);
            Port::<u8>::new(ATA_PRIMARY_LBA_LOW).write((lba & 0xFF) as u8);
            Port::<u8>::new(ATA_PRIMARY_LBA_MID).write(((lba >> 8) & 0xFF) as u8);
            Port::<u8>::new(ATA_PRIMARY_LBA_HIGH).write(((lba >> 16) & 0xFF) as u8);

            self.cmd_port().write(ATA_CMD_READ_PIO);

            for i in 0..count as usize {
                self.wait_drq();
                let mut data_port = self.data_port();
                for j in 0..256 {
                    buffer[i * 256 + j] = data_port.read();
                }
            }
        }
    }

    /// Tulis sektor ke disk
    pub fn write_sectors(&self, lba: u32, count: u8, buffer: &[u16]) {
        assert!(buffer.len() >= count as usize * 256);

        unsafe {
            self.wait_ready();

            Port::<u8>::new(ATA_PRIMARY_DRIVE_HEAD).write(
                (if self.master { 0xE0 } else { 0xF0 }) | ((lba >> 24) as u8 & 0x0F)
            );
            Port::<u8>::new(ATA_PRIMARY_SECTOR_COUNT).write(count);
            Port::<u8>::new(ATA_PRIMARY_LBA_LOW).write((lba & 0xFF) as u8);
            Port::<u8>::new(ATA_PRIMARY_LBA_MID).write(((lba >> 8) & 0xFF) as u8);
            Port::<u8>::new(ATA_PRIMARY_LBA_HIGH).write(((lba >> 16) & 0xFF) as u8);

            self.cmd_port().write(ATA_CMD_WRITE_PIO);

            for i in 0..count as usize {
                self.wait_drq();
                let mut data_port = self.data_port();
                for j in 0..256 {
                    data_port.write(buffer[i * 256 + j]);
                }
                // Cache flush
                self.cmd_port().write(0xE7); // FLUSH_CACHE
                self.wait_ready();
            }
        }
    }
}
```

---

## 7. HPET Timer Driver

```rust
// src/drivers/timer/hpet.rs
// High Precision Event Timer

const HPET_BASE_ADDR: u64 = 0xFED00000; // Standar ACPI

// Register offsets
const HPET_CAPS:      u64 = 0x00; // Capabilities
const HPET_CONFIG:    u64 = 0x10; // Configuration
const HPET_STATUS:    u64 = 0x20; // Interrupt status
const HPET_COUNTER:   u64 = 0xF0; // Main counter

pub struct Hpet {
    base: *mut u64,
    period_femto: u64, // Timer period in femtoseconds
}

impl Hpet {
    pub unsafe fn new() -> Self {
        let base = HPET_BASE_ADDR as *mut u64;
        
        // Baca capabilities (period ada di bits 63:32)
        let caps = base.read_volatile();
        let period_femto = caps >> 32;
        
        // Enable HPET
        let cfg = base.add(HPET_CONFIG as usize / 8);
        cfg.write_volatile(cfg.read_volatile() | 1); // ENABLE_CNF = 1
        
        Hpet { base, period_femto }
    }

    pub fn read_counter(&self) -> u64 {
        unsafe {
            self.base.add(HPET_COUNTER as usize / 8).read_volatile()
        }
    }

    /// Konversi counter ke milliseconds
    pub fn counter_to_ms(&self, counter: u64) -> u64 {
        // period_femto * counter / 1_000_000_000_000 = ms
        counter * self.period_femto / 1_000_000_000_000
    }

    pub fn sleep_ms(&self, ms: u64) {
        let target = self.read_counter() + 
                     (ms * 1_000_000_000_000 / self.period_femto);
        while self.read_counter() < target {
            core::hint::spin_loop();
        }
    }
}
```

---

## 8. Driver Registration System

```rust
// src/drivers/mod.rs
use alloc::{boxed::Box, vec::Vec};

pub mod vga;
pub mod serial;
pub mod keyboard;
pub mod pci;
pub mod net;
pub mod disk;
pub mod timer;

/// Trait untuk semua driver
pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    fn init(&mut self) -> Result<(), &'static str>;
    fn probe(&self) -> bool; // Cek apakah hardware ada
}

/// Global driver registry
pub struct DriverRegistry {
    drivers: Vec<Box<dyn Driver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        DriverRegistry { drivers: Vec::new() }
    }

    pub fn register(&mut self, driver: Box<dyn Driver>) {
        self.drivers.push(driver);
    }

    pub fn init_all(&mut self) {
        for driver in &mut self.drivers {
            if driver.probe() {
                match driver.init() {
                    Ok(_)  => println!("[DRV] {} initialized", driver.name()),
                    Err(e) => println!("[DRV] {} failed: {}", driver.name(), e),
                }
            }
        }
    }
}
```

---

## 9. Referensi

- [OSDev — ATA PIO](https://wiki.osdev.org/ATA_PIO_Mode)
- [OSDev — PCI](https://wiki.osdev.org/PCI)
- [OSDev — PS/2 Keyboard](https://wiki.osdev.org/PS/2_Keyboard)
- [OSDev — HPET](https://wiki.osdev.org/HPET)
- [VirtIO Spec](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)
- [pc-keyboard crate](https://docs.rs/pc-keyboard)
