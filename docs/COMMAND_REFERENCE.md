# NeoCore OS (ncsh) Command Reference (Full Documentation)

Dokumen ini adalah **Manual Resmi (Man Pages)** untuk semua perintah di dalam NeoCore Shell (`ncsh`). Spesifikasi ini mencakup sintaks, deskripsi mendalam, dan status pengembangan dari semua fitur.

---

## 1. System & Hardware Diagnostics

### `sys`
* **Deskripsi:** Menampilkan informasi sistem secara keseluruhan (OS, arsitektur, jenis kernel, scheduler).
* **Penggunaan:** `sys`
* **Status:** **Selesai**

### `cpu`
* **Deskripsi:** Menampilkan spesifikasi prosesor (vendor, model, kecepatan, fitur MSR/CPUID) dan melakukan *stress-test* ringan pada core CPU.
* **Penggunaan:** `cpu [info|test]`
* **Status:** Direncanakan (Fase 4)

### `ram`
* **Deskripsi:** Menampilkan statistik fisik memori (Memory Map, E820) dan melakukan tes stabilitas RAM dengan pola tulis/baca blok.
* **Penggunaan:** `ram [info|test]`
* **Status:** Direncanakan

### `hw`
* **Deskripsi:** Merangkum semua informasi perangkat keras yang terdeteksi oleh BIOS/UEFI dan Kernel (Motherboard, ACPI, BIOS version).
* **Penggunaan:** `hw`
* **Status:** Direncanakan

### `pci` (alias `lspci`)
* **Deskripsi:** Melakukan enumerasi ke Bus PCI dan mendaftar semua perangkat keras yang terpasang (VGA, Network Card, SATA Controller).
* **Penggunaan:** `pci` atau `lspci`
* **Status:** **Selesai**

### `usb`
* **Deskripsi:** Mengecek perangkat USB yang terhubung ke pengontrol UHCI/EHCI/xHCI.
* **Penggunaan:** `usb list`
* **Status:** Direncanakan

### `diag`
* **Deskripsi:** Menjalankan diagnostik otomatis pada seluruh modul perangkat keras.
* **Penggunaan:** `diag`
* **Status:** Direncanakan

### `test`
* **Deskripsi:** Menjalankan *test suite* mandiri kernel (mirip unit testing OS).
* **Penggunaan:** `test all`
* **Status:** Direncanakan

### `temp`
* **Deskripsi:** Membaca sensor suhu CPU/Motherboard melalui ACPI/MSR.
* **Penggunaan:** `temp`
* **Status:** Direncanakan

---

## 2. Kernel & State

### `ver` / `uname`
* **Deskripsi:** Menampilkan versi kernel NeoCore, tanggal rilis, dan informasi arsitektur (*build info*).
* **Penggunaan:** `ver` atau `uname -a`
* **Status:** **Selesai**

### `up` / `uptime`
* **Deskripsi:** Menampilkan waktu sistem berjalan sejak mesin dinyalakan, berdasarkan perhitungan *tick* PIT 8254.
* **Penggunaan:** `uptime`
* **Status:** **Selesai**

### `load`
* **Deskripsi:** Mengecek beban sistem (*system load average*) dan persentase penggunaan CPU dari scheduler.
* **Penggunaan:** `load`
* **Status:** Direncanakan

### `time`
* **Deskripsi:** Mengambil waktu sistem (Jam, Menit, Detik, Tanggal) langsung dari perangkat keras *Real Time Clock* (RTC CMOS).
* **Penggunaan:** `time`
* **Status:** **Selesai**

### `dmesg`
* **Deskripsi:** Mencetak ulang pesan-pesan log kernel yang terjadi sejak proses booting (isi ring-buffer internal OS).
* **Penggunaan:** `dmesg`
* **Status:** Direncanakan

### `log`
* **Deskripsi:** Menampilkan log aplikasi/sistem (*System Logs*).
* **Penggunaan:** `log [tail|all]`
* **Status:** Direncanakan

