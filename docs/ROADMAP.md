# 🗺️ ROADMAP — Peta Pengembangan NeoCore OS

---

## Visi

> Membangun OS desktop ringan yang modern dengan kernel Rust yang aman secara memori dan manajemen jaringan yang fleksibel menggunakan Go.

---

## Status Legend

| Simbol | Status |
|--------|--------|
| ✅ | Selesai |
| 🔄 | Sedang dikerjakan |
| 📋 | Planned (ada spesifikasi) |
| 📅 | Future (belum dirancang) |
| ❌ | Dibatalkan / Ditunda |

---

## 🏁 Phase 0 — Setup & Fondasi (Minggu 1-2)

**Tujuan:** Lingkungan pengembangan siap, "Hello World" kernel berjalan di QEMU.

```
[✅] Setup Rust nightly toolchain
[✅] Install QEMU untuk testing
[✅] Install cargo-bootimage
[✅] Target triple: x86_64-unknown-none
[✅] Dokumentasi lengkap (docs/*.md)
[📋] Init project structure
      ├── kernel/ (Cargo workspace)
      ├── netmgr/ (Go module)
      ├── shell/
      └── Makefile
[📋] "Hello World" di VGA text mode
[📋] Basic panic handler
[📋] Serial output (COM1) untuk debugging
```

**Milestone:** Kernel mencetak "Hello, NeoCore OS!" di QEMU VGA.

---

## 🔩 Phase 1 — Kernel Foundation (Minggu 3-6)

**Tujuan:** Kernel stabil dengan memory management dan interrupt handling.

```
[📋] Interrupt Handling
      ├── GDT (Global Descriptor Table) — Ring 0 + Ring 3
      ├── IDT (Interrupt Descriptor Table) — 256 entries
      ├── PIC 8259 initialization (IRQ remapping)
      ├── Timer interrupt (PIT 1000Hz)
      ├── Keyboard interrupt handler
      ├── Page fault handler
      ├── Double fault handler (IST)
      └── General protection fault handler

[📋] Memory Management
      ├── Physical frame allocator (bitmap-based)
      ├── 4-level paging (PML4 → PDPT → PD → PT)
      ├── Virtual memory mapper (OffsetPageTable)
      ├── Kernel heap initialization (4MB awal)
      ├── linked_list_allocator global allocator
      └── Memory statistics API

[📋] Basic Multitasking (cooperative)
      ├── Task struct (stack, state, context)
      ├── Simple executor (cooperative, no preemption)
      └── Async/await support untuk keyboard + shell
```

**Milestone:** Kernel dapat mengelola memori dan menangani interrupt tanpa crash.

**Deliverable:** `make test` semua unit test lulus.

---

## 💾 Phase 2 — Storage & Filesystem (Minggu 7-9)

**Tujuan:** Kernel dapat membaca dari disk, filesystem dasar berjalan.

```
[📋] PCI Bus Enumeration
      ├── Scan PCI devices
      ├── Read vendor/device ID
      └── Map BAR registers

[📋] ATA/IDE Driver (PIO mode)
      ├── Detect ATA drives
      ├── Read sectors (LBA28)
      ├── Write sectors
      └── Identify command

[📋] Filesystem — FAT32 (read-only dulu)
      ├── MBR/GPT partition parsing
      ├── FAT32 BPB parsing
      ├── Directory listing
      ├── File read
      └── Path resolution ("/etc/neonetd.toml")

[📋] VFS (Virtual Filesystem)
      ├── File descriptor abstraction
      ├── read() / write() / open() / close() syscalls
      └── Mount points (/, /dev, /proc)

[📋] InitRD (Initial RAM Disk)
      ├── Load initrd dari bootloader
      ├── Extract tar archive
      └── Mount sebagai rootfs
```

**Milestone:** Shell dapat `ls /`, `cat /etc/hostname`.

---

## 🌐 Phase 3 — Network Foundation (Minggu 10-13)

**Tujuan:** Kernel dapat mengirim dan menerima paket jaringan.

```
[📋] PCI VirtIO-net Driver (QEMU)
      ├── VirtIO device negotiation
      ├── VirtQueue setup (TX + RX)
      ├── Send Ethernet frames
      ├── Receive Ethernet frames
      └── DMA buffer management

[📋] smoltcp Integration
      ├── Implement smoltcp Device trait untuk VirtIO-net
      ├── Ethernet + ARP
      ├── IPv4
      ├── ICMP (Echo request/reply = ping)
      ├── UDP socket
      └── TCP socket

[📋] Socket Syscall API
      ├── socket() — buat socket
      ├── bind() — bind ke address
      ├── connect() — connect ke server
      ├── send() / sendto()
      ├── recv() / recvfrom()
      └── close()

[📋] Netlink IPC Character Device
      ├── /dev/netctl device file
      ├── dev_write() — Go → Kernel
      ├── dev_read() — Kernel → Go
      └── Message routing ke handler
```

**Milestone:** `ping 8.8.8.8` dari shell berfungsi.

---

## 📡 Phase 4 — Go Network Manager (Minggu 14-17)

**Tujuan:** neonetd berjalan, network auto-configured via DHCP.

