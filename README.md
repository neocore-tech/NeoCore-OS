<div align="center">
  <img src="https://raw.githubusercontent.com/neocore-tech/NeoCore-OS/main/IMG/doc5.png" alt="NeoCore OS Logo" width="600">
  <h1>NeoCore OS</h1>
  <p><b>A Modern, Bare-Metal Operating System written in Rust</b></p>
  
  [![Rust](https://img.shields.io/badge/Rust-Nightly-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
  [![Architecture](https://img.shields.io/badge/Arch-x86__64-blue?style=flat-square)](https://en.wikipedia.org/wiki/X86-64)
  [![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)
</div>

<br/>

NeoCore OS adalah Sistem Operasi hobi (*hobbyist OS*) yang dibangun murni dari awal (scratch) tanpa bersandar pada OS apa pun. Dibuat dengan bahasa pemrograman **Rust** untuk keamanan memori yang luar biasa, OS ini menargetkan arsitektur `x86_64` (PC modern) dan berjalan langsung di atas perangkat keras (Bare-Metal).

## 🚀 Fitur Utama

### 1. Kernel (Core)
- **Monolithic Kernel Architecture**: Kernel solid yang didesain secara monolitik.
- **Custom Bootloader Integration**: Proses booting mulus dari Real Mode hingga Long Mode 64-bit.
- **GDT & IDT**: Pemetaan Global Descriptor Table dan Interrupt Descriptor Table kustom.
- **Memory Manager (Paging & Heap)**: Alokator memori berbasis Linked-List dengan implementasi *Virtual Memory* sejati.

### 2. Hardware Drivers
- **VGA Text Mode Buffer (`0xB8000`)**: Menggambar teks berwarna dengan performa tinggi.
- **PS/2 Keyboard Controller**: Membaca input *scan-codes* secara real-time.
- **Programmable Interval Timer (PIT)**: Detak hardware 1000Hz (1 ms/tick) untuk sistem *uptime* dan *scheduler*.
- **Real-Time Clock (CMOS)**: Mengekstrak waktu langsung dari port `0x70` & `0x71` baterai CMOS.
- **PCI Bus Enumeration (`lspci`)**: Memindai seluruh motherboard untuk mencari periferal terpasang (VGA, Network, Disk).
- **RTL8139 Network Card**: Implementasi inisialisasi I/O Port langsung, menyalakan PCI Bus Mastering, dan membaca MAC Address bawaan silikon!
- **ATA PIO Hard Disk**: Pembaca raw-sector dari disk.

### 3. CPU & Processor
- **CPUID Interrogation**: OS ini melacak spesifikasi dan nama (Brand) dari Processor Host secara dinamis menggunakan instruksi `__cpuid` asli tanpa pustaka eksternal!
- **Hardware Power Control (ACPI)**: Mengirim sinyal murni via Keyboard Controller (Port `0x64`) untuk melakukan *Hard Reboot* PC.

### 4. Sistem File & Shell
- **FAT32 MBR Support**: Mampu mem-parsing skema partisi MBR dan melacak cluster tabel partisi FAT32.
- **NeoCore Shell (`ncsh`)**: Shell interaktif dengan dukungan puluhan perintah bawaan, warna teks dinamik, dan perintah keren seperti `neofetch`!
- **Cooperative Scheduler**: Mengadopsi mekanisme *multitasking* Fase 1.

---

## 🛠️ Build & Run

### Prasyarat
Untuk melakukan kompilasi proyek OS ini, Anda memerlukan perangkat berikut pada Linux Anda:
- `rustup` dengan toolchain **`nightly`** terpasang.
- `cargo-bootimage` (Bisa didapat dengan `cargo install bootimage`)
- `qemu-system-x86_64` (Untuk menjalankan virtualisasinya)
- `make`

### Menjalankan Sistem Operasi
Langkah kompilasi sudah kami sederhanakan sepenuhnya lewat `Makefile`. Cukup ketik perintah ini di root repositori:

```bash
make run
```
Skrip ini akan secara otomatis:
1. Mengkompilasi kernel menjadi *binary executable*.
2. Membungkus kernel ke dalam Master Boot Record (MBR) dengan `disk_builder`.
3. Membakar Image OS (`.img`).
4. Membangunkan PC Emulator (QEMU) lengkap dengan simulasi hardware jaringan.

---

## 📜 Perintah Shell Bawaan
Ketika mesin OS berhasil *booting*, cobalah perintah-perintah sakti berikut di dalam terminal `ncsh`:
- `neofetch` : Menampilkan Logo Sistem beserta info Spesifikasi dan Resolusi.
- `cpu` : Menembakkan instruksi CPUID ke *hardware* untuk membongkar nama spesifik CPU yang Anda pakai.
- `sys` : Menampilkan informasi dasar kernel beserta aktivitas Heap Memory riil.
- `time` : Membaca waktu/jam dari hardware Motherboard CMOS.
- `lspci` : *Scanning* Motherboard secara *brute-force* untuk mencetak semua kartu PCI!
- `ip` / `ping`: Demonstrasi awal komponen jaringan, lengkap dengan real MAC Address RTL8139!
- `reboot` : Lakukan uji coba *restart* paksa ke PC virtual Anda!
- `clear` : Membersihkan layar.

---

## 🏗️ Struktur Repositori

- `kernel/` : Jantung OS. Seluruh kode kernel Rust (`src/drivers`, `src/memory`, `src/shell`) ada di sini.
- `disk_builder/` : Alat untuk mem-build *Image* OS mentah beserta Bootloader-nya.
- `docs/` : Referensi perintah lengkap, peta jalan (*roadmap*), dan arsitektur OS (*under-the-hood*).
- `Makefile` : Resep otomatisasi untuk *Build*, *Run*, dan integrasi berkelanjutan.

---

<div align="center">
  <i>"Don't just use an operating system, build one."</i>
</div>