### `env`
* **Deskripsi:** Melihat atau mengatur variabel *environment* untuk sesi shell saat ini.
* **Penggunaan:** `env` atau `env SET VAR=VALUE`
* **Status:** Direncanakan

---

## 3. Filesystem & Disk Management

### `dsk`
* **Deskripsi:** Menampilkan informasi *physical drive* (ATA/SATA) yang terdeteksi dan kapasitas aslinya.
* **Penggunaan:** `dsk`
* **Status:** Direncanakan

### `fs`
* **Deskripsi:** Menjalankan pengecekan integritas (*Filesystem Check*) pada tabel partisi FAT32 / MBR.
* **Penggunaan:** `fs check`
* **Status:** Direncanakan

### `mount` / `umount`
* **Deskripsi:** Me-*mount* (memuat) atau melepas partisi filesystem ke dalam direktori maya root (`/`).
* **Penggunaan:** `mount /dev/ata0p1 /mnt`
* **Status:** Direncanakan

### `ls`
* **Deskripsi:** Menampilkan daftar file dan direktori pada lokasi saat ini (Root FAT32).
* **Penggunaan:** `ls` atau `ls /path`
* **Status:** **Selesai (Kerangka)**

### `cd` & `pwd`
* **Deskripsi:** Berpindah direktori (`cd`) dan menampilkan direktori saat ini (`pwd`).
* **Penggunaan:** `cd /docs`
* **Status:** Direncanakan

### `cp`, `mv`, `rm`
* **Deskripsi:** Perintah operasi file: Salin (`cp`), Pindah/Ubah Nama (`mv`), Hapus (`rm`).
* **Penggunaan:** `cp file1.txt file2.txt`
* **Status:** Direncanakan

### `mkdir`
* **Deskripsi:** Membuat direktori atau folder baru di dalam *filesystem*.
* **Penggunaan:** `mkdir [nama_folder]`
* **Status:** Direncanakan

### `cat`
* **Deskripsi:** Membaca file teks dan mencetak isinya ke layar QEMU secara langsung.
* **Penggunaan:** `cat file.txt`
* **Status:** Direncanakan

### `nano`
* **Deskripsi:** Sebuah antarmuka *text editor* sederhana di layar terminal untuk mengedit teks secara langsung.
* **Penggunaan:** `nano file.txt`
* **Status:** Direncanakan

### `find` & `grep`
* **Deskripsi:** Mencari lokasi file dalam disk (`find`) atau mencari teks (*string*) di dalam file (`grep`).
* **Penggunaan:** `grep "kata" file.txt`
* **Status:** Direncanakan

### `df`, `du`, `iostat`
* **Deskripsi:** Statistik Disk: Kapasitas bebas (`df`), kapasitas terpakai per-folder (`du`), kecepatan baca-tulis IO (`iostat`).
* **Status:** Direncanakan

---

## 4. Networking (Fase 3 - Aktif Dikembangkan)

### `net`
* **Deskripsi:** Menampilkan informasi umum adapter jaringan (NIC), tipe koneksi, dan diagnostik kartu jaringan.
* **Penggunaan:** `net info`
* **Status:** Direncanakan

### `ip`
* **Deskripsi:** Mengatur atau melihat konfigurasi IP Address (IPv4). Saat ini mendukung interface `lo` (Loopback) dan `net0` (RTL8139).
* **Penggunaan:** `ip a`
* **Status:** **Selesai (Kerangka Mockup)**

### `ifconfig`
* **Deskripsi:** Menampilkan antarmuka jaringan (Perintah klasik alternatif dari `ip`).
* **Penggunaan:** `ifconfig`
* **Status:** Direncanakan

### `ping`
* **Deskripsi:** Mengirim paket ICMP Echo Request ke IP atau host tujuan untuk mengetes konektivitas kabel/jaringan.
* **Penggunaan:** `ping 8.8.8.8`
* **Status:** Direncanakan (Segera di Fase 3)

### `dns`
* **Deskripsi:** Mengetes protokol DNS UDP 53 untuk menerjemahkan nama domain web menjadi alamat IP.
* **Penggunaan:** `dns resolve google.com`
* **Status:** Direncanakan