```
[📋] Project Setup Go
      ├── go.mod init
      ├── Build system (cross-compile untuk NeoCore OS)
      └── Link ke kernel binary di initrd

[📋] IPC Client (Go)
      ├── Open /dev/netctl
      ├── Send/Recv dengan header protocol
      ├── Encode/Decode message types
      └── ACK handling + timeout

[📋] Interface Manager
      ├── Detect available interfaces dari kernel
      ├── Monitor interface state changes
      └── Apply configuration

[📋] DHCP Client
      ├── DHCP Discover broadcast
      ├── Parse DHCP Offer
      ├── DHCP Request
      ├── Parse DHCP ACK
      ├── Apply config ke kernel via IPC
      ├── Lease renewal (T1, T2, expire)
      └── Error recovery

[📋] DNS Resolver
      ├── UDP queries ke upstream servers
      ├── TTL-based cache
      ├── Multiple upstream failover
      └── Integration dengan shell nslookup

[📋] Routing Manager
      ├── Static routes dari config
      ├── Default gateway dari DHCP
      └── CRUD routing table

[📋] Firewall (Basic)
      ├── Default policies (drop incoming, allow established)
      ├── Allow ICMP
      ├── Allow outbound
      └── Rule management via config
```

**Milestone:** Boot OS, DHCP otomatis mendapat IP, `ping google.com` berhasil.

---

## 🖥️ Phase 5 — Shell & User Experience (Minggu 18-20)

**Tujuan:** Shell yang fully functional dengan network commands.

```
[📋] Shell (ncsh)
      ├── REPL loop
      ├── Command parser (tokenizer + AST)
      ├── Built-in: help, clear, history, exit
      ├── Built-in: ls, cat, echo, uname, uptime, free, ps
      ├── Built-in: ping, ifconfig, ip, route, netstat, nslookup
      ├── Pipe support (cmd1 | cmd2)
      ├── Redirection (> >> <)
      ├── Command history (↑↓ arrows)
      └── Tab completion

[📋] Process Management
      ├── fork() syscall
      ├── exec() syscall
      ├── Preemptive scheduler (Round-Robin)
      ├── Process priority
      └── Graceful process termination

[📋] TTY Support
      ├── Terminal emulator (ANSI escape codes)
      ├── Line discipline
      └── Job control (Ctrl+C, Ctrl+Z)
```

**Milestone:** Full shell session dengan network commands berfungsi.

---

## 🚀 Phase 6 — Advanced Features (Minggu 21+)

**Tujuan:** Fitur-fitur advanced untuk membuat OS lebih useful.

```
[📅] Intel e1000 Driver (Real Hardware)
      ├── MMIO register mapping
      ├── TX/RX descriptor rings
      ├── DMA setup
      └── Interrupt-driven I/O

[📅] IPv6 Support
      ├── smoltcp IPv6 enable
      ├── ICMPv6 + NDP
      ├── DHCPv6
      └── Dual-stack (IPv4 + IPv6)

[📅] WiFi Support
      ├── 802.11 driver (Intel iwlwifi-like)
      ├── WPA2/WPA3 authentication
      └── WiFi manager di neonetd

[📅] User Space
      ├── ELF binary loading
      ├── User/kernel mode separation (Ring 0/3)
      ├── Memory protection (separate address spaces)
      └── System call gate

[📅] GUI/Framebuffer
      ├── UEFI GOP framebuffer
      ├── Basic 2D renderer
      ├── Window manager (minimal)
      └── Font rendering

[📅] Networking Advanced
      ├── TCP connection pool
      ├── HTTP client library
      ├── SSH client
      ├── TLS (rustls integration)
      └── Network namespaces

[📅] Package Management
      ├── Simple package format (.kpkg)
      ├── Package installer
      └── Online package repo
```

---

## 📊 Timeline Visualisasi

```
Month 1          Month 2          Month 3          Month 4          Month 5+
│ Phase 0  │ Phase 1           │ Phase 2  │ Phase 3      │ Phase 4      │ Phase 5 │
├──────────┼───────────────────┼──────────┼──────────────┼──────────────┼─────────┤
│ Setup &  │ Kernel Foundation │ Storage  │ Network      │ Go NetMgr    │ Shell   │
│ Docs     │ (GDT,IDT,Mem,Task)│ & FS     │ (VirtIO,TCP) │ (DHCP,DNS)   │ + UX    │
└──────────┴───────────────────┴──────────┴──────────────┴──────────────┴─────────┘

Week:  1  2  3  4  5  6  7  8  9  10 11 12 13 14 15 16 17 18 19 20 21+
```

---

## 🎯 Key Milestones

| Milestone | Target | Deskripsi |
|-----------|--------|-----------|
| **M0** | Week 2  | "Hello World" kernel di QEMU |
| **M1** | Week 6  | Stable kernel + memory management |
| **M2** | Week 9  | Shell dapat baca file dari disk |
| **M3** | Week 13 | `ping 8.8.8.8` berfungsi |
| **M4** | Week 17 | DHCP auto-config + `ping google.com` |
| **M5** | Week 20 | Full shell session functional |
| **M6** | Week 24 | Run di hardware nyata (e1000) |

---

## 🐛 Known Issues & Technical Debt

| Issue | Priority | Notes |
|-------|----------|-------|
| smoltcp poll harus dipanggil periodik | High | Butuh timer-driven polling |
| Heap tidak bisa grow | Medium | Implement dynamic heap expansion |
| No user-space isolation | High | Fork/exec + memory isolation |
| Single-core only | Low | SMP support butuh spinlock audit |
| No power management | Low | ACPI integration needed |

---

## 📝 Contribution Areas

Kalau mau berkontribusi, fokus ke area ini:

1. **Beginner friendly:** VGA driver improvements, Shell built-in commands
2. **Intermediate:** ATA driver, FAT32 filesystem, DNS parser
3. **Advanced:** VirtIO-net driver, smoltcp integration, DHCP client state machine
4. **Expert:** SMP scheduler, user space isolation, TLS implementation

---

*Roadmap ini akan diupdate seiring perkembangan proyek.*
*Last updated: 2026-10-01*
