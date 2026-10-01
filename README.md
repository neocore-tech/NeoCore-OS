# NeoCore-OS

Sistem Operasi *bare-metal* yang dibangun dari nol menggunakan bahasa Rust. 
Proyek ini mengimplementasikan kernel monolitik sederhana dengan berbagai fitur tingkat rendah.

## Fitur Saat Ini
- Bootloader Kustom & Inisialisasi GDT/IDT
- Driver Layar VGA Text Buffer
- Sistem Interrupt & Hardware Timer (PIT)
- Memory Manager & Heap Allocation
- Pembaca Waktu RTC (CMOS)
- Deteksi Hardware via PCI Bus Enumeration
- Identifikasi CPU melalui instruksi `CPUID` asli
- Ekstraksi MAC Address Kartu Jaringan RTL8139 via I/O Port
- *Cooperative Scheduler* & Multitasking Shell (`ncsh`)

## Cara Menjalankan
Pastikan Anda memiliki toolchain `rust-nightly`, paket `qemu-system-x86_64`, dan `make`.

Jalankan perintah berikut di dalam repositori:
```bash
make run
```