### `rt` / `route`
* **Deskripsi:** Menampilkan atau mengatur *routing table* (Jalur internet keluar via *Gateway*).
* **Penggunaan:** `route show`
* **Status:** Direncanakan

### `netstat`
* **Deskripsi:** Menampilkan statistik koneksi jaringan aktif dan TCP/UDP port yang sedang mendengarkan (*listening*).
* **Penggunaan:** `netstat`
* **Status:** Direncanakan

### `arp`
* **Deskripsi:** Menampilkan tabel translasi MAC Address ke IP Address dari jaringan lokal (ARP Table).
* **Penggunaan:** `arp`
* **Status:** Direncanakan

### `hostname`
* **Deskripsi:** Melihat atau mengubah nama sistem (*hostname*) komputer di jaringan.
* **Penggunaan:** `hostname set neocore-pc`
* **Status:** Direncanakan

---

## 5. Process & Memory Management

### `proc` & `ps`
* **Deskripsi:** Menginspeksi struktur tugas/proses individu (`proc`) dan mendaftarkan proses yang berjalan di latar (`ps`).
* **Penggunaan:** `ps`
* **Status:** Direncanakan (Scheduler Phase 2)

### `top`
* **Deskripsi:** Menampilkan grafik interaktif dari penggunaan CPU/Memori untuk tiap proses (*real-time*).
* **Penggunaan:** `top`
* **Status:** Direncanakan

### `kill`
* **Deskripsi:** Mengirim sinyal penghentian paksa ke Task ID tertentu.
* **Penggunaan:** `kill 4`
* **Status:** Direncanakan

### `free` / `meminfo`
* **Deskripsi:** Menampilkan penggunaan memori (Heap Allocation / Frame Allocation) yang ada di RAM.
* **Penggunaan:** `meminfo` atau `free`
* **Status:** **Selesai**

### `svc`
* **Deskripsi:** Mengontrol layanan *background daemon* (seperti server jaringan).
* **Penggunaan:** `svc list`
* **Status:** Direncanakan

---

## 6. Security, Users & Packages

### `sec` & `perm`
* **Deskripsi:** Pengecekan keamanan (*Security Check*) dan Hak Akses file/direktori.
* **Status:** Direncanakan

### `pkg`
* **Deskripsi:** Modul pengunduhan (*Package Manager*) jarak jauh via HTTP jaringan.
* **Penggunaan:** `pkg install net-tools`
* **Status:** Direncanakan

### `usr`, `whoami`, `passwd`
* **Deskripsi:** Modul multi-pengguna. Membuat pengguna (`usr`), mengecek sesi aktif (`whoami`), mengganti sandi (`passwd`).
* **Status:** Direncanakan

---

## 7. Power & Utilities

### `help`
* **Deskripsi:** Menampilkan daftar pintasan (*cheatsheet*) perintah di layar.
* **Penggunaan:** `help`
* **Status:** **Selesai**

### `clear`
* **Deskripsi:** Menghapus seluruh teks di terminal (VGA Buffer).
* **Penggunaan:** `clear`
* **Status:** **Selesai**

### `echo`
* **Deskripsi:** Mencetak string kembali ke layar.
* **Penggunaan:** `echo Hello World`
* **Status:** **Selesai**

### `power`
* **Deskripsi:** Kontrol manajemen status ACPI tingkat lanjut.
* **Status:** Direncanakan

### `reboot`
* **Deskripsi:** Menembakkan sinyal 'Reset' (0xFE) via port Keyboard Controller agar PC mati hidup secara drastis (*Hard Reboot*).
* **Penggunaan:** `reboot`
* **Status:** **Selesai**

### `halt`
* **Deskripsi:** Menghentikan operasi CPU (*Halting CPU*) sepenuhnya (sistem berhenti total sampai direstart manual).
* **Penggunaan:** `halt`
* **Status:** **Selesai**

---
*(End of Reference File)*
