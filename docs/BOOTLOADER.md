# 🥾 BOOTLOADER — Implementasi Bootloader NeoCore OS

---

## 1. Overview

Bootloader adalah kode pertama yang dieksekusi oleh CPU saat komputer dinyalakan. Tanggung jawabnya:

1. Mengambil alih dari BIOS/UEFI
2. Melakukan transisi mode: **Real Mode** → **Protected Mode** → **Long Mode (64-bit)**
3. Setup paging awal
4. Load kernel dari disk ke memori
5. Lompat ke entry point kernel

---

## 2. Boot Process Flow

```
Power ON
    │
    ▼
BIOS / UEFI Firmware
    │ POST (Power-On Self-Test)
    │ Mencari bootable device
    ▼
MBR (Master Boot Record) [Sektor 0, 512 bytes]
    │ Stage 1 Bootloader
    │ Kode minimal untuk load Stage 2
    ▼
Stage 2 Bootloader (Lebih besar, loaded dari disk)
    │ Switch ke Protected Mode (32-bit)
    │ Enable A20 line
    │ Setup GDT sementara
    ▼
Stage 3 - Long Mode Setup
    │ Enable PAE (Physical Address Extension)
    │ Setup PML4 page table
    │ Set bit LME di EFER MSR
    │ Enable paging
    ▼
Long Mode (64-bit) 🎉
    │ Load kernel ELF dari disk
    │ Parse ELF headers
    │ Map segmen ke virtual address
    ▼
Kernel Entry Point (_start)
    │ Kernel mulai berjalan!
```

---

## 3. Opsi Implementasi

### 3.1 Menggunakan Crate `bootloader` (Direkomendasikan untuk awal)

```toml
# bootloader/Cargo.toml
[package]
name = "bootloader-stage"
version = "0.1.0"
edition = "2021"

[dependencies]
bootloader = { version = "0.11", features = ["map_physical_memory"] }
```

```rust
// kernel/src/main.rs
#![no_std]
#![no_main]

use bootloader_api::{entry_point, BootInfo};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Kernel mulai di sini
    let framebuffer = boot_info.framebuffer.as_mut().unwrap();
    // ... inisialisasi kernel
    loop {}
}
```

### 3.2 Custom Bootloader (Advanced)

Untuk kontrol penuh, kita bisa buat bootloader sendiri dari scratch menggunakan assembly + Rust.

---

## 4. Struktur File Bootloader

```
bootloader/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point bootloader
│   ├── stage1/
│   │   └── boot.asm         # Stage 1 (real mode, 512 bytes)
│   ├── stage2/
│   │   ├── protected.rs     # Transisi ke protected mode
│   │   └── a20.rs           # Enable A20 line
│   ├── longmode/
│   │   ├── paging.rs        # Setup page tables
│   │   └── longmode.rs      # Switch ke 64-bit
│   └── elf/
│       └── loader.rs        # ELF kernel loader
└── linker.ld                # Linker script
```

---

## 5. Implementasi Detail

### 5.1 Stage 1 — Real Mode (Assembly)

```asm
; boot.asm - 512 bytes, berjalan di real mode (16-bit)
BITS 16
ORG 0x7C00          ; BIOS memuat bootloader di sini

start:
    ; Setup segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Tampilkan pesan "Loading..."
    mov si, msg_loading
    call print_string

    ; Baca stage 2 dari disk
    mov ah, 0x02    ; BIOS read sectors
    mov al, 32      ; Jumlah sektor yang dibaca
    mov ch, 0       ; Cylinder 0
    mov cl, 2       ; Sektor 2 (setelah MBR)
    mov dh, 0       ; Head 0
    mov dl, 0x80    ; Drive C:
    mov bx, 0x8000  ; Buffer address
    int 0x13        ; BIOS interrupt
    jc disk_error

    ; Lompat ke Stage 2
    jmp 0x0000:0x8000

disk_error:
    mov si, msg_error
    call print_string
    hlt

print_string:
    mov ah, 0x0E    ; BIOS teletype mode
.loop:
    lodsb
    test al, al
    jz .done
    int 0x10
    jmp .loop
.done:
    ret

msg_loading db "NeoCore OS Loading...", 13, 10, 0
msg_error   db "Disk Error!", 13, 10, 0

; Padding ke 512 bytes
times 510-($-$$) db 0
dw 0xAA55           ; Boot signature
```

### 5.2 Enable A20 Line

```rust
// src/stage2/a20.rs
// A20 line harus diaktifkan untuk akses memori > 1MB

pub fn enable_a20() {
    // Method 1: Via keyboard controller (paling kompatibel)
    unsafe {
        wait_a20_ready();
        x86::io::outb(0x64, 0xAD); // Disable keyboard
        wait_a20_ready();
        x86::io::outb(0x64, 0xD0); // Read output port
        wait_a20_ready();
        let val = x86::io::inb(0x60);
        wait_a20_ready();
        x86::io::outb(0x64, 0xD1); // Write output port
        wait_a20_ready();
        x86::io::outb(0x60, val | 0x02); // Set A20 bit
        wait_a20_ready();
        x86::io::outb(0x64, 0xAE); // Enable keyboard
        wait_a20_ready();
    }
}

fn wait_a20_ready() {
    unsafe {
        while x86::io::inb(0x64) & 0x02 != 0 {}
    }
}
```

### 5.3 Setup GDT Awal

```rust
// src/stage2/gdt.rs
use core::mem::size_of;

#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

static GDT: [GdtEntry; 3] = [
    // Null descriptor
    GdtEntry { limit_low: 0, base_low: 0, base_middle: 0,
               access: 0, granularity: 0, base_high: 0 },
    // Code segment (Ring 0)
    GdtEntry { limit_low: 0xFFFF, base_low: 0, base_middle: 0,
               access: 0x9A, granularity: 0xCF, base_high: 0 },
    // Data segment (Ring 0)
    GdtEntry { limit_low: 0xFFFF, base_low: 0, base_middle: 0,
               access: 0x92, granularity: 0xCF, base_high: 0 },
];
```

### 5.4 Long Mode - Setup Page Tables

```rust
// src/longmode/paging.rs
// 4-level paging: PML4 → PDPT → PD → PT

pub fn setup_page_tables() {
    // Alokasi page tables di lokasi tetap
    let pml4 = 0x1000 as *mut u64;
    let pdpt = 0x2000 as *mut u64;
    let pd   = 0x3000 as *mut u64;

    unsafe {
        // Bersihkan semua tables
        core::ptr::write_bytes(pml4, 0, 512);
        core::ptr::write_bytes(pdpt, 0, 512);
        core::ptr::write_bytes(pd, 0, 512);

        // PML4[0] → PDPT
        *pml4 = 0x2000 | 0b11; // Present + Writable

        // PDPT[0] → PD
        *pdpt = 0x3000 | 0b11; // Present + Writable

        // PD[0..n] → 2MB huge pages (identity mapping)
        for i in 0..512usize {
            *pd.add(i) = (i as u64 * 0x20_0000) | 0b1000_0011;
            // Present + Writable + Huge Page
        }
    }
}
```

### 5.5 ELF Kernel Loader

```rust
// src/elf/loader.rs

#[repr(C)]
struct ElfHeader {
    magic: [u8; 4],   // 0x7F 'E' 'L' 'F'
    class: u8,        // 2 = 64-bit
    data: u8,         // 1 = little-endian
    version: u8,
    os_abi: u8,
    _pad: [u8; 8],
    e_type: u16,
    e_machine: u16,   // 0x3E = x86_64
    e_version: u32,
    e_entry: u64,     // Entry point virtual address
    e_phoff: u64,     // Program header offset
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,     // Jumlah program headers
    // ...
}

#[repr(C)]
struct ProgramHeader {
    p_type: u32,      // 1 = PT_LOAD (perlu dimuat ke memori)
    p_flags: u32,
    p_offset: u64,    // Offset di file
    p_vaddr: u64,     // Virtual address tujuan
    p_paddr: u64,
    p_filesz: u64,    // Ukuran di file
    p_memsz: u64,     // Ukuran di memori (>= filesz, sisa diisi 0)
    p_align: u64,
}

pub fn load_kernel(kernel_data: &[u8]) -> u64 {
    let header = unsafe {
        &*(kernel_data.as_ptr() as *const ElfHeader)
    };

    // Validasi magic number
    assert_eq!(&header.magic, b"\x7FELF");
    assert_eq!(header.class, 2); // 64-bit
    assert_eq!(header.e_machine, 0x3E); // x86_64

    // Load setiap program header bertipe PT_LOAD
    let phoff = header.e_phoff as usize;
    let phnum = header.e_phnum as usize;

    for i in 0..phnum {
        let ph = unsafe {
            &*(kernel_data.as_ptr().add(phoff + i * 56) as *const ProgramHeader)
        };

        if ph.p_type != 1 { continue; } // Bukan PT_LOAD, skip

        // Copy dari file ke virtual address
        let dst = ph.p_vaddr as *mut u8;
        let src = kernel_data.as_ptr().wrapping_add(ph.p_offset as usize);
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, ph.p_filesz as usize);
            // Isi sisa dengan nol (BSS segment)
            core::ptr::write_bytes(
                dst.add(ph.p_filesz as usize),
                0,
                (ph.p_memsz - ph.p_filesz) as usize
            );
        }
    }

    header.e_entry // Return entry point
}
```

---

## 6. Build & Test Bootloader

### Linker Script

```ld
/* linker.ld */
ENTRY(_start)

SECTIONS {
    . = 0x7C00;     /* MBR loading address */

    .text : {
        *(.text.boot)
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data   : { *(.data*)   }
    .bss    : { *(.bss*)    }

    /DISCARD/ : { *(.eh_frame) *(.note*) }
}
```

### Build Command

```bash
# Build bootloader
cargo build --release --target x86_64-unknown-none

# Buat disk image
dd if=/dev/zero of=disk.img bs=1M count=64
dd if=target/x86_64-unknown-none/release/bootloader \
   of=disk.img conv=notrunc

# Test di QEMU
qemu-system-x86_64 \
    -drive file=disk.img,format=raw \
    -m 256M \
    -serial stdio \
    -display gtk
```

---

## 7. Multiboot2 Support (Opsional)

Jika menggunakan GRUB sebagai bootloader luar:

```rust
// Multiboot2 header di kernel
#[used]
#[link_section = ".multiboot2"]
static MULTIBOOT2_HEADER: [u32; 4] = [
    0xE85250D6,          // Magic number
    0,                    // Architecture: i386 protected mode
    16,                   // Header length
    (0x100000000u64 - (0xE85250D6u64 + 0 + 16)) as u32, // Checksum
];
```

---

## 8. Referensi

- [OSDev Bootloader Wiki](https://wiki.osdev.org/Bootloader)
- [bootloader crate](https://github.com/rust-osdev/bootloader)
- [ELF Format Specification](https://refspecs.linuxbase.org/elf/elf.pdf)
- [x86_64 Long Mode](https://wiki.osdev.org/Setting_Up_Long_Mode)
